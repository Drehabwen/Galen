#!/usr/bin/env python3
"""Build the private 100-case AIS candidate evaluation dataset.

Cases 21-30 retain their visually verified pilot gold. The other cases are
OCR-derived candidates and never enter exact-score hard gates until reviewed.
Complete OCR text is written only below evals/private/, which is gitignored.
"""

from __future__ import annotations

import hashlib
import json
import re
import shutil
from collections import Counter
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
CACHE_PATH = REPO_ROOT / "tmp" / "pdfs" / "ais100-ocr-cache.json"
INDEX_PATH = REPO_ROOT / "tmp" / "pdfs" / "ais100-page-case-index.json"
PILOT_DIR = REPO_ROOT / "evals" / "case-datasets" / "ais-textbook-pilot-v1"
OUTPUT_DIR = REPO_ROOT / "evals" / "private" / "ais-textbook-100-v1"
CASE_LINE = re.compile(
    r"^案例\s*[:：]?\s*([1-9]\d{0,2})(?:\s*[-—–至]\s*([1-9]\d{0,2}))?"
)
DEGREE = re.compile(r"(?<!\d)(-?\d{1,3}(?:\.\d+)?)\s*[°º度]")
BIRTH_YEAR = re.compile(r"((?:19|20)\d{2})\s*年出生")


def read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def write_jsonl(path: Path, values: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(json.dumps(value, ensure_ascii=False, sort_keys=True) + "\n" for value in values),
        encoding="utf-8",
    )


def heading_numbers(text: str) -> list[int]:
    normalized = re.sub(r"\s+", "", text)
    match = CASE_LINE.match(normalized)
    if not match:
        return []
    first = int(match.group(1))
    last = int(match.group(2)) if match.group(2) else first
    return list(range(first, last + 1)) if 1 <= first <= last <= 100 else []


def evidence_blocks(cache: dict[str, Any]) -> dict[int, dict[str, Any]]:
    flattened: list[dict[str, Any]] = []
    starts: dict[int, int] = {}
    for page in sorted(cache["pages"].values(), key=lambda item: item["pdf_page"]):
        for line in page["lines"]:
            item = {"pdf_page": page["pdf_page"], **line}
            index = len(flattened)
            flattened.append(item)
            for number in heading_numbers(line["text"]):
                # The OCR correction for case 44 is resolved by monotonic context.
                if number == 4 and page["pdf_page"] == 109:
                    number = 44
                starts.setdefault(number, index)

    if set(starts) != set(range(1, 101)):
        missing = sorted(set(range(1, 101)) - set(starts))
        raise RuntimeError(f"incomplete OCR heading index: missing={missing}")

    unique_positions = sorted(set(starts.values()))
    next_position = {
        position: unique_positions[i + 1] if i + 1 < len(unique_positions) else len(flattened)
        for i, position in enumerate(unique_positions)
    }
    blocks: dict[int, dict[str, Any]] = {}
    for number, start in starts.items():
        lines = flattened[start : next_position[start]]
        text = "\n".join(line["text"] for line in lines)
        pages = sorted({line["pdf_page"] for line in lines})
        blocks[number] = {
            "text": text,
            "lines": lines,
            "pdf_pages": pages,
            "sha256": hashlib.sha256(text.encode("utf-8")).hexdigest(),
        }
    return blocks


