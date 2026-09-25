#!/usr/bin/env python3
"""Validate and orchestrate Galen's existing evaluation lanes.

This module deliberately contains no scoring logic. Domain scoring remains in
the native Rust evaluator; this file only validates and executes a versioned
suite manifest.
"""

from __future__ import annotations

import argparse
import datetime as dt
import fnmatch
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_MANIFEST = REPO_ROOT / "evals" / "suites" / "continuous-improvement-v1.json"
VALID_STAGES = {"pr", "nightly", "release"}
VALID_KINDS = {"command", "native_eval", "live_model", "experiment"}


class ManifestError(ValueError):
    pass


def load_manifest(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise ManifestError(f"manifest 不存在: {path}") from error
    except json.JSONDecodeError as error:
        raise ManifestError(f"manifest JSON 无效: {error}") from error
    if not isinstance(value, dict):
        raise ManifestError("manifest 顶层必须是对象")
    return value


def _non_empty_strings(value: Any, field: str) -> list[str]:
    if not isinstance(value, list) or not value or not all(isinstance(item, str) and item for item in value):
        raise ManifestError(f"{field} 必须是非空字符串数组")
    return value


def validate_manifest(manifest: dict[str, Any], root: Path = REPO_ROOT) -> list[str]:
    if manifest.get("schema_version") != 1:
        raise ManifestError("仅支持 schema_version=1")
    if not isinstance(manifest.get("suite_id"), str) or not manifest["suite_id"]:
        raise ManifestError("suite_id 不能为空")
    dimensions = set(_non_empty_strings(manifest.get("dimensions"), "dimensions"))
    lanes = manifest.get("lanes")
    if not isinstance(lanes, list) or not lanes:
        raise ManifestError("lanes 必须是非空数组")

    lane_ids: set[str] = set()
    warnings: list[str] = []
    for index, lane in enumerate(lanes):
        prefix = f"lanes[{index}]"
        if not isinstance(lane, dict):
            raise ManifestError(f"{prefix} 必须是对象")
        lane_id = lane.get("id")
        if not isinstance(lane_id, str) or not lane_id:
            raise ManifestError(f"{prefix}.id 不能为空")
        if lane_id in lane_ids:
            raise ManifestError(f"lane id 重复: {lane_id}")
        lane_ids.add(lane_id)
        if lane.get("stage") not in VALID_STAGES:
            raise ManifestError(f"{lane_id}.stage 必须是 {sorted(VALID_STAGES)}")
        if lane.get("kind") not in VALID_KINDS:
            raise ManifestError(f"{lane_id}.kind 必须是 {sorted(VALID_KINDS)}")
        lane_dimensions = set(_non_empty_strings(lane.get("dimensions"), f"{lane_id}.dimensions"))
        unknown = lane_dimensions - dimensions
        if unknown:
            raise ManifestError(f"{lane_id} 使用未声明维度: {sorted(unknown)}")
        _non_empty_strings(lane.get("owns"), f"{lane_id}.owns")
        _non_empty_strings(lane.get("hard_gates"), f"{lane_id}.hard_gates")

        for relative in lane.get("required_paths", []):
            if not isinstance(relative, str) or not relative:
                raise ManifestError(f"{lane_id}.required_paths 包含空路径")
            if not (root / relative).exists():
                raise ManifestError(f"{lane_id} 缺少依赖路径: {relative}")

        command = lane.get("command")
        if command is not None:
            if not isinstance(command, dict):
                raise ManifestError(f"{lane_id}.command 必须是对象")
            argv = _non_empty_strings(command.get("argv"), f"{lane_id}.command.argv")
            cwd = command.get("cwd")
            if not isinstance(cwd, str) or not (root / cwd).is_dir():
                raise ManifestError(f"{lane_id}.command.cwd 不存在: {cwd}")
            if any("\n" in item or "\r" in item for item in argv):
                raise ManifestError(f"{lane_id}.command.argv 不允许换行")
        elif lane["kind"] in {"command", "native_eval"}:
            raise ManifestError(f"{lane_id} 必须声明 command")

        if lane["kind"] == "live_model" and lane.get("repeat", 0) < manifest["policy"]["live_model_min_repeats"]:
            raise ManifestError(f"{lane_id}.repeat 低于 live_model_min_repeats")
        if command is None:
            warnings.append(f"{lane_id}: 声明型 lane，由现有专用 runner 执行")
    return warnings


def selected_lanes(manifest: dict[str, Any], stage: str, lane_ids: list[str]) -> list[dict[str, Any]]:
    selected = [lane for lane in manifest["lanes"] if lane["stage"] == stage]
    if lane_ids:
        requested = set(lane_ids)
        known = {lane["id"] for lane in selected}
        unknown = requested - known
        if unknown:
            raise ManifestError(f"阶段 {stage} 中不存在 lane: {sorted(unknown)}")
        selected = [lane for lane in selected if lane["id"] in requested]
    return selected


def changed_paths(root: Path) -> list[str]:
    commands = (
        ["git", "diff", "--name-only", "HEAD"],
        ["git", "ls-files", "--others", "--exclude-standard"],
    )
    paths: set[str] = set()
    for command in commands:
        result = subprocess.run(command, cwd=root, text=True, capture_output=True, check=False)
        if result.returncode != 0:
            raise ManifestError(f"无法读取 Git 改动: {' '.join(command)}")
        paths.update(line.strip().replace("\\", "/") for line in result.stdout.splitlines() if line.strip())
    return sorted(paths)


def lanes_for_changes(lanes: list[dict[str, Any]], paths: list[str]) -> list[dict[str, Any]]:
    return [
        lane
        for lane in lanes
        if any(fnmatch.fnmatch(path, pattern) for path in paths for pattern in lane["owns"])
    ]


def command_text(lane: dict[str, Any]) -> str:
    command = lane.get("command")
    if not command:
        return "<由专用 runner 执行>"
    return f"[{command['cwd']}] " + subprocess.list2cmdline(command["argv"])


def git_revision(root: Path) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=root, text=True, capture_output=True, check=False
    )
    return result.stdout.strip() if result.returncode == 0 else "unknown"


