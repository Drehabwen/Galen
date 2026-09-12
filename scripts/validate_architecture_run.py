"""Validate coverage and provenance metadata for an architecture-ablation run.

The analyzer intentionally supports partial pilot runs.  This companion
validator is the hard gate used before numbers are copied into a manuscript:
every declared variant must contain every declared case with the expected
repeat count, and the run manifest must match the frozen matrix.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


CASE_ID_RE = re.compile(r'^id\s*=\s*"([^"]+)"', re.MULTILINE)


def load_case_ids(cases_dir: Path) -> list[str]:
    ids: list[str] = []
    for path in sorted(cases_dir.glob("*.toml")):
        match = CASE_ID_RE.search(path.read_text(encoding="utf-8"))
        if match:
            ids.append(match.group(1))
    return ids


def hash_cases(cases_dir: Path, expected_cases: set[str]) -> str:
    import hashlib
    digest = hashlib.sha256()
    paths: list[Path] = []
    for path in cases_dir.glob("*.toml"):
        match = CASE_ID_RE.search(path.read_text(encoding="utf-8"))
        if match and match.group(1) in expected_cases:
            paths.append(path)
    for path in sorted(paths, key=lambda item: item.name):
        digest.update(path.name.encode("utf-8"))
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def load_rows(path: Path) -> list[dict]:
    rows: list[dict] = []
    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as exc:
            raise ValueError(f"{path}:{line_no}: invalid JSONL: {exc}") from exc
        if not isinstance(value, dict):
            raise ValueError(f"{path}:{line_no}: JSONL row must be an object")
        rows.append(value)
    return rows


def validate(run_dir: Path, matrix_path: Path, cases_dir: Path) -> dict:
    matrix = json.loads(matrix_path.read_text(encoding="utf-8"))
    manifest_path = run_dir / "manifest.json"
    errors: list[str] = []
    warnings: list[str] = []
    manifest: dict = {}
    if not manifest_path.is_file():
        errors.append("missing manifest.json")
    else:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

    expected_variants = set(manifest.get("variants") or [item["id"] for item in matrix["variants"]])
    expected_cases = set(manifest.get("cases") or load_case_ids(cases_dir))
    expected_repeat = int(manifest.get("repeat") or matrix.get("repeats_per_case", 0))

    actual_variants = {
        path.name for path in run_dir.iterdir() if path.is_dir() and not path.name.startswith(".")
    } if run_dir.is_dir() else set()
    missing_variants = sorted(expected_variants - actual_variants)
    extra_variants = sorted(actual_variants - expected_variants)
    if missing_variants:
        errors.append(f"missing variants: {', '.join(missing_variants)}")
    if extra_variants:
        warnings.append(f"unlisted variants present: {', '.join(extra_variants)}")

    by_variant: dict[str, dict] = {}
    for variant in sorted(actual_variants & expected_variants):
        variant_dir = run_dir / variant
        rows_by_case: dict[str, int] = {}
        rows_by_case_data: dict[str, list[dict]] = {}
        malformed_case_files: list[str] = []
        for path in sorted(variant_dir.glob("*.jsonl")):
            try:
                rows = load_rows(path)
            except ValueError as exc:
                errors.append(str(exc))
                continue
            file_case = path.stem.upper()
            for row in rows:
                row_case = str(row.get("case_id", "")).upper()
                if row_case != file_case:
                    malformed_case_files.append(f"{path.name}: row case_id={row_case or '<missing>'}")
                    continue
                rows_by_case[file_case] = rows_by_case.get(file_case, 0) + 1
                rows_by_case_data.setdefault(file_case, []).append(row)
        missing_cases = sorted(case for case in expected_cases if rows_by_case.get(case, 0) == 0)
        wrong_repeats = {
            case: count for case, count in sorted(rows_by_case.items())
            if case in expected_cases and count != expected_repeat
        }
        extra_cases = sorted(case for case in rows_by_case if case not in expected_cases)
        if missing_cases:
            errors.append(f"{variant}: missing cases: {', '.join(missing_cases)}")
        if wrong_repeats:
            errors.append(f"{variant}: wrong repeat counts: {wrong_repeats}")
        if extra_cases:
            warnings.append(f"{variant}: unlisted cases present: {', '.join(extra_cases)}")
        if malformed_case_files:
            errors.extend(f"{variant}: {item}" for item in malformed_case_files)
        for case, rows in rows_by_case_data.items():
            # A provider outage (for example HTTP 402) produces a syntactically
            # complete JSONL row but zero model/tool activity. It is useful for
            # operations debugging, never for an architecture comparison.
            if rows and all(
                int(row.get("model_requests", 0) or 0) == 0
                and not any(
                    assertion.get("name") == "run_completed" and assertion.get("pass")
                    for assertion in row.get("assertions", [])
                )
                for row in rows
            ):
                errors.append(
                    f"{variant}/{case}: all runs ended before a model request; provider unavailable"
                )
        by_variant[variant] = {
            "cases": rows_by_case,
            "missing_cases": missing_cases,
            "wrong_repeats": wrong_repeats,
        }

    if manifest:
        if manifest.get("status") != "completed":
            errors.append(f"manifest status is {manifest.get('status', '<missing>')}; run is not complete")
        if manifest.get("matrix_sha256"):
            import hashlib
            actual_hash = hashlib.sha256(matrix_path.read_bytes()).hexdigest()
            if manifest["matrix_sha256"] != actual_hash:
                errors.append("manifest matrix_sha256 does not match the supplied matrix")
        if manifest.get("cases_sha256"):
            actual_cases_hash = hash_cases(cases_dir, expected_cases)
            if manifest["cases_sha256"] != actual_cases_hash:
                errors.append("manifest cases_sha256 does not match the supplied task cards")
        if manifest.get("dry_run"):
            errors.append("manifest is marked dry_run; it cannot support manuscript results")
    return {
        "schema_version": 1,
        "run_dir": str(run_dir.resolve()),
        "matrix": str(matrix_path.resolve()),
        "expected": {
            "variants": sorted(expected_variants),
            "cases": sorted(expected_cases),
            "repeat": expected_repeat,
        },
        "by_variant": by_variant,
        "errors": errors,
        "warnings": warnings,
        "valid": not errors,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("run_dir", type=Path)
    parser.add_argument("--matrix", type=Path, default=Path("evals/experiments/architecture-v1/matrix.json"))
    parser.add_argument("--cases", type=Path, default=Path("evals/cases/paired"))
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = validate(args.run_dir.resolve(), args.matrix.resolve(), args.cases.resolve())
    output = (args.output or args.run_dir / "coverage-validation.json").resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if result["valid"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
