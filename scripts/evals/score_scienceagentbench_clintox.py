#!/usr/bin/env python3
"""Run the official ClinTox evaluator and expose its numeric ROC-AUC."""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path

import pandas as pd
from sklearn.metrics import roc_auc_score


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--testbed", type=Path, default=Path("/testbed"))
    args = parser.parse_args()
    root = args.testbed.resolve()
    evaluator = root / "benchmark/eval_programs/clintox_nn_eval.py"
    spec = importlib.util.spec_from_file_location("official_clintox_eval", evaluator)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load official evaluator: {evaluator}")
    module = importlib.util.module_from_spec(spec)
    previous = Path.cwd()
    try:
        os.chdir(root)
        spec.loader.exec_module(module)
        official_success, official_detail = module.eval()
        pred = pd.read_csv(root / "pred_results/clintox_test_pred.csv")
        gold = pd.read_csv(root / "benchmark/eval_programs/gold_results/clintox_gold.csv")
        auc = float(
            roc_auc_score(
                gold[["FDA_APPROVED", "CT_TOX"]],
                pred[["FDA_APPROVED", "CT_TOX"]],
            )
        )
    finally:
        os.chdir(previous)
    result = {
        "valid_program": 1,
        "roc_auc": auc,
        "threshold": 0.77,
        "threshold_passed": auc >= 0.77,
        "official_success": int(official_success),
        "official_detail": official_detail,
    }
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if official_success else 1


if __name__ == "__main__":
    raise SystemExit(main())