def run_lanes(lanes: list[dict[str, Any]], root: Path) -> list[dict[str, Any]]:
    results: list[dict[str, Any]] = []
    for lane in lanes:
        command = lane.get("command")
        if not command:
            results.append({"lane_id": lane["id"], "status": "delegated", "reason": "specialized runner required"})
            continue
        started = time.monotonic()
        completed = subprocess.run(command["argv"], cwd=root / command["cwd"], check=False)
        results.append(
            {
                "lane_id": lane["id"],
                "status": "passed" if completed.returncode == 0 else "failed",
                "exit_code": completed.returncode,
                "duration_ms": round((time.monotonic() - started) * 1000),
            }
        )
        if completed.returncode != 0:
            break
    return results


def report_passed(results: list[dict[str, Any]]) -> bool:
    """A delegated or skipped lane is incomplete, never a successful gate."""
    return bool(results) and all(item.get("status") == "passed" for item in results)


def write_report(path: Path, report: dict[str, Any]) -> None:
    if path.exists():
        raise ManifestError(f"拒绝覆盖已有报告: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("validate", "plan", "run"))
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--stage", choices=sorted(VALID_STAGES))
    parser.add_argument("--lane", action="append", default=[])
    parser.add_argument("--changed", action="store_true", help="只选择与当前 Git 改动匹配的 lane")
    parser.add_argument("--output", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        manifest_path = args.manifest.resolve()
        manifest = load_manifest(manifest_path)
        warnings = validate_manifest(manifest)
        if args.action == "validate":
            print(f"OK {manifest['suite_id']}: {len(manifest['lanes'])} lanes, {len(manifest['dimensions'])} dimensions")
            for warning in warnings:
                print(f"NOTE {warning}")
            return 0
        if not args.stage:
            raise ManifestError("plan/run 必须指定 --stage")
        lanes = selected_lanes(manifest, args.stage, args.lane)
        if args.changed:
            paths = changed_paths(REPO_ROOT)
            lanes = lanes_for_changes(lanes, paths)
        if not lanes:
            raise ManifestError(f"阶段 {args.stage} 没有匹配的 lane")
        for lane in lanes:
            print(f"{lane['id']}: {command_text(lane)}")
        if args.action == "plan":
            return 0
        if not args.output:
            raise ManifestError("run 必须指定 --output，避免结果丢失")
        results = run_lanes(lanes, REPO_ROOT)
        report = {
            "schema_version": 1,
            "suite_id": manifest["suite_id"],
            "stage": args.stage,
            "git_revision": git_revision(REPO_ROOT),
            "created_at": dt.datetime.now(dt.timezone.utc).isoformat(),
            "host": os.environ.get("COMPUTERNAME", "unknown"),
            "results": results,
            "passed": report_passed(results),
        }
        write_report(args.output.resolve(), report)
        return 0 if report["passed"] else 1
    except ManifestError as error:
        print(f"ERROR {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
