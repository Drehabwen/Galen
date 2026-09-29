#!/usr/bin/env python3
"""Render and validate explicit runtime contracts for ScienceAgentBench tasks."""

from __future__ import annotations

import argparse
import csv
import json
from pathlib import Path, PurePosixPath
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_CONTRACT = ROOT / "evals/contracts/scienceagentbench-clintox-v3.json"
DEFAULT_BENCHMARK = (
    ROOT / "evals/public-benchmarks/downloads/benchmark_verified/benchmark"
)


def load_contract(path: Path = DEFAULT_CONTRACT) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError("task contract must be a JSON object")
    return value


def _safe_relative(value: str, field: str) -> PurePosixPath:
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts or not path.parts:
        raise ValueError(f"{field} must be a non-empty safe relative path: {value!r}")
    return path


def validate_contract(contract: dict[str, Any], benchmark_root: Path) -> dict[str, Any]:
    if contract.get("schema_version") != 1:
        raise ValueError("unsupported contract schema_version")
    if not isinstance(contract.get("contract_id"), str) or not contract["contract_id"]:
        raise ValueError("contract_id is required")

    runtime = contract["runtime"]
    dataset = contract["dataset"]
    output = contract["output"]
    evaluation = contract["evaluation"]
    runtime_root = PurePosixPath(runtime["benchmark_root"])
    if not runtime_root.is_absolute():
        raise ValueError("runtime.benchmark_root must be absolute")

    dataset_rel = _safe_relative(dataset["relative_path"], "dataset.relative_path")
    expected_runtime_dataset = runtime_root.joinpath(dataset_rel)
    if PurePosixPath(dataset["runtime_path"]) != expected_runtime_dataset:
        raise ValueError("dataset.runtime_path does not match benchmark_root + relative_path")
    output_rel = _safe_relative(output["relative_path"], "output.relative_path")
    evaluator_rel = _safe_relative(evaluation["relative_path"], "evaluation.relative_path")

    local_dataset = benchmark_root.joinpath(*dataset_rel.parts)
    train = local_dataset / dataset["train_file"]
    test = local_dataset / dataset["test_file"]
    evaluator = benchmark_root.joinpath(*evaluator_rel.parts)
    required_columns = [dataset["feature_column"], *dataset["target_columns"]]
    for path in (train, test, evaluator):
        if not path.is_file():
            raise FileNotFoundError(f"required contract path missing: {path}")
    for path in (train, test):
        with path.open("r", encoding="utf-8", newline="") as stream:
            columns = next(csv.reader(stream))
        missing = [column for column in required_columns if column not in columns]
        if missing:
            raise ValueError(f"{path.name} is missing columns: {missing}")
    if output["required_columns"] != required_columns:
        raise ValueError("output.required_columns must preserve feature then target columns")
    threshold = float(evaluation["threshold"])
    if not 0.0 < threshold < 1.0:
        raise ValueError("evaluation.threshold must be between zero and one")
    enhanced_sections = ("generation", "capabilities", "prediction_semantics")
    present = [name in contract for name in enhanced_sections]
    if any(present) and not all(present):
        raise ValueError("enhanced contract sections must be provided together")
    if all(present):
        generation = contract["generation"]
        capabilities = contract["capabilities"]
        prediction = contract["prediction_semantics"]
        if generation["phase"] != "code_generation" or generation["runtime_paths_available_now"]:
            raise ValueError("generation phase must declare runtime paths unavailable")
        required_symbol = "deepchem.models.MultitaskClassifier"
        if required_symbol not in capabilities["available_symbols"]:
            raise ValueError(f"capability manifest is missing {required_symbol}")
        if prediction["raw_tensor_axes"] != ["sample", "task", "class"]:
            raise ValueError("prediction_semantics.raw_tensor_axes drifted")
        if prediction["output_task_order"] != dataset["target_columns"]:
            raise ValueError("prediction output task order must match dataset targets")

    data_quality = contract.get("data_quality")
    if contract["contract_id"] == "scienceagentbench-clintox-v3" and not data_quality:
        raise ValueError("contract v3 requires data_quality")
    if data_quality:
        fingerprint = data_quality["fingerprint"]
        policy = data_quality["invalid_molecule_policy"]
        if fingerprint != {"type": "ECFP", "size": 1024, "dtype": "float32"}:
            raise ValueError("data_quality fingerprint must be fixed 1024-wide float32 ECFP")
        if policy["processing_unit"] != "one input row at a time":
            raise ValueError("invalid molecule handling must operate one row at a time")
        if policy["required_feature_shape"] != [fingerprint["size"]]:
            raise ValueError("invalid molecule fallback must preserve the fixed feature shape")
        if policy["final_fallback"] != "all-zero float32 vector":
            raise ValueError("invalid molecule final fallback must be deterministic zero vector")
        required_true = (
            "preserve_train_rows", "preserve_test_rows", "preserve_test_order", "label_independent"
        )
        if not all(policy.get(field) is True for field in required_true):
            raise ValueError("invalid molecule policy cannot drop rows, reorder tests, or use labels")
        if policy["log_fallback_counts_to"] != "stderr":
            raise ValueError("invalid molecule fallback counts must be logged to stderr")

    return {
        "contract_id": contract["contract_id"],
        "benchmark_root": str(benchmark_root.resolve()),
        "dataset_root": str(local_dataset.resolve()),
        "train_file": str(train.resolve()),
        "test_file": str(test.resolve()),
        "evaluator": str(evaluator.resolve()),
        "output_relative_path": str(output_rel),
        "threshold": threshold,
        "valid": True,
    }


