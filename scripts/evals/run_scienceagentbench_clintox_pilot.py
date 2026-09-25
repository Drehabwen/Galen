#!/usr/bin/env python3
"""Run the verified ScienceAgentBench ClinTox instance for staged agents."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
BENCHMARK = (
    ROOT
    / "evals"
    / "public-benchmarks"
    / "downloads"
    / "benchmark_verified"
    / "benchmark"
)
STAGED = ROOT / "evals" / "runs" / "scienceagentbench-after-download" / "pred_programs"
RUN_ROOT = ROOT / "evals" / "runs" / "scienceagentbench-clintox-pilot"
AGENTS = ("codex", "claude")
PROGRAM_NAME = "pred_clintox_nn.py"
TIMEOUT_SECONDS = 900


def file_hash(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json(path: Path, value: dict) -> None:
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def prepare_workspace(agent: str) -> tuple[Path, Path, str]:
    source = STAGED / agent / PROGRAM_NAME
    if not source.is_file():
        raise FileNotFoundError(f"Missing staged prediction: {source}")

    workspace = RUN_ROOT / agent
    workspace.mkdir(parents=True, exist_ok=True)
    program = workspace / PROGRAM_NAME
    source_sha256 = file_hash(source)
    if not program.exists() or file_hash(program) != source_sha256:
        shutil.copy2(source, program)

    dataset_target = workspace / "clintox"
    if not dataset_target.exists():
        shutil.copytree(BENCHMARK / "datasets" / "clintox", dataset_target)

    gold_target = workspace / "benchmark" / "eval_programs" / "gold_results"
    gold_target.mkdir(parents=True, exist_ok=True)
    shutil.copy2(
        BENCHMARK / "eval_programs" / "gold_results" / "clintox_gold.csv",
        gold_target / "clintox_gold.csv",
    )
    return workspace, program, source_sha256


def score(workspace: Path) -> tuple[int, float, str]:
    import pandas as pd
    from sklearn.metrics import roc_auc_score

    evaluator_path = BENCHMARK / "eval_programs" / "clintox_nn_eval.py"
    spec = importlib.util.spec_from_file_location("scienceagentbench_clintox_eval", evaluator_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Unable to load evaluator: {evaluator_path}")
    module = importlib.util.module_from_spec(spec)
    previous_cwd = Path.cwd()
    try:
        os.chdir(workspace)
        spec.loader.exec_module(module)
        official_result, official_detail = module.eval()
        pred = pd.read_csv(workspace / "pred_results" / "clintox_test_pred.csv")
        gold = pd.read_csv(workspace / "benchmark" / "eval_programs" / "gold_results" / "clintox_gold.csv")
        auc = float(
            roc_auc_score(
                gold[["FDA_APPROVED", "CT_TOX"]],
                pred[["FDA_APPROVED", "CT_TOX"]],
            )
        )
    finally:
        os.chdir(previous_cwd)
    return int(official_result), auc, str(official_detail)


def run_agent(agent: str, python: Path, force: bool) -> dict:
    workspace, program, source_sha256 = prepare_workspace(agent)
    result_path = workspace / "result.json"
    if result_path.exists() and not force:
        existing = json.loads(result_path.read_text(encoding="utf-8"))
        if existing.get("status") == "complete" and existing.get("source_sha256") == source_sha256:
            existing["skipped_existing_success"] = True
            return existing

    pred_dir = workspace / "pred_results"
    if pred_dir.exists():
        shutil.rmtree(pred_dir)

    started = time.time()
    env = os.environ.copy()
    env.setdefault("TF_CPP_MIN_LOG_LEVEL", "2")
    completed = subprocess.run(
        [str(python), str(program)],
        cwd=workspace,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=TIMEOUT_SECONDS,
    )
    (workspace / "program.stdout.log").write_text(completed.stdout, encoding="utf-8")
    (workspace / "program.stderr.log").write_text(completed.stderr, encoding="utf-8")

    result = {
        "schema_version": 1,
        "agent": agent,
        "status": "failed",
        "source_sha256": source_sha256,
        "program_returncode": completed.returncode,
        "duration_seconds": round(time.time() - started, 3),
        "valid_program": False,
        "roc_auc": None,
        "threshold": 0.77,
        "threshold_passed": False,
        "official_success": 0,
    }
    try:
        output = workspace / "pred_results" / "clintox_test_pred.csv"
        result["valid_program"] = completed.returncode == 0 and output.is_file()
        if result["valid_program"]:
            official_success, auc, detail = score(workspace)
            result.update(
                {
                    "status": "complete",
                    "roc_auc": auc,
                    "threshold_passed": auc >= result["threshold"],
                    "official_success": official_success,
                    "official_detail": detail,
                }
            )
    except Exception as exc:  # preserve program logs and a machine-readable failure
        result["score_error"] = f"{type(exc).__name__}: {exc}"

    write_json(result_path, result)
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--python", type=Path, required=True)
    parser.add_argument("--agent", choices=(*AGENTS, "all"), default="all")
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    if not args.python.is_file():
        raise FileNotFoundError(f"Python runtime not found: {args.python}")
    agents = AGENTS if args.agent == "all" else (args.agent,)
    results = [run_agent(agent, args.python.resolve(), args.force) for agent in agents]
    summary = {"schema_version": 1, "benchmark": "ScienceAgentBench/verified/ClinTox", "results": results}
    RUN_ROOT.mkdir(parents=True, exist_ok=True)
    write_json(RUN_ROOT / "summary.json", summary)
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    return 0 if all(item.get("status") == "complete" for item in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
