"""Score the Galen-SciCode pilot with evaluator-generated hidden datasets.

This is a ScienceAgentBench-style local pilot, not an official
ScienceAgentBench score. It executes only programs that pass a conservative
static safety gate and never exposes hidden rows to the agent workspace.
"""

from __future__ import annotations

import argparse
import ast
import csv
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
SCRIPT_REL = Path("output/scicode/analyze_recovery.py")
PUBLIC_SUMMARY = Path("output/scicode/pred_results/recovery_summary.csv")
PUBLIC_QUALITY = Path("output/scicode/pred_results/quality_report.json")
REQUIRED = ["rehab_id", "timepoint", "HRV_RMSSD_ms", "CMJ_cm", "RPE"]
SUMMARY_FIELDS = [
    "rehab_id", "rmssd_change_24h", "rmssd_recovery_pct_72h",
    "cmj_change_24h", "cmj_recovery_pct_72h", "peak_rpe",
]
ALLOWED_IMPORTS = {
    "argparse", "csv", "json", "pathlib", "math", "statistics", "decimal",
    "collections", "datetime", "typing", "sys", "os",
}
FORBIDDEN_CALLS = {
    "eval", "exec", "compile", "__import__", "system", "popen", "remove",
    "unlink", "rmdir", "removedirs", "rmtree", "spawn", "fork",
}


HIDDEN_DATASETS: dict[str, list[dict[str, str]]] = {
    "unseen_reordered": [
        {"rehab_id": "NEW-B", "timepoint": "72h", "HRV_RMSSD_ms": "50", "CMJ_cm": "30", "RPE": "3", "quality_status": "VERIFIED"},
        {"rehab_id": "NEW-A", "timepoint": "24h", "HRV_RMSSD_ms": "30", "CMJ_cm": "20", "RPE": "8", "quality_status": "verified"},
        {"rehab_id": "NEW-B", "timepoint": "0h", "HRV_RMSSD_ms": "55", "CMJ_cm": "33", "RPE": "4", "quality_status": "verified"},
        {"rehab_id": "NEW-A", "timepoint": "72h", "HRV_RMSSD_ms": "45", "CMJ_cm": "29", "RPE": "4", "quality_status": "verified"},
        {"rehab_id": "NEW-B", "timepoint": "24h", "HRV_RMSSD_ms": "40", "CMJ_cm": "25", "RPE": "7", "quality_status": "verified"},
        {"rehab_id": "NEW-A", "timepoint": "0h", "HRV_RMSSD_ms": "50", "CMJ_cm": "32", "RPE": "3", "quality_status": "verified"},
    ],
    "quality_edge_cases": [
        {"rehab_id": "EDGE-1", "timepoint": "0h", "HRV_RMSSD_ms": "60", "CMJ_cm": "36", "RPE": "4", "quality_status": "verified"},
        {"rehab_id": "EDGE-1", "timepoint": "24h", "HRV_RMSSD_ms": "45", "CMJ_cm": "30", "RPE": "9", "quality_status": "verified"},
        {"rehab_id": "EDGE-1", "timepoint": "24h", "HRV_RMSSD_ms": "44", "CMJ_cm": "29", "RPE": "10", "quality_status": "verified"},
        {"rehab_id": "EDGE-1", "timepoint": "72h", "HRV_RMSSD_ms": "60", "CMJ_cm": "35", "RPE": "5", "quality_status": "verified"},
        {"rehab_id": "EDGE-2", "timepoint": "0h", "HRV_RMSSD_ms": "40", "CMJ_cm": "28", "RPE": "3", "quality_status": "verified"},
        {"rehab_id": "EDGE-2", "timepoint": "24h", "HRV_RMSSD_ms": "40", "CMJ_cm": "28", "RPE": "6", "quality_status": "verified"},
        {"rehab_id": "EDGE-2", "timepoint": "72h", "HRV_RMSSD_ms": "42", "CMJ_cm": "29", "RPE": "4", "quality_status": "verified"},
        {"rehab_id": "DROP-1", "timepoint": "0h", "HRV_RMSSD_ms": "", "CMJ_cm": "31", "RPE": "4", "quality_status": "verified"},
        {"rehab_id": "DROP-2", "timepoint": "0h", "HRV_RMSSD_ms": "52", "CMJ_cm": "32", "RPE": "4", "quality_status": "rejected"},
    ],
}