def render_prompt_block(contract: dict[str, Any]) -> str:
    runtime = contract["runtime"]
    dataset = contract["dataset"]
    output = contract["output"]
    evaluation = contract["evaluation"]
    generation = contract["generation"]
    capabilities = contract["capabilities"]
    prediction = contract["prediction_semantics"]
    data_quality = contract.get("data_quality")
    targets = ", ".join(dataset["target_columns"])
    columns = ", ".join(output["required_columns"])
    lines = [
            "Authoritative runtime contract (do not infer paths from the source-file location):",
            f"- Contract ID: {contract['contract_id']}",
            f"- Current phase: {generation['phase']}",
            "- Runtime paths available during generation: no",
            f"- Generation workspace rule: {generation['workspace_rule']}",
            f"- Working directory: {runtime['working_directory']}",
            f"- Dataset directory: {dataset['runtime_path']}",
            f"- Training data: {dataset['runtime_path']}/{dataset['train_file']}",
            f"- Test data: {dataset['runtime_path']}/{dataset['test_file']}",
            f"- Feature column: {dataset['feature_column']}",
            f"- Target columns: {targets}",
            f"- Required output: {runtime['working_directory']}/{output['relative_path']}",
            f"- Required output columns: {columns}",
            f"- Row order: {output['row_order']}",
            f"- Hardware profile: {runtime['hardware']}",
            f"- Success gate: {evaluation['metric']} >= {evaluation['threshold']}",
            f"- Verified DeepChem version: {capabilities['deepchem']}",
            "- Verified classifier symbol: deepchem.models.MultitaskClassifier (use dc.models.MultitaskClassifier)",
            "- Unavailable symbol: deepchem.models.torch_models.MultitaskClassifier",
            f"- Raw prediction axes: {', '.join(prediction['raw_tensor_axes'])}",
            f"- Required prediction extraction: {prediction['required_extraction']}",
            "Validate path existence only when the generated program runs inside /testbed. During generation, do not read or probe /testbed paths.",
        ]
    if data_quality:
        fingerprint = data_quality["fingerprint"]
        policy = data_quality["invalid_molecule_policy"]
        lines.extend(
            [
                "Input robustness requirements (these are submission gates):",
                f"- Featurize one input row at a time into fixed {fingerprint['size']}-wide {fingerprint['dtype']} {fingerprint['type']} vectors.",
                "- If standard featurization fails, is empty, or has the wrong shape, the final deterministic fallback is an all-zero vector of that same width.",
                "- A label-independent relaxed RDKit attempt is allowed before the final zero-vector fallback, but labels must never influence fallback behavior.",
                "- Preserve every training row and every test row; preserve test order exactly. Do not drop invalid molecules.",
                f"- Count training and test fallbacks separately and log both counts to {policy['log_fallback_counts_to']}.",
            ]
        )
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument("--benchmark-root", type=Path, default=DEFAULT_BENCHMARK)
    parser.add_argument("--render-prompt", action="store_true")
    args = parser.parse_args()
    contract = load_contract(args.contract)
    result = validate_contract(contract, args.benchmark_root)
    if args.render_prompt:
        result["prompt_block"] = render_prompt_block(contract)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
