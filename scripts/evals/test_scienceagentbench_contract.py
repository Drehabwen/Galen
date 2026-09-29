from __future__ import annotations

import csv
import json
from pathlib import Path

import pytest

from scienceagentbench_contract import load_contract, render_prompt_block, validate_contract
from scienceagentbench_generation_smoke import _command
from preflight_scienceagentbench_candidate import inspect_candidate


def _benchmark(tmp_path: Path) -> Path:
    root = tmp_path / "benchmark"
    dataset = root / "datasets/clintox"
    evaluator = root / "eval_programs/clintox_nn_eval.py"
    dataset.mkdir(parents=True)
    evaluator.parent.mkdir(parents=True)
    evaluator.write_text("def eval(): return 1, '{}'\n", encoding="utf-8")
    for name in ("clintox_train.csv", "clintox_test.csv"):
        with (dataset / name).open("w", encoding="utf-8", newline="") as stream:
            writer = csv.writer(stream)
            writer.writerow(["smiles", "FDA_APPROVED", "CT_TOX"])
            writer.writerow(["CC", 1, 0])
    return root


def test_contract_validates_files_columns_and_runtime_mapping(tmp_path: Path) -> None:
    result = validate_contract(load_contract(), _benchmark(tmp_path))
    assert result["valid"] is True
    assert result["threshold"] == 0.77
    assert result["output_relative_path"] == "pred_results/clintox_test_pred.csv"


def test_prompt_exposes_authoritative_runtime_paths() -> None:
    rendered = render_prompt_block(load_contract())
    assert "/testbed/benchmark/datasets/clintox/clintox_train.csv" in rendered
    assert "/testbed/pred_results/clintox_test_pred.csv" in rendered
    assert "do not infer paths from the source-file location" in rendered
    assert "Runtime paths available during generation: no" in rendered
    assert "deepchem.models.MultitaskClassifier" in rendered
    assert "predictions[:, :, 1]" in rendered
    assert "all-zero vector" in rendered
    assert "Preserve every training row and every test row" in rendered
    assert "log both counts to stderr" in rendered


def test_contract_rejects_runtime_path_drift(tmp_path: Path) -> None:
    contract = json.loads(json.dumps(load_contract()))
    contract["dataset"]["runtime_path"] = "/wrong/clintox"
    with pytest.raises(ValueError, match="does not match"):
        validate_contract(contract, _benchmark(tmp_path))


def test_contract_rejects_prediction_axis_drift(tmp_path: Path) -> None:
    contract = json.loads(json.dumps(load_contract()))
    contract["prediction_semantics"]["raw_tensor_axes"] = ["sample", "class", "task"]
    with pytest.raises(ValueError, match="raw_tensor_axes drifted"):
        validate_contract(contract, _benchmark(tmp_path))


def test_contract_rejects_row_dropping_policy(tmp_path: Path) -> None:
    contract = json.loads(json.dumps(load_contract()))
    contract["data_quality"]["invalid_molecule_policy"]["preserve_test_rows"] = False
    with pytest.raises(ValueError, match="cannot drop rows"):
        validate_contract(contract, _benchmark(tmp_path))


def test_contract_rejects_variable_fallback_shape(tmp_path: Path) -> None:
    contract = json.loads(json.dumps(load_contract()))
    contract["data_quality"]["invalid_molecule_policy"]["required_feature_shape"] = [2048]
    with pytest.raises(ValueError, match="fixed feature shape"):
        validate_contract(contract, _benchmark(tmp_path))


def test_candidate_preflight_accepts_explicit_safe_fallback(tmp_path: Path) -> None:
    program = tmp_path / "program.py"
    program.write_text(
        '''import sys\nimport numpy as np\nimport deepchem as dc\n'''
        '''DATA = "/testbed/benchmark/datasets/clintox"\n'''
        '''OUTPUT = "/testbed/pred_results/clintox_test_pred.csv"\n'''
        '''def safe_features(featurizer, smiles):\n'''
        '''    rows = []\n'''
        '''    for value in smiles:\n'''
        '''        feature = featurizer.featurize([value])\n'''
        '''        rows.append(feature[0] if len(feature) and feature[0].shape == (1024,) else np.zeros(1024, dtype=np.float32))\n'''
        '''    print("fallback counts", file=sys.stderr)\n'''
        '''    return np.asarray(rows)\n'''
        '''featurizer = dc.feat.CircularFingerprint(size=1024)\n'''
        '''model = dc.models.MultitaskClassifier(n_tasks=2, n_features=1024)\n'''
        '''predictions = model.predict(None)\n'''
        '''positive = predictions[:, :, 1]\n'''
        '''print("smiles clintox_test_pred.csv", DATA, OUTPUT)\n''',
        encoding="utf-8",
    )
    assert inspect_candidate(program, load_contract())["passed"] is True


def test_claude_adapter_sends_long_prompt_over_stdin(tmp_path: Path) -> None:
    prompt = "runtime contract " * 1000
    command, stdin = _command("claude", prompt, tmp_path, "deepseek-flash")
    args_json = command[-1]
    assert prompt not in args_json
    assert stdin == prompt.encode("utf-8")