def _read_rows(path: Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8-sig", newline="") as handle:
        return list(csv.DictReader(handle))


def _gold(rows: list[dict[str, str]]) -> tuple[list[dict[str, Any]], dict[str, int]]:
    seen: set[tuple[str, str]] = set()
    duplicate_rows = 0
    verified_rows = 0
    missing = 0
    usable: dict[str, dict[str, dict[str, str]]] = {}
    for row in rows:
        is_missing = any(not str(row.get(field, "")).strip() for field in REQUIRED)
        missing += int(is_missing)
        verified = str(row.get("quality_status", "")).strip().lower() == "verified"
        verified_rows += int(verified)
        key = (str(row.get("rehab_id", "")).strip(), str(row.get("timepoint", "")).strip())
        if key in seen:
            duplicate_rows += 1
            continue
        seen.add(key)
        if is_missing or not verified:
            continue
        usable.setdefault(key[0], {})[key[1]] = row

    output: list[dict[str, Any]] = []
    for rehab_id in sorted(usable):
        points = usable[rehab_id]
        if not {"0h", "24h", "72h"}.issubset(points):
            continue
        def metric(field: str) -> tuple[float, float | None]:
            baseline = float(points["0h"][field])
            low = float(points["24h"][field])
            end = float(points["72h"][field])
            denominator = baseline - low
            recovery = None if denominator == 0 else round((end - low) / denominator * 100, 2)
            return round(low - baseline, 2), recovery
        rmssd_change, rmssd_recovery = metric("HRV_RMSSD_ms")
        cmj_change, cmj_recovery = metric("CMJ_cm")
        output.append({
            "rehab_id": rehab_id,
            "rmssd_change_24h": rmssd_change,
            "rmssd_recovery_pct_72h": rmssd_recovery,
            "cmj_change_24h": cmj_change,
            "cmj_recovery_pct_72h": cmj_recovery,
            "peak_rpe": max(float(point["RPE"]) for point in points.values()),
        })
    quality = {
        "row_count": len(rows), "verified_rows": verified_rows,
        "duplicate_rows": duplicate_rows, "missing_required_values": missing,
        "complete_athletes": len(output),
    }
    return output, quality


def _static_gate(path: Path) -> tuple[bool, list[str], ast.AST | None]:
    problems: list[str] = []
    try:
        source = path.read_text(encoding="utf-8")
        tree = ast.parse(source, filename=str(path))
        compile(tree, str(path), "exec")
    except (OSError, SyntaxError, UnicodeError) as error:
        return False, [f"compile: {error}"], None
    for node in ast.walk(tree):
        if isinstance(node, (ast.Import, ast.ImportFrom)):
            modules = [alias.name.split(".")[0] for alias in node.names] if isinstance(node, ast.Import) else [(node.module or "").split(".")[0]]
            for module in modules:
                if module not in ALLOWED_IMPORTS:
                    problems.append(f"non-standard or disallowed import: {module}")
        if isinstance(node, ast.Call):
            name = node.func.id if isinstance(node.func, ast.Name) else (node.func.attr if isinstance(node.func, ast.Attribute) else "")
            if name.lower() in FORBIDDEN_CALLS:
                problems.append(f"forbidden call: {name}")
    for marker in ("ATH-001", "ATH-002", "D:\\Users\\DORAT"):
        if marker in source:
            problems.append(f"hardcoded public value/path: {marker}")
    return not problems, sorted(set(problems)), tree


def _write_dataset(path: Path, rows: list[dict[str, str]]) -> None:
    fields = ["rehab_id", "timestamp", "timepoint", "HRV_RMSSD_ms", "CMJ_cm", "RPE", "source", "collection_context", "quality_status"]
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        for row in rows:
            writer.writerow({field: row.get(field, "") for field in fields})


def _equal_summary(actual_path: Path, expected: list[dict[str, Any]]) -> tuple[bool, str]:
    try:
        rows = _read_rows(actual_path)
    except Exception as error:
        return False, f"summary unreadable: {error}"
    if (list(rows[0].keys()) if rows else []) != SUMMARY_FIELDS:
        return False, f"summary schema mismatch: {list(rows[0].keys()) if rows else []}"
    if len(rows) != len(expected):
        return False, f"summary row count {len(rows)} != {len(expected)}"
    for actual, wanted in zip(rows, expected):
        if actual.get("rehab_id") != wanted["rehab_id"]:
            return False, "rehab_id/order mismatch"
        for field in SUMMARY_FIELDS[1:]:
            value = actual.get(field, "").strip()
            target = wanted[field]
            if target is None:
                if value:
                    return False, f"{field} should be empty"
            else:
                try:
                    if abs(float(value) - float(target)) > 0.005:
                        return False, f"{field} mismatch for {wanted['rehab_id']}"
                except ValueError:
                    return False, f"{field} is not numeric"
    return True, "ok"


def _equal_quality(path: Path, expected: dict[str, int]) -> tuple[bool, str]:
    try:
        actual = json.loads(path.read_text(encoding="utf-8"))
    except Exception as error:
        return False, f"quality unreadable: {error}"
    return (actual == expected, "ok" if actual == expected else f"quality mismatch: {actual} != {expected}")


def _run_dataset(script: Path, rows: list[dict[str, str]], label: str) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix=f"galen-scicode-{label}-") as directory:
        root = Path(directory)
        input_path = root / "input.csv"
        output_dir = root / "results"
        _write_dataset(input_path, rows)
        expected_summary, expected_quality = _gold(rows)
        try:
            result = subprocess.run(
                [sys.executable, str(script), "--input", str(input_path), "--output-dir", str(output_dir)],
                cwd=script.parent, capture_output=True, text=True, timeout=20,
            )
        except subprocess.TimeoutExpired:
            return {"pass": False, "detail": "execution timed out"}
        if result.returncode != 0:
            return {"pass": False, "detail": f"exit={result.returncode}: {result.stderr[-500:]}"}
        summary_ok, summary_detail = _equal_summary(output_dir / "recovery_summary.csv", expected_summary)
        quality_ok, quality_detail = _equal_quality(output_dir / "quality_report.json", expected_quality)
        return {"pass": summary_ok and quality_ok, "summary": summary_detail, "quality": quality_detail}


