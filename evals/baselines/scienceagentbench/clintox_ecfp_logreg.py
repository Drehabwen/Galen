#!/usr/bin/env python3
"""Deterministic, leakage-free ClinTox baseline for contract calibration."""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

import numpy as np
import pandas as pd
from rdkit import Chem
from rdkit.Chem import AllChem
from sklearn.linear_model import LogisticRegression


CONTRACT_PATH = Path(os.environ.get("GALEN_TASK_CONTRACT", "/testbed/task-contract.json"))


def fingerprint(smiles: str, size: int = 1024) -> tuple[np.ndarray, str]:
    molecule = Chem.MolFromSmiles(smiles)
    mode = "standard"
    if molecule is None:
        molecule = Chem.MolFromSmiles(smiles, sanitize=False)
        if molecule is None:
            return np.zeros((size,), dtype=np.int8), "zero_fallback"
        operations = Chem.SanitizeFlags.SANITIZE_ALL ^ Chem.SanitizeFlags.SANITIZE_KEKULIZE
        try:
            Chem.SanitizeMol(molecule, sanitizeOps=operations)
            mode = "relaxed"
        except (ValueError, RuntimeError):
            return np.zeros((size,), dtype=np.int8), "zero_fallback"
    vector = np.zeros((size,), dtype=np.int8)
    bit_vector = AllChem.GetMorganFingerprintAsBitVect(molecule, 2, nBits=size)
    for index in bit_vector.GetOnBits():
        vector[index] = 1
    return vector, mode


def featurize(values: pd.Series) -> tuple[np.ndarray, dict[str, int]]:
    rows = [fingerprint(value) for value in values]
    counts = {mode: sum(int(row[1] == mode) for row in rows) for mode in ("standard", "relaxed", "zero_fallback")}
    return np.stack([row[0] for row in rows]), counts


def probabilities(train_x: np.ndarray, labels: np.ndarray, test_x: np.ndarray) -> np.ndarray:
    classes = np.unique(labels)
    if len(classes) == 1:
        return np.full(len(test_x), float(classes[0]), dtype=float)
    model = LogisticRegression(
        C=1.0,
        class_weight="balanced",
        max_iter=2000,
        random_state=42,
        solver="liblinear",
    )
    model.fit(train_x, labels)
    positive_index = int(np.flatnonzero(model.classes_ == 1)[0])
    return model.predict_proba(test_x)[:, positive_index]


def main() -> None:
    if not CONTRACT_PATH.is_file():
        raise FileNotFoundError(f"authoritative task contract missing: {CONTRACT_PATH}")
    contract = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))
    dataset = contract["dataset"]
    dataset_root = Path(dataset["runtime_path"])
    train_path = dataset_root / dataset["train_file"]
    test_path = dataset_root / dataset["test_file"]
    for path in (train_path, test_path):
        if not path.is_file():
            raise FileNotFoundError(f"required contract input missing: {path}")

    feature = dataset["feature_column"]
    targets = dataset["target_columns"]
    train = pd.read_csv(train_path)
    test = pd.read_csv(test_path)
    train_x, train_modes = featurize(train[feature])
    test_x, test_modes = featurize(test[feature])
    print(
        json.dumps(
            {
                "train_rows": len(train),
                "test_rows": len(test),
                "train_featurization": train_modes,
                "test_featurization": test_modes,
            }
        ),
        file=sys.stderr,
    )
    result = pd.DataFrame({feature: test[feature]})
    for target in targets:
        result[target] = probabilities(train_x, train[target].to_numpy(), test_x)

    output = Path(contract["runtime"]["working_directory"]) / contract["output"]["relative_path"]
    output.parent.mkdir(parents=True, exist_ok=True)
    result.to_csv(output, index=False)


if __name__ == "__main__":
    main()