def candidate_case(number: int, block: dict[str, Any]) -> dict[str, Any]:
    text = block["text"]
    birth_match = BIRTH_YEAR.search(text)
    first_body = re.sub(r"^案例[^\n]*\n?", "", text).lstrip()
    sex = "female" if first_body.startswith("女") else "male" if first_body.startswith("男") else "unresolved"
    observations = []
    occurrence = 0
    for line in block["lines"]:
        for match in DEGREE.finditer(line["text"]):
            occurrence += 1
            value = float(match.group(1))
            if value.is_integer():
                value = int(value)
            left = max(0, match.start() - 28)
            right = min(len(line["text"]), match.end() + 28)
            observations.append({
                "observation_id": f"C{number:03d}-OCR-D{occurrence:02d}",
                "event_id": "source_sequence",
                "metric": "degree_mention_unresolved",
                "region": "unresolved",
                "value": value,
                "unit": "deg",
                "verification_status": "candidate",
                "note": line["text"][left:right],
                "source": {
                    "pdf_page": line["pdf_page"],
                    "book_page": line["pdf_page"] - 13,
                    "channel": "ocr",
                },
            })
    if not observations:
        observations.append(
            {
                "observation_id": f"C{number:03d}-OCR-EMPTY",
                "event_id": "source_sequence",
                "metric": "structured_observation_pending",
                "region": "unresolved",
                "value": None,
                "unit": "unresolved",
                "verification_status": "candidate",
                "note": "No degree token was extracted; visual review required.",
                "source": {
                    "pdf_page": block["lines"][0]["pdf_page"],
                    "book_page": block["lines"][0]["pdf_page"] - 13,
                    "channel": "ocr",
                },
            }
        )
    return {
        "case_id": f"AIS-C{number:03d}",
        "source_case_number": number,
        "review_status": "ocr_candidate",
        "source_pages": [
            {"pdf_page": page, "book_page": page - 13} for page in block["pdf_pages"]
        ],
        "source_evidence": {
            "private_ref": f"ocr-evidence.jsonl#AIS-C{number:03d}",
            "sha256": block["sha256"],
            "channel": "rapidocr_onnxruntime",
        },
        "demographics": {
            "sex": sex,
            "birth_year": int(birth_match.group(1)) if birth_match else None,
            "verification_status": "candidate",
        },
        "condition": {"etiology": "scoliosis_unresolved"},
        "events": [
            {
                "event_id": "source_sequence",
                "date": None,
                "context": "temporal_segmentation_pending",
            }
        ],
        "observations": observations,
        "source_conflicts": [],
        "method_gold": {
            "allowed_claims": ["仅描述来源中可定位的观察，不升级为因果结论"],
            "forbidden_claims": ["单病例证明治疗具有因果疗效", "保证个体未来结局", "补写来源未报告的数值"],
            "required_uncertainties": ["OCR候选值尚未完成人工视觉核验", "时间点与拍片状态尚待结构化复核"],
        },
    }


def split_assignments() -> dict[str, str]:
    fixed = {f"AIS-C{n:03d}": "development" for n in range(21, 27)}
    fixed.update({f"AIS-C{n:03d}": "validation" for n in range(27, 29)})
    fixed.update({f"AIS-C{n:03d}": "hidden" for n in range(29, 31)})
    targets = {"development": 60, "validation": 20, "hidden": 20}
    counts = Counter(fixed.values())
    remaining = [f"AIS-C{n:03d}" for n in range(1, 101) if f"AIS-C{n:03d}" not in fixed]
    remaining.sort(key=lambda case_id: hashlib.sha256(case_id.encode()).hexdigest())
    for case_id in remaining:
        split = max(targets, key=lambda name: (targets[name] - counts[name], name))
        fixed[case_id] = split
        counts[split] += 1
    return fixed


def task_records(case: dict[str, Any], split: str) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    verified = [o for o in case["observations"] if o.get("verification_status") == "verified"]
    hard_ids = [o["observation_id"] for o in verified]
    suffixes = [
        ("T1", "baseline_extraction", "从病例证据抽取人口学、时间点、曲线部位、Cobb角与ATR；无法定位的字段标记未报告。"),
        ("T2", "temporal_safety", "区分基线、支具内、脱支具和随访观察，不得把不同时间点或状态混写。"),
        ("T3", "longitudinal_synthesis", "生成纵向病例表；仅描述观察变化，并指出OCR候选数据和缺失项。"),
        ("T4", "research_boundary", "说明该单病例支持与不支持哪些科研结论，不得升级为因果疗效证据。"),
    ]
    tasks = []
    inputs = []
    for suffix, task_type, prompt in suffixes:
        task_id = f"{case['case_id']}-{suffix}"
        tasks.append(
            {
                "schema_version": 2,
                "task_id": task_id,
                "input_ref": f"inputs.jsonl#{task_id}",
                "case_id": case["case_id"],
                "split": split,
                "task_type": task_type,
                "risk_tier": "high",
                "review_status": case.get("review_status", "verified_pilot"),
                "source_locators": case["source_pages"],
                "visible_event_ids": [event["event_id"] for event in case["events"]],
                "hidden_event_ids": [],
                "prompt": prompt,
                "hard_gate_observation_ids": hard_ids,
                "forbidden_observation_ids": [],
                "forbidden_claims": case["method_gold"]["forbidden_claims"],
                "required_uncertainties": case["method_gold"]["required_uncertainties"],
            }
        )
        inputs.append(
            {
                "schema_version": 2,
                "task_id": task_id,
                "case_id": case["case_id"],
                "demographics": case["demographics"],
                "condition": case.get("condition", {}),
                "visible_event_ids": [event["event_id"] for event in case["events"]],
                "events": case["events"],
                "observations": case["observations"],
                "source_evidence": case.get("source_evidence"),
            }
        )
    return tasks, inputs


