"""Inspect AI tasks that execute Galen's native Rust evaluation harness."""

from __future__ import annotations

import json
import os
import tempfile
from pathlib import Path
from typing import Any

from inspect_ai import Task, task
from inspect_ai.dataset import Sample
from inspect_ai.model import ModelOutput
from inspect_ai.scorer import CORRECT, INCORRECT, Score, Target, accuracy, scorer, stderr
from inspect_ai.solver import Generate, TaskState, solver
from inspect_ai.util import subprocess

from galen_agent_eval.scenarios import load_scenarios
from galen_agent_eval.simulated_user import simulate_user_trace, trace_passed


REPO_ROOT = Path(__file__).resolve().parents[3]
CASES_DIR = REPO_ROOT / "evals" / "cases"


def _native_eval_binary() -> Path:
    configured = os.environ.get("GALEN_EVAL_BIN")
    if configured:
        return Path(configured).resolve()
    suffix = ".exe" if os.name == "nt" else ""
    return REPO_ROOT / "rust" / "target" / "debug" / f"eval{suffix}"


def _samples() -> list[Sample]:
    return [
        Sample(
            id=scenario.id,
            input=scenario.public_opening,
            target="native_hard_gates_passed",
            metadata=scenario.evaluator_metadata(),
        )
        for scenario in load_scenarios()
    ]


def _contract_samples() -> list[Sample]:
    return [
        Sample(
            id=scenario.id,
            input=scenario.public_opening,
            target="contract_valid",
            metadata=scenario.evaluator_metadata(),
        )
        for scenario in load_scenarios()
    ]


@solver
def validate_contract():
    """Exercise Inspect task loading without a model or Galen API call."""

    async def solve(state: TaskState, generate: Generate) -> TaskState:
        del generate
        state.store.set("contract_valid", True)
        state.output = ModelOutput(model="galen-contract", completion="contract_valid")
        state.completed = True
        return state

    return solve


@solver
def run_native_galen():
    """Run one Galen CaseSpec and attach its immutable JSONL record to Inspect."""

    async def solve(state: TaskState, generate: Generate) -> TaskState:
        del generate
        binary = _native_eval_binary()
        if not binary.is_file():
            raise RuntimeError(
                f"Galen evaluator not found at {binary}. "
                "Run: cargo build -p galen --bin eval"
            )
        case_id = str((state.metadata or {}).get("galen_case_id", ""))
        if not case_id:
            raise RuntimeError("scenario is missing galen_case_id")
        model_alias = os.environ.get("GALEN_EVAL_MODEL")
        with tempfile.TemporaryDirectory(prefix="galen-inspect-") as directory:
            output = Path(directory) / "record.jsonl"
            command = [
                str(binary),
                "run",
                "--case",
                case_id,
                "--cases",
                str(CASES_DIR),
                "--repeat",
                "1",
                "--output",
                str(output),
            ]
            if model_alias:
                command.extend(["--model", model_alias])
            result = await subprocess(
                command,
                cwd=REPO_ROOT / "rust",
                timeout=420,
                output_limit=2_000_000,
            )
            if not result.success:
                raise RuntimeError(
                    f"native Galen eval failed ({result.returncode}): {result.stderr[-4000:]}"
                )
            records = [
                json.loads(line)
                for line in output.read_text(encoding="utf-8").splitlines()
                if line.strip()
            ]
            if len(records) != 1:
                raise RuntimeError(f"expected one native record, received {len(records)}")
            record = records[0]
        state.store.set("galen_record", record)
        scenario = next(item for item in load_scenarios() if item.id == state.sample_id)
        state.store.set("simulated_user_trace", simulate_user_trace(scenario, record))
        state.output = ModelOutput(
            model=f"galen/{record.get('model', 'unknown')}",
            completion=str(record.get("final_response", "")),
            metadata={
                "run_id": record.get("run_id"),
                "case_id": record.get("case_id"),
            },
        )
        state.completed = True
        return state

    return solve


def _record(state: TaskState) -> dict[str, Any]:
    value = state.store.get("galen_record")
    return value if isinstance(value, dict) else {}


@scorer(metrics=[accuracy(), stderr()])
def contract_score():
    async def score(state: TaskState, target: Target) -> Score:
        del target
        valid = state.store.get("contract_valid") is True
        return Score(
            value=CORRECT if valid else INCORRECT,
            explanation="scenario contract loaded with private/public separation",
        )

    return score


