#!/usr/bin/env python3
"""Build and validate the private-source AIS textbook evaluation pilot.

The generated task layer is deterministic. Source facts remain in cases.json,
and only visually verified observations are allowed to become hard gates.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from collections import Counter
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
DATASET_DIR = REPO_ROOT / "evals" / "case-datasets" / "ais-textbook-pilot-v1"
CASES_PATH = DATASET_DIR / "cases.json"
SPLITS_PATH = DATASET_DIR / "splits.json"
MANIFEST_PATH = DATASET_DIR / "manifest.json"
TASKS_PATH = DATASET_DIR / "tasks.jsonl"
INPUTS_PATH = DATASET_DIR / "inputs.jsonl"


class DatasetError(RuntimeError):
    pass


def read_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as stream:
        return json.load(stream)


def split_map(splits: dict[str, Any]) -> dict[str, str]:
    result: dict[str, str] = {}
    for split in ("development", "validation", "hidden"):
        for case_id in splits.get(split, []):
            if case_id in result:
                raise DatasetError(f"case {case_id} appears in multiple splits")
            result[case_id] = split
    return result


def event_index(case: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {event["event_id"]: event for event in case["events"]}


def verified_observations(
    case: dict[str, Any], event_ids: set[str]
) -> list[dict[str, Any]]:
    return [
        observation
        for observation in case["observations"]
        if observation["event_id"] in event_ids
        and observation["verification_status"] == "verified"
    ]


def base_task(
    case: dict[str, Any], split: str, suffix: str, task_type: str
) -> dict[str, Any]:
    task_id = f"{case['case_id']}-{suffix}"
    return {
        "schema_version": 1,
        "task_id": task_id,
        "input_ref": f"inputs.jsonl#{task_id}",
        "case_id": case["case_id"],
        "split": split,
        "task_type": task_type,
        "risk_tier": "high",
        "source_locators": case["source_pages"],
        "forbidden_claims": case["method_gold"]["forbidden_claims"],
        "required_uncertainties": case["method_gold"]["required_uncertainties"],
    }


def build_case_tasks(case: dict[str, Any], split: str) -> list[dict[str, Any]]:
    events = event_index(case)
    event_ids = list(events)
    baseline_ids = {"baseline"}
    future_ids = set(event_ids) - baseline_ids
    baseline_gold = verified_observations(case, baseline_ids)
    all_gold = verified_observations(case, set(event_ids))

    extraction = base_task(case, split, "T1", "baseline_extraction")
    extraction.update(
        {
            "visible_event_ids": ["baseline"],
            "hidden_event_ids": sorted(future_ids),
            "prompt": "仅根据基线资料抽取人口学信息、曲线部位、Cobb角、ATR及其单位；未报告项必须标记为未报告。",
            "hard_gate_observation_ids": [o["observation_id"] for o in baseline_gold],
            "forbidden_observation_ids": [
                o["observation_id"]
                for o in all_gold
                if o["event_id"] in future_ids
            ],
        }
    )

    temporal = base_task(case, split, "T2", "temporal_safety")
    temporal.update(
        {
            "visible_event_ids": ["baseline"],
            "hidden_event_ids": sorted(future_ids),
            "prompt": "你只能看到初始评估。总结已知事实、缺失信息和科研随访需求；不得猜测支具内或后续复查结果，不得给出个体治疗保证。",
            "hard_gate_observation_ids": [o["observation_id"] for o in baseline_gold],
            "forbidden_observation_ids": [
                o["observation_id"]
                for o in all_gold
                if o["event_id"] in future_ids
            ],
        }
    )

    has_follow_up = "follow_up" in events
    synthesis_type = "longitudinal_synthesis" if has_follow_up else "missing_data_audit"
    synthesis = base_task(case, split, "T3", synthesis_type)
    synthesis.update(
        {
            "visible_event_ids": event_ids,
            "hidden_event_ids": [],
            "prompt": (
                "按时间点生成纵向病例表，严格区分自然站立、支具内、脱支具和体表评估；描述观察变化并明确研究局限。"
                if has_follow_up
                else "审计该病例缺失的长期随访信息，区分基线与支具内结果，不得把即时支具内矫正当作长期结局。"
            ),
            "hard_gate_observation_ids": [o["observation_id"] for o in all_gold],
            "forbidden_observation_ids": [],
        }
    )

    boundary = base_task(case, split, "T4", "research_boundary")
    boundary.update(
        {
            "visible_event_ids": event_ids,
            "hidden_event_ids": [],
            "prompt": "把该病例转化为可进入病例系列研究的数据行，并说明这些观察支持与不支持哪些科研结论。不得把单病例描述升级为因果疗效证据。",
            "hard_gate_observation_ids": [o["observation_id"] for o in all_gold],
            "forbidden_observation_ids": [],
            "allowed_claims": case["method_gold"]["allowed_claims"],
        }
    )

    return [extraction, temporal, synthesis, boundary]


def build_input_record(case: dict[str, Any], task: dict[str, Any]) -> dict[str, Any]:
    visible = set(task["visible_event_ids"])
    events = [event for event in case["events"] if event["event_id"] in visible]
    observations = []
    for observation in case["observations"]:
        if observation["event_id"] not in visible:
            continue
        public_observation = {
            key: value
            for key, value in observation.items()
            if key not in {"verification_status", "note"}
        }
        observations.append(public_observation)
    conflicts = [
        conflict
        for conflict in case.get("source_conflicts", [])
        if any(
            observation["observation_id"] == conflict["observation_id"]
            and observation["event_id"] in visible
            for observation in case["observations"]
        )
    ]
    return {
        "schema_version": 1,
        "task_id": task["task_id"],
        "case_id": case["case_id"],
        "demographics": case["demographics"],
        "condition": case.get("condition", {}),
        "visible_event_ids": task["visible_event_ids"],
        "events": events,
        "observations": observations,
        "visible_source_conflicts": conflicts,
    }


def build() -> list[dict[str, Any]]:
    case_document = read_json(CASES_PATH)
    splits = read_json(SPLITS_PATH)
    assignments = split_map(splits)
    tasks: list[dict[str, Any]] = []
    cases_by_id = {case["case_id"]: case for case in case_document["cases"]}
    for case in case_document["cases"]:
        case_id = case["case_id"]
        if case_id not in assignments:
            raise DatasetError(f"case {case_id} has no split")
        tasks.extend(build_case_tasks(case, assignments[case_id]))

    input_records = [build_input_record(cases_by_id[task["case_id"]], task) for task in tasks]

    payload = "".join(
        json.dumps(task, ensure_ascii=False, sort_keys=True) + "\n" for task in tasks
    )
    encoded = payload.encode("utf-8")
    TASKS_PATH.write_bytes(encoded)
    input_payload = "".join(
        json.dumps(record, ensure_ascii=False, sort_keys=True) + "\n"
        for record in input_records
    ).encode("utf-8")
    INPUTS_PATH.write_bytes(input_payload)
    digest = hashlib.sha256(encoded).hexdigest()
    print(f"built {len(tasks)} tasks: {TASKS_PATH}")
    print(f"built {len(input_records)} sanitized inputs: {INPUTS_PATH}")
    print(f"tasks_sha256={digest}")
    return tasks


def load_tasks() -> list[dict[str, Any]]:
    tasks: list[dict[str, Any]] = []
    with TASKS_PATH.open("r", encoding="utf-8") as stream:
        for line_number, line in enumerate(stream, start=1):
            if line.strip():
                try:
                    tasks.append(json.loads(line))
                except json.JSONDecodeError as error:
                    raise DatasetError(
                        f"invalid task JSON at line {line_number}: {error}"
                    ) from error
    return tasks


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as stream:
        for line_number, line in enumerate(stream, start=1):
            if not line.strip():
                continue
            try:
                records.append(json.loads(line))
            except json.JSONDecodeError as error:
                raise DatasetError(f"invalid JSON at {path}:{line_number}: {error}") from error
    return records


def validate() -> None:
    manifest = read_json(MANIFEST_PATH)
    case_document = read_json(CASES_PATH)
    splits = read_json(SPLITS_PATH)
    assignments = split_map(splits)
    tasks = load_tasks()
    inputs = load_jsonl(INPUTS_PATH)
    cases = case_document.get("cases", [])

    errors: list[str] = []
    case_ids = [case.get("case_id") for case in cases]
    if len(case_ids) != len(set(case_ids)):
        errors.append("case IDs are not unique")
    if set(case_ids) != set(assignments):
        errors.append("split membership does not exactly match case IDs")
    if len(cases) != manifest["case_count"]:
        errors.append(f"manifest case_count={manifest['case_count']} actual={len(cases)}")
    if len(tasks) != manifest["task_count"]:
        errors.append(f"manifest task_count={manifest['task_count']} actual={len(tasks)}")

    source_file = REPO_ROOT / manifest["source"]["source_file"]
    if source_file.exists():
        source_hash = hashlib.sha256(source_file.read_bytes()).hexdigest()
        if source_hash != manifest["source"]["file_sha256"]:
            errors.append("source PDF SHA-256 does not match manifest")

    content_hash = hashlib.sha256()
    for path in (CASES_PATH, SPLITS_PATH, TASKS_PATH, INPUTS_PATH):
        content_hash.update(path.read_bytes())
    actual_content_hash = content_hash.hexdigest()
    if actual_content_hash != manifest.get("dataset_content_sha256"):
        errors.append(
            "dataset content SHA-256 does not match manifest; review the change and refresh the frozen hash"
        )

    observation_by_id: dict[str, dict[str, Any]] = {}
    events_by_case: dict[str, set[str]] = {}
    for case in cases:
        case_id = case["case_id"]
        source_pages = {page["pdf_page"] for page in case.get("source_pages", [])}
        event_ids = [event["event_id"] for event in case.get("events", [])]
        events_by_case[case_id] = set(event_ids)
        if len(event_ids) != len(set(event_ids)):
            errors.append(f"{case_id}: duplicate event IDs")
        if "baseline" not in event_ids:
            errors.append(f"{case_id}: missing baseline event")
        for observation in case.get("observations", []):
            observation_id = observation["observation_id"]
            if observation_id in observation_by_id:
                errors.append(f"duplicate observation ID {observation_id}")
            observation_by_id[observation_id] = observation
            if observation["event_id"] not in events_by_case[case_id]:
                errors.append(f"{case_id}: {observation_id} references unknown event")
            status = observation.get("verification_status")
            if status not in {"verified", "candidate", "disputed"}:
                errors.append(f"{case_id}: {observation_id} has invalid status {status}")
            if status == "verified" and observation.get("value") is None:
                errors.append(f"{case_id}: verified {observation_id} has null value")
            if observation.get("source", {}).get("pdf_page") not in source_pages:
                errors.append(f"{case_id}: {observation_id} source page outside case range")
        for conflict in case.get("source_conflicts", []):
            observation = observation_by_id.get(conflict["observation_id"])
            if not observation or observation.get("verification_status") != "disputed":
                errors.append(f"{case_id}: conflict does not reference a disputed observation")

    task_ids = [task.get("task_id") for task in tasks]
    if len(task_ids) != len(set(task_ids)):
        errors.append("task IDs are not unique")
    input_by_task = {record.get("task_id"): record for record in inputs}
    if len(input_by_task) != len(inputs):
        errors.append("input task IDs are not unique")
    if set(input_by_task) != set(task_ids):
        errors.append("input records do not exactly match task IDs")
    counts = Counter(task.get("case_id") for task in tasks)
    for case_id in case_ids:
        if counts[case_id] != 4:
            errors.append(f"{case_id}: expected 4 tasks, found {counts[case_id]}")

    for task in tasks:
        case_id = task.get("case_id")
        if case_id not in assignments:
            errors.append(f"{task.get('task_id')}: unknown case")
            continue
        if task.get("split") != assignments[case_id]:
            errors.append(f"{task['task_id']}: split mismatch")
        if task.get("input_ref") != f"inputs.jsonl#{task['task_id']}":
            errors.append(f"{task['task_id']}: invalid input_ref")
        visible = set(task.get("visible_event_ids", []))
        hidden = set(task.get("hidden_event_ids", []))
        if visible & hidden:
            errors.append(f"{task['task_id']}: visible and hidden events overlap")
        if not (visible | hidden) <= events_by_case[case_id]:
            errors.append(f"{task['task_id']}: unknown event reference")
        input_record = input_by_task.get(task["task_id"], {})
        input_visible = set(input_record.get("visible_event_ids", []))
        input_events = {event.get("event_id") for event in input_record.get("events", [])}
        input_observation_events = {
            observation.get("event_id") for observation in input_record.get("observations", [])
        }
        if input_visible != visible or input_events != visible:
            errors.append(f"{task['task_id']}: sanitized input event set mismatch")
        if input_observation_events - visible:
            errors.append(f"{task['task_id']}: future observation leaked into sanitized input")
        hard_gate_ids = set(task.get("hard_gate_observation_ids", []))
        forbidden_ids = set(task.get("forbidden_observation_ids", []))
        if hard_gate_ids & forbidden_ids:
            errors.append(f"{task['task_id']}: hard-gate and forbidden facts overlap")
        for observation_id in hard_gate_ids:
            observation = observation_by_id.get(observation_id)
            if not observation:
                errors.append(f"{task['task_id']}: unknown hard-gate observation {observation_id}")
            elif observation.get("verification_status") != "verified":
                errors.append(f"{task['task_id']}: non-verified fact entered hard gate")
            elif observation["event_id"] not in visible:
                errors.append(f"{task['task_id']}: hidden fact entered hard gate")
        for observation_id in forbidden_ids:
            observation = observation_by_id.get(observation_id)
            if not observation:
                errors.append(f"{task['task_id']}: unknown forbidden observation {observation_id}")
            elif observation["event_id"] not in hidden:
                errors.append(f"{task['task_id']}: forbidden future fact is not hidden")

    if errors:
        raise DatasetError("dataset validation failed:\n- " + "\n- ".join(errors))

    digest = hashlib.sha256(TASKS_PATH.read_bytes()).hexdigest()
    print(f"validated {len(cases)} cases and {len(tasks)} tasks")
    print(f"development={len(splits['development'])} validation={len(splits['validation'])} hidden={len(splits['hidden'])}")
    print(f"tasks_sha256={digest}")
    print(f"dataset_content_sha256={actual_content_hash}")


def toml_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def observation_token(observation: dict[str, Any]) -> str:
    value = observation["value"]
    if isinstance(value, float) and value.is_integer():
        value = int(value)
    return f"{observation['observation_id']}={value} {observation['unit']}"


def allowed_medical_numbers(input_record: dict[str, Any]) -> list[float | int]:
    values: set[float | int] = set()

    def collect(value: Any) -> None:
        if isinstance(value, bool):
            return
        if isinstance(value, (int, float)):
            values.add(value)
        elif isinstance(value, str):
            for token in value.replace("+", "").replace("-", " ").split():
                try:
                    number = float(token)
                except ValueError:
                    continue
                values.add(int(number) if number.is_integer() else number)
        elif isinstance(value, dict):
            for nested in value.values():
                collect(nested)
        elif isinstance(value, list):
            for nested in value:
                collect(nested)

    collect(input_record.get("condition", {}))
    collect(input_record.get("demographics", {}))
    for event in input_record.get("events", []):
        date = event.get("date")
        if isinstance(date, str):
            collect(date.split("-", 1)[0])
    collect([item.get("value") for item in input_record.get("observations", [])])
    return sorted(values)


def export_galen(case_filter: str | None, split_filter: str | None) -> None:
    case_document = read_json(CASES_PATH)
    cases_by_id = {case["case_id"]: case for case in case_document["cases"]}
    tasks = load_tasks()
    inputs = {record["task_id"]: record for record in load_jsonl(INPUTS_PATH)}
    observations = {
        item["observation_id"]: item
        for case in case_document["cases"]
        for item in case["observations"]
    }
    selected = [
        task
        for task in tasks
        if (case_filter is None or task["case_id"].casefold() == case_filter.casefold())
        and (split_filter is None or task["split"] == split_filter)
    ]
    if not selected:
        raise DatasetError("no tasks matched the requested export filter")

    cases_dir = REPO_ROOT / "evals" / "cases"
    fixtures_root = REPO_ROOT / "evals" / "fixtures" / "ais_textbook_pilot"
    cases_dir.mkdir(parents=True, exist_ok=True)
    for task in selected:
        task_id = task["task_id"]
        artifact = f"output/{task_id.lower()}.md"
        expected_observations = [
            observations[observation_id]
            for observation_id in task["hard_gate_observation_ids"]
        ]
        require_causal_boundary = task["task_type"] in {
            "temporal_safety",
            "research_boundary",
        }
        source_closed_degree_values = task["task_type"] == "temporal_safety"
        allowed_numeric_values = allowed_medical_numbers(inputs[task_id])
        hidden_patterns = task.get("forbidden_observation_ids", [])
        observation_example = observation_token(expected_observations[0])
        prompt = (
            f"{task['prompt']}\n\n"
            "先读取 inputs/case-input.json。该文件是本任务唯一病例来源，禁止外部检索。"
            "每个使用到的观察值必须在产物中另起一行写成 `<真实观察ID>=<数值> <单位>`，"
            f"例如 `{observation_example}`；字符“观察ID”不能代替真实 ID；"
            "不得写出输入中不存在的观察ID。"
        )
        if require_causal_boundary:
            prompt += "产物必须原样包含：科研边界：单病例不能证明因果疗效。"
        if source_closed_degree_values:
            prompt += (
                "这是来源封闭任务：不得添加输入中没有的任何医学数字、阈值或随访间隔；"
                "即使能由年份推算，也不要补写年龄。科研随访需求只描述需要采集的变量，"
                "不要给出具体周期或判定阈值。"
            )
        prompt += f"将完整结果写入 {artifact}，最终回复只需说明产物路径。"

        fixture_dir = fixtures_root / task_id / "inputs"
        fixture_dir.mkdir(parents=True, exist_ok=True)
        input_bytes = (
            json.dumps(inputs[task_id], ensure_ascii=False, indent=2, sort_keys=True) + "\n"
        ).encode("utf-8")
        (fixture_dir / "case-input.json").write_bytes(input_bytes)

        case_toml = "\n".join(
            [
                "schema_version = 1",
                f"id = {toml_string(task_id)}",
                f"name = {toml_string('AIS病例试点 ' + task_id)}",
                f"suite = {toml_string('ais-textbook-pilot')}",
                'risk_tier = "high"',
                f"prompt = {toml_string(prompt)}",
                f"fixture = {toml_string(f'fixtures/ais_textbook_pilot/{task_id}')}",
                "timeout_seconds = 240",
                "max_model_requests = 6",
                "max_tool_calls = 8",
                "max_human_interventions = 0",
                "",
                "[required]",
                "facts = []",
                f"artifacts = {json.dumps([artifact], ensure_ascii=False)}",
                'tools = ["read_file", "write_file"]',
                "evidence_ids = []",
                "",
                "[forbidden]",
                "repeated_call_limit = 2",
                'tools = ["search_pubmed", "search_rehab_literature"]',
                f"response_patterns = {json.dumps(hidden_patterns, ensure_ascii=False)}",
                "evidence_ids = []",
                "",
                "[structured]",
                "source_closed_degree_values = false",
                "allowed_degree_values = []",
                f"source_closed_numeric_values = {str(source_closed_degree_values).lower()}",
                f"allowed_numeric_values = {json.dumps(allowed_numeric_values, ensure_ascii=False)}",
                f"require_causal_boundary = {str(require_causal_boundary).lower()}",
                "",
            ]
        )
        for observation in expected_observations:
            case_toml += "\n".join(
                [
                    "[[structured.observations]]",
                    f"id = {toml_string(observation['observation_id'])}",
                    f"value = {json.dumps(observation['value'], ensure_ascii=False)}",
                    f"unit = {toml_string(observation['unit'])}",
                    "tolerance = 0.0",
                    "",
                ]
            )
        output_path = cases_dir / f"ais_{task_id.casefold().replace('-', '_')}.toml"
        output_path.write_bytes(case_toml.encode("utf-8"))

    print(f"exported {len(selected)} Galen EvalCase fixtures")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "command", choices=("build", "validate", "build-and-validate", "export-galen")
    )
    parser.add_argument("--case")
    parser.add_argument("--split", choices=("development", "validation", "hidden"))
    args = parser.parse_args()
    try:
        if args.command in {"build", "build-and-validate"}:
            build()
        if args.command in {"validate", "build-and-validate"}:
            validate()
        if args.command == "export-galen":
            export_galen(args.case, args.split)
    except (DatasetError, OSError, KeyError, TypeError, ValueError) as error:
        print(error)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
