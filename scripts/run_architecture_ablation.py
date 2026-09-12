"""Run the frozen architecture-ablation matrix against the Galen eval CLI.

The runner uses a process-local GALEN_ARCH_VARIANT switch and a temporary
HOME containing a copy of the user's models.toml. No persisted Galen/Codex
configuration is changed. Use --repeat 1 for smoke; formal runs should use
at least 5 repeats per card (20 for a paper-quality release result).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path


VARIANTS = {
    "stateless",
    "no_data_contract",
    "no_exec_contract",
    "no_evidence_link",
    "generic_same_runtime",
    "full_galen",
}


def redact(value: str) -> str:
    return re.sub(r"sk-[A-Za-z0-9_-]{8,}", "[REDACTED_SECRET]", value)


def case_id(path: Path) -> str | None:
    match = re.search(r'^id\s*=\s*"([^"]+)"', path.read_text(encoding="utf-8"), re.M)
    return match.group(1) if match else None


def hash_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def hash_cases(cases: list[Path]) -> str:
    """Hash the ordered task-card bytes, including filenames."""
    digest = hashlib.sha256()
    for path in sorted(cases, key=lambda item: item.name):
        digest.update(path.name.encode("utf-8"))
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def git_revision(repo_root: Path) -> str | None:
    """Return the checked-out revision without failing runs in source archives."""
    try:
        result = subprocess.run(
            ["git", "-C", str(repo_root), "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    value = result.stdout.strip()
    return value or None


def provider_unavailable(output: Path) -> bool:
    """Detect a provider outage before launching the remaining variants."""
    if not output.is_file():
        return False
    rows = []
    for line in output.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError:
            return False
    if not rows:
        return False
    return all(
        int(row.get("model_requests", 0) or 0) == 0
        and not any(
            assertion.get("name") == "run_completed" and assertion.get("pass")
            for assertion in row.get("assertions", [])
        )
        for row in rows
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--matrix", type=Path, default=Path("evals/experiments/architecture-v1/matrix.json"))
    parser.add_argument("--cases", type=Path, default=Path("evals/cases/paired"))
    parser.add_argument("--eval-exe", type=Path, default=Path("rust/target/debug/eval.exe"))
    parser.add_argument("--output-root", type=Path, default=Path("evals/runs/architecture-v1"))
    parser.add_argument("--repeat", type=int, default=1)
    parser.add_argument(
        "--run-id",
        default="",
        help="显式运行编号；未指定时按任务卡生成，避免不同任务卡混写同一目录",
    )
    parser.add_argument("--variants", nargs="*", default=sorted(VARIANTS))
    parser.add_argument("--cases-filter", nargs="*", default=[])
    parser.add_argument("--model", default="")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    if args.repeat < 1:
        parser.error("--repeat 必须大于 0")
    unknown = set(args.variants) - VARIANTS
    if unknown:
        parser.error(f"未知变体: {', '.join(sorted(unknown))}")
    matrix = json.loads(args.matrix.read_text(encoding="utf-8"))
    cases = sorted(path for path in args.cases.glob("*.toml") if case_id(path))
    selected = {value.upper() for value in args.cases_filter}
    if selected:
        cases = [path for path in cases if case_id(path).upper() in selected]
    if not cases:
        parser.error("没有找到任务卡")
    eval_exe = args.eval_exe.resolve()
    if not eval_exe.is_file() and not args.dry_run:
        parser.error(f"找不到 eval 可执行文件: {eval_exe}")

    output_root = args.output_root.resolve()
    output_root.mkdir(parents=True, exist_ok=True)
    case_tag = "-".join(case_id(path).lower() for path in cases)
    run_id = args.run_id or f"architecture-v1-{case_tag}-r{args.repeat}"
    run_dir = output_root / run_id
    if run_dir.exists() and any(run_dir.iterdir()) and not args.dry_run:
        parser.error(f"运行目录已存在且非空：{run_dir}；请使用新的 --run-id，避免覆盖既有结果")
    run_dir.mkdir(parents=True, exist_ok=True)
    repo_root = Path(__file__).resolve().parents[1]
    manifest = {
        "schema_version": 1,
        "experiment_id": matrix["experiment_id"],
        "run_id": run_id,
        "repeat": args.repeat,
        "variants": args.variants,
        "cases": [case_id(path) for path in cases],
        "matrix_sha256": hash_file(args.matrix),
        "cases_sha256": hash_cases(cases),
        "eval_exe": str(eval_exe),
        "eval_exe_sha256": hash_file(eval_exe) if eval_exe.is_file() else None,
        "runner_schema": "architecture-ablation-runner-v1.2",
        "architecture_switch": "GALEN_ARCH_VARIANT",
        "context_policy": "public_prompt_only_v1",
        "git_revision": git_revision(repo_root),
        "status": "running",
        "dry_run": args.dry_run,
    }
    manifest_path = run_dir / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    original_home = os.environ.get("HOME")
    original_userprofile = os.environ.get("USERPROFILE")
    source_models = Path(original_userprofile or os.path.expanduser("~")) / ".galen" / "models.toml"
    temp_home: Path | None = None
    completed = False
    try:
        if not args.dry_run:
            if not source_models.is_file():
                parser.error(f"找不到模型配置: {source_models}")
            temp_home = Path(tempfile.mkdtemp(prefix="galen-architecture-home-"))
            (temp_home / ".galen").mkdir(parents=True, exist_ok=True)
            shutil.copy2(source_models, temp_home / ".galen" / "models.toml")

        for variant in args.variants:
            variant_dir = run_dir / variant
            variant_dir.mkdir(parents=True, exist_ok=True)
            for path in cases:
                cid = case_id(path)
                output = (variant_dir / f"{cid}.jsonl").resolve()
                command = [
                    str(eval_exe),
                    "run",
                    "--case",
                    cid,
                    "--cases",
                    str(args.cases.resolve()),
                    "--repeat",
                    str(args.repeat),
                    "--output",
                    str(output),
                ]
                if args.model:
                    command.extend(["--model", args.model])
                print(f"RUN {variant}/{cid} x{args.repeat}")
                if args.dry_run:
                    print("  " + " ".join(command))
                    continue
                env = os.environ.copy()
                env["GALEN_ARCH_VARIANT"] = variant
                env["GALEN_EVAL_RUN_TAG"] = f"{run_id}-{variant}"
                env["HOME"] = str(temp_home)
                env["USERPROFILE"] = str(temp_home)
                result = subprocess.run(
                    command,
                    cwd=eval_exe.parent.parent.parent,
                    env=env,
                    capture_output=True,
                    text=True,
                    encoding="utf-8",
                    errors="replace",
                )
                log = variant_dir / f"{cid}.log"
                log.write_text(redact(result.stdout + "\n" + result.stderr), encoding="utf-8")
                if result.returncode != 0:
                    manifest["status"] = "failed"
                    manifest["failure"] = f"{variant}/{cid} exit={result.returncode}"
                    manifest_path.write_text(
                        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
                    )
                    print(f"FAIL {variant}/{cid} exit={result.returncode}; see {log}")
                    return result.returncode
                if provider_unavailable(output):
                    manifest["status"] = "provider_unavailable"
                    manifest["failure"] = f"{variant}/{cid} produced zero model requests"
                    manifest_path.write_text(
                        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
                    )
                    print(
                        f"STOP provider unavailable after {variant}/{cid}; "
                        f"see {log} and {output}"
                    )
                    return 2
        manifest["status"] = "completed"
        manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        completed = True
        print(f"DONE {run_dir}")
        return 0
    finally:
        if not completed and manifest_path.is_file():
            current = json.loads(manifest_path.read_text(encoding="utf-8"))
            if current.get("status") == "running":
                current["status"] = "aborted"
                manifest_path.write_text(
                    json.dumps(current, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
                )
        if original_home is None:
            os.environ.pop("HOME", None)
        else:
            os.environ["HOME"] = original_home
        if original_userprofile is None:
            os.environ.pop("USERPROFILE", None)
        else:
            os.environ["USERPROFILE"] = original_userprofile
        if temp_home is not None:
            shutil.rmtree(temp_home, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