@scorer(metrics=[accuracy(), stderr()])
def native_hard_gates():
    async def score(state: TaskState, target: Target) -> Score:
        del target
        record = _record(state)
        passed = record.get("hard_gates_passed") is True
        failed = [
            item.get("name", "unknown")
            for item in record.get("assertions", [])
            if not item.get("pass", False)
        ]
        return Score(
            value=CORRECT if passed else INCORRECT,
            explanation="native Galen hard gates passed"
            if passed
            else f"failed native gates: {', '.join(failed)}",
            metadata={
                "quality_score": record.get("quality_score"),
                "failed_assertions": failed,
            },
        )

    return score


@scorer(metrics=[accuracy(), stderr()])
def delivery_proof():
    async def score(state: TaskState, target: Target) -> Score:
        del target
        record = _record(state)
        artifacts = record.get("artifacts", {})
        required = int(artifacts.get("required", 0) or 0)
        valid = int(artifacts.get("valid", 0) or 0)
        previewable = int(artifacts.get("previewable", 0) or 0)
        passed = required == 0 or (valid >= required and previewable >= required)
        return Score(
            value=CORRECT if passed else INCORRECT,
            explanation=(
                f"artifact proof required={required}, valid={valid}, previewable={previewable}"
            ),
            metadata={"files": artifacts.get("files", [])},
        )

    return score


@scorer(metrics=[accuracy(), stderr()])
def memory_retention():
    async def score(state: TaskState, target: Target) -> Score:
        del target
        record = _record(state)
        context = record.get("context", {})
        required = int(context.get("required_facts", 0) or 0)
        retained = int(context.get("retained_facts", 0) or 0)
        passed = required == 0 or retained == required
        return Score(
            value=CORRECT if passed else INCORRECT,
            explanation=f"retained {retained}/{required} required facts",
        )

    return score


@scorer(metrics=[accuracy(), stderr()])
def response_speed():
    async def score(state: TaskState, target: Target) -> Score:
        del target
        record = _record(state)
        latency = record.get("latency", {})
        ttfr = latency.get("ttfr_ms")
        total = int(latency.get("total_ms", 0) or 0)
        metadata = state.metadata or {}
        max_ttfr = int(metadata.get("max_ttfr_ms", 0) or 0)
        max_total = int(metadata.get("max_total_ms", 0) or 0)
        ttfr_ok = ttfr is not None and int(ttfr) <= max_ttfr
        total_ok = total <= max_total
        passed = ttfr_ok and total_ok
        return Score(
            value=CORRECT if passed else INCORRECT,
            explanation=(
                f"TTFR={ttfr}ms/{max_ttfr}ms, total={total}ms/{max_total}ms"
            ),
            metadata={"ttfr_ms": ttfr, "total_ms": total},
        )

    return score


@scorer(metrics=[accuracy(), stderr()])
def dynamic_user_outcome():
    async def score(state: TaskState, target: Target) -> Score:
        del target
        trace = state.store.get("simulated_user_trace")
        trace = trace if isinstance(trace, list) else []
        passed = trace_passed(trace)
        return Score(
            value=CORRECT if passed else INCORRECT,
            explanation=f"simulated user terminal intent={trace[-1].get('intent') if trace else 'missing'}",
            metadata={"trace": trace},
        )

    return score


@task
def galen_contracts() -> Task:
    """Zero-model validation of scenario loading and Inspect registration."""
    return Task(
        dataset=_contract_samples(),
        solver=validate_contract(),
        scorer=contract_score(),
        name="galen-contracts",
        version="1.0.0",
    )


@task
def galen_foundation() -> Task:
    """Execute Galen's native foundation cases under Inspect orchestration."""
    return Task(
        dataset=_samples(),
        solver=run_native_galen(),
        scorer=[
            native_hard_gates(),
            delivery_proof(),
            memory_retention(),
            response_speed(),
            dynamic_user_outcome(),
        ],
        name="galen-foundation",
        version="1.0.0",
        time_limit=450,
        fail_on_error=True,
        metadata={
            "agent_under_test": "galen-rust-loop",
            "user_simulation": "tau-style-contract-v1",
            "memory_protocol": "letta-style-v1",
        },
    )
