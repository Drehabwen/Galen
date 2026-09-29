#!/usr/bin/env python3
"""Fail-fast static checks for a generated ScienceAgentBench ClinTox program."""

from __future__ import annotations

import argparse
import ast
import json
import re
from pathlib import Path
from typing import Any

try:
    from .scienceagentbench_contract import DEFAULT_CONTRACT, load_contract
except ImportError:  # direct script execution
    from scienceagentbench_contract import DEFAULT_CONTRACT, load_contract


def _dotted_name(node: ast.AST) -> str | None:
    parts: list[str] = []
    while isinstance(node, ast.Attribute):
        parts.append(node.attr)
        node = node.value
    if isinstance(node, ast.Name):
        parts.append(node.id)
        return ".".join(reversed(parts))
    return None


def _has_call(tree: ast.AST, suffix: str) -> bool:
    return any(
        isinstance(node, ast.Call)
        and (_dotted_name(node.func) or "").endswith(suffix)
        for node in ast.walk(tree)
    )


def inspect_candidate(program: Path, contract: dict[str, Any]) -> dict[str, Any]:
    source = program.read_text(encoding="utf-8")
    checks: dict[str, dict[str, Any]] = {}

    def record(name: str, passed: bool, detail: str) -> None:
        checks[name] = {"passed": passed, "detail": detail}

    try:
        tree = ast.parse(source, filename=str(program))
    except SyntaxError as error:
        record("python_syntax", False, f"{error.msg} at line {error.lineno}")
        return {"schema_version": 1, "program": str(program), "passed": False, "checks": checks}
    record("python_syntax", True, "program parses as Python")

    unavailable = contract["capabilities"]["unavailable_symbols"]
    forbidden = [symbol for symbol in unavailable if symbol in source]
    record("available_classifier", bool(re.search(r"\b(?:dc|deepchem)\.models\.MultitaskClassifier\b", source)),
           "uses the verified DeepChem classifier namespace")
    record("no_unavailable_symbols", not forbidden,
           "none referenced" if not forbidden else f"referenced: {forbidden}")

    dataset_path = contract["dataset"]["runtime_path"]
    output_path = contract["runtime"]["working_directory"] + "/" + contract["output"]["relative_path"]
    record("runtime_paths", dataset_path in source and output_path in source,
           "authoritative dataset and output paths are encoded")
    positive_class = bool(re.search(r"\[\s*:\s*,\s*:\s*,\s*1\s*\]", source))
    record("positive_class_extraction", positive_class,
           "extracts class index 1 from (sample, task, class) predictions")

    width = int(contract["data_quality"]["fingerprint"]["size"])
    width_literal = re.search(rf"\b{width}\b", source) is not None
    circular = _has_call(tree, "CircularFingerprint")
    record("fixed_fingerprint_width", circular and width_literal,
           f"CircularFingerprint and fixed width {width} are present")

    zero_fallback = _has_call(tree, "zeros") and width_literal
    record("deterministic_zero_fallback", zero_fallback,
           f"a deterministic {width}-wide zero-vector fallback is present")
    stderr_logging = "sys.stderr" in source or "stderr" in source
    record("fallback_observability", stderr_logging,
           "fallback counts are emitted to stderr")

    dropping_tokens = ("dropna(", "drop_duplicates(")
    drops_rows = any(token in source.replace(" ", "") for token in dropping_tokens)
    preserves_smiles = "smiles" in source and "clintox_test_pred.csv" in source
    record("row_preservation", preserves_smiles and not drops_rows,
           "test SMILES are written without explicit row-dropping operations")

    target_names = set(contract["dataset"]["target_columns"])
    fallback_functions: list[ast.AST] = []
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            if _has_call(node, "zeros") and _has_call(node, "featurize"):
                fallback_functions.append(node)
    fallback_text = "\n".join(ast.get_source_segment(source, node) or "" for node in fallback_functions)
    leaks_labels = any(target in fallback_text for target in target_names)
    record("label_independent_fallback", bool(fallback_functions) and not leaks_labels,
           "featurization fallback is isolated from target columns")

    passed = all(item["passed"] for item in checks.values())
    return {
        "schema_version": 1,
        "contract_id": contract["contract_id"],
        "program": str(program.resolve()),
        "passed": passed,
        "checks": checks,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("program", type=Path)
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = inspect_candidate(args.program, load_contract(args.contract))
    payload = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload, encoding="utf-8")
    print(payload, end="")
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