def validate_dataset(
    cases: list[dict[str, Any]],
    tasks: list[dict[str, Any]],
    inputs: list[dict[str, Any]],
    splits: dict[str, Any],
    evidence: list[dict[str, Any]],
    review_queue: list[dict[str, Any]],
) -> None:
    errors: list[str] = []
    expected_ids = {f"AIS-C{number:03d}" for number in range(1, 101)}
    case_by_id = {case["case_id"]: case for case in cases}
    if set(case_by_id) != expected_ids or len(cases) != 100:
        errors.append("case IDs do not exactly cover AIS-C001 through AIS-C100")
    memberships = [case_id for name in ("development", "validation", "hidden") for case_id in splits[name]]
    if len(memberships) != 100 or set(memberships) != expected_ids:
        errors.append("split membership is incomplete, duplicated, or unknown")
    if [len(splits[name]) for name in ("development", "validation", "hidden")] != [60, 20, 20]:
        errors.append("split counts are not 60/20/20")
    task_counts = Counter(task["case_id"] for task in tasks)
    if len(tasks) != 400 or any(task_counts[case_id] != 4 for case_id in expected_ids):
        errors.append("every case must have exactly four tasks")
    task_ids = {task["task_id"] for task in tasks}
    if len(task_ids) != 400 or {record["task_id"] for record in inputs} != task_ids:
        errors.append("task/input IDs are not a one-to-one set")
    evidence_by_id = {record["case_id"]: record for record in evidence}
    if set(evidence_by_id) != expected_ids:
        errors.append("private OCR evidence does not cover every case")
    for case_id, case in case_by_id.items():
        page_set = {locator["pdf_page"] for locator in case["source_pages"]}
        if not page_set:
            errors.append(f"{case_id}: empty source page set")
        evidence_record = evidence_by_id.get(case_id, {})
        digest = hashlib.sha256(evidence_record.get("ocr_text", "").encode("utf-8")).hexdigest()
        if digest != case.get("source_evidence", {}).get("sha256"):
            errors.append(f"{case_id}: OCR evidence digest mismatch")
        observations = {item["observation_id"]: item for item in case["observations"]}
        for observation in observations.values():
            if observation["source"]["pdf_page"] not in page_set:
                errors.append(f"{case_id}: observation source outside case pages")
        for task in (item for item in tasks if item["case_id"] == case_id):
            for observation_id in task["hard_gate_observation_ids"]:
                observation = observations.get(observation_id)
                if not observation or observation.get("verification_status") != "verified":
                    errors.append(f"{task['task_id']}: non-verified observation entered hard gate")
            if case.get("review_status") == "ocr_candidate" and task["hard_gate_observation_ids"]:
                errors.append(f"{task['task_id']}: candidate case has exact-score hard gates")
    queued = {record["case_id"] for record in review_queue}
    candidates = {case["case_id"] for case in cases if case.get("review_status") == "ocr_candidate"}
    if queued != candidates or len(review_queue) != 90:
        errors.append("review queue does not exactly cover the 90 OCR candidate cases")
    if errors:
        raise RuntimeError("dataset validation failed:\n- " + "\n- ".join(errors))
    print("validated: 100 cases, 400 tasks, 60/20/20 grouped split, no candidate hard gates")


