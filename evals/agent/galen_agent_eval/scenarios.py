"""tau-style scenario contracts with strict private/public separation."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any


SCENARIO_PATH = Path(__file__).resolve().parent.parent / "scenarios" / "foundation.json"


@dataclass(frozen=True)
class Scenario:
    id: str
    galen_case_id: str
    persona: str
    public_opening: str
    private_goal: str
    hidden_facts: tuple[str, ...]
    required_final_state: tuple[str, ...]
    max_turns: int
    max_ttfr_ms: int
    max_total_ms: int

    @classmethod
    def from_dict(cls, value: dict[str, Any]) -> "Scenario":
        return cls(
            id=str(value["id"]),
            galen_case_id=str(value["galen_case_id"]),
            persona=str(value["persona"]),
            public_opening=str(value["public_opening"]),
            private_goal=str(value["private_goal"]),
            hidden_facts=tuple(str(item) for item in value.get("hidden_facts", [])),
            required_final_state=tuple(
                str(item) for item in value.get("required_final_state", [])
            ),
            max_turns=int(value["max_turns"]),
            max_ttfr_ms=int(value["max_ttfr_ms"]),
            max_total_ms=int(value["max_total_ms"]),
        )

    def public_metadata(self) -> dict[str, Any]:
        """Metadata safe to expose to the agent-under-test."""
        return {
            "scenario_id": self.id,
            "persona": self.persona,
            "max_turns": self.max_turns,
            "max_ttfr_ms": self.max_ttfr_ms,
            "max_total_ms": self.max_total_ms,
        }

    def evaluator_metadata(self) -> dict[str, Any]:
        """Gold state kept in Inspect's evaluator channel, never in agent input."""
        return {
            **self.public_metadata(),
            "galen_case_id": self.galen_case_id,
            "private_goal": self.private_goal,
            "hidden_facts": list(self.hidden_facts),
            "required_final_state": list(self.required_final_state),
        }


def load_scenarios(path: Path = SCENARIO_PATH) -> list[Scenario]:
    raw = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(raw, list):
        raise ValueError("scenario file must contain a JSON array")
    scenarios = [Scenario.from_dict(item) for item in raw]
    validate_scenarios(scenarios)
    return scenarios


def validate_scenarios(scenarios: list[Scenario]) -> None:
    ids = [scenario.id for scenario in scenarios]
    if len(ids) != len(set(ids)):
        raise ValueError("scenario ids must be unique")
    for scenario in scenarios:
        if not scenario.id or not scenario.galen_case_id:
            raise ValueError("scenario and Galen case ids are required")
        if scenario.max_turns < 1:
            raise ValueError(f"{scenario.id}: max_turns must be positive")
        if scenario.max_ttfr_ms < 1 or scenario.max_total_ms < scenario.max_ttfr_ms:
            raise ValueError(f"{scenario.id}: latency thresholds are invalid")
        if not scenario.public_opening.strip() or not scenario.private_goal.strip():
            raise ValueError(f"{scenario.id}: public opening and private goal are required")
        leaked = [fact for fact in scenario.hidden_facts if fact and fact in scenario.public_opening]
        if leaked:
            raise ValueError(f"{scenario.id}: hidden facts leaked into public opening: {leaked}")
        if not scenario.required_final_state:
            raise ValueError(f"{scenario.id}: at least one final-state requirement is required")
