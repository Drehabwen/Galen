"""Deterministic, response-aware user policy for reproducible agent journeys."""

from __future__ import annotations

from dataclasses import asdict, dataclass
from typing import Any

from galen_agent_eval.scenarios import Scenario


@dataclass(frozen=True)
class UserAct:
    turn: int
    intent: str
    utterance: str
    terminal: bool


def simulate_user_trace(scenario: Scenario, record: dict[str, Any]) -> list[dict[str, Any]]:
    """Choose the next act from observable Galen outcomes, never evaluator gold facts."""

    acts = [UserAct(1, "open", scenario.public_opening, False)]
    assertions = {
        str(item.get("name", "")): bool(item.get("pass", False))
        for item in record.get("assertions", [])
        if isinstance(item, dict)
    }
    context = record.get("context", {}) if isinstance(record.get("context"), dict) else {}
    artifacts = record.get("artifacts", {}) if isinstance(record.get("artifacts"), dict) else {}
    latency = record.get("latency", {}) if isinstance(record.get("latency"), dict) else {}

    required_facts = int(context.get("required_facts", 0) or 0)
    retained_facts = int(context.get("retained_facts", 0) or 0)
    required_artifacts = int(artifacts.get("required", 0) or 0)
    valid_artifacts = int(artifacts.get("valid", 0) or 0)
    previewable = int(artifacts.get("previewable", 0) or 0)
    ttfr = latency.get("ttfr_ms")
    total = int(latency.get("total_ms", 0) or 0)

    if any(not passed for name, passed in assertions.items() if "tool" in name.lower()):
        acts.append(UserAct(2, "recover_tool", "不要只解释错误，换一条可行路径继续完成。", False))
    elif required_facts > retained_facts:
        acts.append(UserAct(2, "challenge_memory", "这些条件之前已经确认过，请从已有记录继续，不要让我重复。", False))
    elif required_artifacts > valid_artifacts:
        acts.append(UserAct(2, "demand_delivery", "我需要真实可检查的文件，不是完成说明。", False))
    elif required_artifacts > previewable:
        acts.append(UserAct(2, "demand_preview", "请直接在 Galen 里打开成果让我检查。", False))
    elif ttfr is None or int(ttfr) > scenario.max_ttfr_ms or total > scenario.max_total_ms:
        acts.append(UserAct(2, "challenge_latency", "响应太慢了，请先给可用结论，再补充细节。", False))
    else:
        acts.append(UserAct(2, "accept", "结果和交付证据都清楚，可以结束。", True))

    if not acts[-1].terminal and len(acts) < scenario.max_turns:
        acts.append(UserAct(len(acts) + 1, "stop_after_failure", "这次仍未达到可用状态，记录为失败并保留轨迹。", True))

    return [asdict(act) for act in acts[: scenario.max_turns]]


def trace_passed(trace: list[dict[str, Any]]) -> bool:
    return bool(trace) and trace[-1].get("intent") == "accept" and trace[-1].get("terminal") is True