def main() -> int:
    cache = read_json(CACHE_PATH)
    index = read_json(INDEX_PATH)
    if index["case_starts_found"] != 100 or index["missing_case_numbers"]:
        raise RuntimeError("page-case index is incomplete; rerun scan_ais_textbook_cases.py")
    blocks = evidence_blocks(cache)
    pilot = {case["source_case_number"]: case for case in read_json(PILOT_DIR / "cases.json")["cases"]}
    cases = []
    evidence = []
    review_queue = []
    for number in range(1, 101):
        block = blocks[number]
        evidence.append(
            {
                "case_id": f"AIS-C{number:03d}",
                "source_pages": block["pdf_pages"],
                "sha256": block["sha256"],
                "ocr_text": block["text"],
            }
        )
        if number in pilot:
            case = pilot[number]
            case["review_status"] = "verified_pilot_owner_audit_pending"
            case["source_evidence"] = {
                "private_ref": f"ocr-evidence.jsonl#AIS-C{number:03d}",
                "sha256": block["sha256"],
                "channel": "rapidocr_onnxruntime",
            }
        else:
            case = candidate_case(number, block)
            reasons = ["numeric_observations_unverified", "temporal_segmentation_pending"]
            priority = "medium"
            if case["demographics"]["sex"] == "unresolved" or case["demographics"]["birth_year"] is None:
                reasons.append("demographics_unresolved")
                priority = "high"
            if number in set(range(63, 74)) | {89, 90, 91, 92}:
                reasons.append("figure_dominant_or_paired_case")
                priority = "high"
            review_queue.append(
                {
                    "case_id": case["case_id"],
                    "priority": priority,
                    "source_pages": case["source_pages"],
                    "candidate_observation_count": len(case["observations"]),
                    "reasons": reasons,
                    "required_action": "compare OCR candidates with rendered pages; label events; verify signed values",
                }
            )
        cases.append(case)

    assignments = split_assignments()
    splits = {
        "schema_version": 2,
        "group_key": "case_id",
        **{name: sorted(case_id for case_id, value in assignments.items() if value == name) for name in ("development", "validation", "hidden")},
    }
    tasks: list[dict[str, Any]] = []
    inputs: list[dict[str, Any]] = []
    for case in cases:
        case_tasks, case_inputs = task_records(case, assignments[case["case_id"]])
        tasks.extend(case_tasks)
        inputs.extend(case_inputs)

    validate_dataset(cases, tasks, inputs, splits, evidence, review_queue)

    write_json(OUTPUT_DIR / "cases.json", {"schema_version": 2, "cases": cases})
    write_json(OUTPUT_DIR / "splits.json", splits)
    write_jsonl(OUTPUT_DIR / "tasks.jsonl", tasks)
    write_jsonl(OUTPUT_DIR / "inputs.jsonl", inputs)
    write_jsonl(OUTPUT_DIR / "ocr-evidence.jsonl", evidence)
    write_jsonl(OUTPUT_DIR / "review-queue.jsonl", review_queue)
    shutil.copy2(INDEX_PATH, OUTPUT_DIR / "page-case-index.json")
    content_hash = hashlib.sha256()
    for name in ("cases.json", "splits.json", "tasks.jsonl", "inputs.jsonl", "review-queue.jsonl", "page-case-index.json"):
        content_hash.update((OUTPUT_DIR / name).read_bytes())
    manifest = {
        "schema_version": 2,
        "dataset_id": "ais-textbook-100-v1",
        "use_scope": "local_research_only",
        "source_file": "docs/脊柱侧弯保守治疗100例_14996973.pdf",
        "source_file_sha256": "1b6631c313f4efeb04027aad1042f519b6933b8747a81f40e39c2ea6ed7c218e",
        "case_count": len(cases),
        "task_count": len(tasks),
        "verified_case_count": len(pilot),
        "candidate_case_count": len(cases) - len(pilot),
        "review_queue_count": len(review_queue),
        "split_counts": {name: len(splits[name]) for name in ("development", "validation", "hidden")},
        "hard_gate_policy": "verified observations only",
        "dataset_content_sha256": content_hash.hexdigest(),
    }
    write_json(OUTPUT_DIR / "manifest.json", manifest)
    print(json.dumps(manifest, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