def _score_record(record: dict[str, Any]) -> dict[str, Any]:
    workspace = Path(record.get("workspace", ""))
    script = workspace / SCRIPT_REL
    exists = script.is_file() and script.stat().st_size > 0
    static_ok, static_problems, _ = _static_gate(script) if exists else (False, ["script missing"], None)
    public_rows = _read_rows(workspace / "inputs/fatigue-cohort.csv") if (workspace / "inputs/fatigue-cohort.csv").is_file() else []
    public_execution = _run_dataset(script, public_rows, "public") if static_ok and public_rows else {"pass": False, "detail": "static gate or public input failed"}
    expected_summary, expected_quality = _gold(public_rows) if public_rows else ([], {})
    artifact_summary = _equal_summary(workspace / PUBLIC_SUMMARY, expected_summary) if expected_summary else (False, "public input missing")
    artifact_quality = _equal_quality(workspace / PUBLIC_QUALITY, expected_quality) if expected_quality else (False, "public input missing")
    artifact_ok = artifact_summary[0] and artifact_quality[0]
    hidden = {name: _run_dataset(script, rows, name) if static_ok else {"pass": False, "detail": "static gate failed"} for name, rows in HIDDEN_DATASETS.items()}
    hidden_passes = sum(int(result["pass"]) for result in hidden.values())
    score = (
        15 * int(exists and static_ok)
        + 15 * int(public_execution["pass"])
        + 10 * int(artifact_ok)
        + 45 * hidden_passes / len(HIDDEN_DATASETS)
        + 15 * int(static_ok)
    )
    return {
        "case_id": record.get("case_id"), "model": record.get("model"),
        "run_id": record.get("run_id"), "workspace": str(workspace),
        "score": round(score, 2), "pass": score == 100,
        "latency_ms": record.get("latency", {}).get("total_ms"),
        "checks": {
            "script_exists_and_compiles": exists and static_ok,
            "public_execution": public_execution,
            "public_artifacts_match": {"pass": artifact_ok, "summary": artifact_summary[1], "quality": artifact_quality[1]},
            "hidden_generalization": hidden,
            "static_safety": {"pass": static_ok, "problems": static_problems},
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("validate")
    score = sub.add_parser("score")
    score.add_argument("--input", type=Path, required=True)
    score.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "validate":
        case = ROOT / "evals/cases/scicode/scicode01_recovery_analysis.toml"
        fixture = ROOT / "evals/fixtures/scicode/recovery/inputs/fatigue-cohort.csv"
        if not case.is_file() or not fixture.is_file():
            raise SystemExit("SCICODE01 case or fixture is missing")
        _gold(_read_rows(fixture))
        print("OK SCICODE01: public fixture and 2 hidden datasets validated")
        return 0
    if args.output.exists():
        raise SystemExit(f"refusing to overwrite: {args.output}")
    records = [json.loads(line) for line in args.input.read_text(encoding="utf-8").splitlines() if line.strip()]
    results = [_score_record(record) for record in records]
    report = {
        "schema_version": 1, "benchmark": "Galen-SciCode Pilot v1",
        "official_scienceagentbench_score": False,
        "weights": {"compile_and_delivery": 15, "public_execution": 15, "public_artifacts": 10, "hidden_generalization": 45, "static_safety": 15},
        "results": results,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    for result in results:
        print(f"{result['model']} score={result['score']:.2f} pass={result['pass']} latency_ms={result['latency_ms']}")
    return 0 if all(result["pass"] for result in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
