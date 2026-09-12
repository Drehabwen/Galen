"""Run an external CLI agent on Galen paired cases and emit RunRecord JSONL.

The adapter is intentionally conservative: it measures observable files and
events only, and never invents token counts or tool traces that the backend did
not expose.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[3]
CASES = ROOT / "evals" / "cases" / "paired"


def _redact(value: str) -> str:
    """Remove credential-shaped values before persisting external traces."""
    text = str(value)
    text = re.sub(r"sk-[A-Za-z0-9_-]{8,}", "[REDACTED_SECRET]", text)
    text = re.sub(
        r"(?i)(api[_-]?key|bearer[_-]?token|authorization)\s*=\s*([\"']?)[^\s\"']+\2",
        r"\1=[REDACTED_SECRET]",
        text,
    )
    return text


def _case(case_id: str) -> dict[str, Any]:
    # Keep the adapter runnable with the Python 3.10 bundled on Windows.  The
    # paired cards use a deliberately small TOML subset, so parsing only the
    # fields needed for an external run avoids adding a dependency to Galen.
    path = next(item for item in CASES.glob("*.toml")
                if re.search(r'^id\s*=\s*"([^"]+)"', item.read_text(encoding="utf-8"), re.M)
                and re.search(r'^id\s*=\s*"([^"]+)"', item.read_text(encoding="utf-8"), re.M).group(1).lower() == case_id.lower())
    text = path.read_text(encoding="utf-8")
    def value(key: str, default: str = "") -> str:
        match = re.search(rf'^{key}\s*=\s*"([^"]*)"', text, re.M)
        return match.group(1) if match else default
    def array(key: str) -> list[str]:
        match = re.search(rf'^{key}\s*=\s*\[([^\]]*)\]', text, re.M)
        return re.findall(r'"([^"]*)"', match.group(1)) if match else []
    def integer(key: str, default: int) -> int:
        match = re.search(rf'^{key}\s*=\s*(\d+)', text, re.M)
        return int(match.group(1)) if match else default
    sequence: list[dict[str, Any]] = []
    sequence_match = re.search(r'^tool_sequence\s*=\s*\[(.*?)\]', text, re.M | re.S)
    if sequence_match:
        for item in re.finditer(r'\{([^}]*)\}', sequence_match.group(1), re.S):
            fields = item.group(1)
            tool_match = re.search(r'tool\s*=\s*"([^"]+)"', fields)
            input_match = re.search(r'input_contains\s*=\s*"([^"]*)"', fields)
            error_match = re.search(r'is_error\s*=\s*(true|false)', fields)
            if tool_match:
                sequence.append({
                    "tool": tool_match.group(1),
                    "input_contains": input_match.group(1) if input_match else "",
                    "is_error": (error_match.group(1) == "true") if error_match else None,
                })
    return {
        "id": value("id"), "name": value("name"), "prompt": value("prompt"),
        "fixture": value("fixture") or None,
        "max_model_requests": integer("max_model_requests", 10**9),
        "max_tool_calls": integer("max_tool_calls", 10**9),
        "timeout_seconds": integer("timeout_seconds", 300),
        "forbidden": {
            "response_patterns": array("response_patterns"),
            "artifact_patterns": array("artifact_patterns"),
            "repeated_call_limit": integer("repeated_call_limit", 10**9),
        },
        "required": {
            "facts": array("facts"), "artifacts": array("artifacts"),
            "tools": array("tools"), "tool_sequence": sequence,
        },
    }


def _case_ids() -> list[str]:
    ids: list[str] = []
    for path in sorted(CASES.glob("*.toml")):
        match = re.search(r'^id\s*=\s*"([^"]+)"', path.read_text(encoding="utf-8"), re.M)
        if match:
            ids.append(match.group(1))
    return ids


def _copy_fixture(case: dict[str, Any], workspace: Path) -> None:
    fixture = case.get("fixture")
    if not fixture:
        (workspace / "output").mkdir(parents=True, exist_ok=True)
        return
    source = (CASES.parent / fixture).resolve()
    shutil.copytree(source, workspace, dirs_exist_ok=True)
    (workspace / "output").mkdir(parents=True, exist_ok=True)


def _hash_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _semantic_tool(item_type: str, item: dict[str, Any]) -> str:
    if item_type == "file_change":
        return "write_file"
    if item_type == "web_search":
        return "search_evidence"
    if item_type == "mcp_tool_call":
        return str(item.get("tool", "mcp_tool"))
    if item_type == "command_execution":
        command = str(item.get("command", ""))
        if re.search(r"Get-Content|Get-Item|Get-ChildItem|Import-Csv|Select-String|\brg\b|\bcat\b|\btype\b", command, re.I):
            return "read_file"
        if re.search(r"Set-Content|Add-Content|Out-File|New-Item.*-ItemType\s+File", command, re.I):
            return "write_file"
        return "shell"
    return item_type


def _event_tool_traces(lines: list[str]) -> tuple[list[dict[str, Any]], list[str], int, int]:
    pending: dict[str, dict[str, Any]] = {}
    traces: list[dict[str, Any]] = []
    names: list[str] = []
    calls = 0
    errors = 0
    for line in lines:
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        # OpenCode emits one JSON object per tool execution with the tool name
        # under ``part.tool`` and the arguments/result under ``part.state``.
        # Normalize it into the same conservative semantic trace used for
        # Codex, without guessing calls that are not present in the stream.
        if isinstance(event, dict) and event.get("type") == "tool_use":
            part = event.get("part", {})
            if isinstance(part, dict) and part.get("type") == "tool":
                state = part.get("state", {}) or {}
                tool_name = str(part.get("tool", "external_tool"))
                semantic = "write_file" if tool_name.lower() == "write" else (
                    "read_file" if tool_name.lower() in {"read", "glob", "grep"} else (
                        "shell" if tool_name.lower() in {"bash", "shell", "powershell"} else tool_name
                    )
                )
                calls += 1
                if semantic not in names:
                    names.append(semantic)
                input_value = state.get("input", {}) if isinstance(state, dict) else {}
                output_value = state.get("output", "") if isinstance(state, dict) else ""
                command = input_value.get("command", "") if isinstance(input_value, dict) else ""
                semantic = _semantic_tool(
                    "command_execution", {"command": command}
                ) if command else semantic
                if semantic not in names:
                    names.append(semantic)
                failed = bool(
                    isinstance(state, dict)
                    and (state.get("status") in {"failed", "error"}
                         or (state.get("metadata", {}) or {}).get("exit", 0) not in (None, 0))
                )
                errors += int(failed)
                traces.append({
                    "turn": 1,
                    "tool": semantic,
                    "input": _redact(json.dumps(input_value, ensure_ascii=False)),
                    "output": _redact(str(output_value)),
                    "is_error": failed,
                })
                continue
        # Claude Code stream-json places tool calls in assistant message
        # content blocks.  The result event is not required to count a call;
        # failures are captured when a later tool_result is visible.
        if isinstance(event, dict) and event.get("type") == "assistant":
            message = event.get("message", {}) or {}
            content = message.get("content", []) if isinstance(message, dict) else []
            if isinstance(content, list):
                for block in content:
                    if not isinstance(block, dict) or block.get("type") != "tool_use":
                        continue
                    tool_name = str(block.get("name", "external_tool"))
                    semantic = "write_file" if tool_name.lower() in {"write", "edit"} else (
                        "read_file" if tool_name.lower() in {"read", "glob", "grep"} else (
                            "shell" if tool_name.lower() in {"bash", "powershell"} else tool_name
                        )
                    )
                    calls += 1
                    if semantic not in names:
                        names.append(semantic)
                    traces.append({
                        "turn": 1,
                        "tool": semantic,
                        "input": _redact(json.dumps(block.get("input", {}), ensure_ascii=False)),
                        "output": "",
                        "is_error": False,
                    })
        item = event.get("item", {}) if isinstance(event, dict) else {}
        event_type = item.get("type") if isinstance(item, dict) else None
        observable_types = {"command_execution", "file_change", "mcp_tool_call", "web_search"}
        if event_type in observable_types and event.get("type") == "item.started":
            calls += 1
            semantic = _semantic_tool(event_type, item)
            if semantic not in names:
                names.append(semantic)
            pending[str(item.get("id", f"item-{calls}"))] = {"type": event_type, "item": item, "semantic": semantic}
        if event_type in observable_types and event.get("type") == "item.completed":
            failed = item.get("status") in {"failed", "error"} or item.get("exit_code", 0) not in (None, 0)
            if failed:
                errors += 1
            key = str(item.get("id", ""))
            started = pending.pop(key, {})
            semantic = str(started.get("semantic") or _semantic_tool(event_type, item))
            input_value = item.get("command") or item.get("query") or item.get("action") or item.get("changes") or ""
            output_value = item.get("aggregated_output") or item.get("text") or item.get("action") or ""
            traces.append({
                "turn": 1,
                "tool": semantic,
                "input": _redact(json.dumps(input_value, ensure_ascii=False)),
                "output": _redact(str(output_value)),
                "is_error": bool(failed),
            })
        text = json.dumps(event, ensure_ascii=False)
        for name in ("read_file", "write_file", "search_pubmed", "search_evidence", "shell"):
            if name in text and name not in names:
                names.append(name)
    return traces, names, calls, errors


def _fact_present(fact: str, response: str, file_text: str) -> bool:
    """Match a required fact with small, predeclared presentation aliases.

    The cards assert scientific content, not one language's typography.  In
    particular, a model may render ``12 名`` as ``12名``, ``12 participants``
    or ``sample size: 12``.  Keep the aliases narrow and deterministic; all
    other facts remain literal matches so a fluent but incorrect answer cannot
    pass by accident.
    """
    haystack = f"{response}\n{file_text}"
    if fact == "12 名":
        return bool(re.search(
            r"(?<!\d)12\s*(?:名|人|participants?|subjects?|受试者)\b"
            r"|(?:sample\s*size|n)\s*[:=]?\s*12\b",
            haystack, re.IGNORECASE,
        ))
    if fact == "来源":
        return bool(re.search(
            r"来源|source|doi|pmid|pubmed|https?://|参考文献|citation",
            haystack, re.IGNORECASE,
        ))
    return fact in haystack


def _run_codex(
    prompt: str,
    workspace: Path,
    last_message: Path,
    raw_events: Path,
    codex_home: Path | None = None,
    timeout_seconds: int = 300,
) -> tuple[str, int, int]:
    command = [
        "codex", "exec", "--json", "--ephemeral", "--skip-git-repo-check",
        "-C", str(workspace), "-s", "danger-full-access", "-c", 'approval_policy="never"',
        "-o", str(last_message), "-",
    ]
    started = time.perf_counter()
    environment = os.environ.copy()
    if codex_home is not None:
        environment["CODEX_HOME"] = str(codex_home)
    try:
        result = subprocess.run(
            command, input=prompt.encode("utf-8"), text=False, capture_output=True,
            timeout=timeout_seconds, cwd=workspace, env=environment,
        )
    except subprocess.TimeoutExpired as exc:
        elapsed = int((time.perf_counter() - started) * 1000)
        stdout = (exc.stdout or b"").decode("utf-8", errors="replace")
        stderr = (exc.stderr or b"").decode("utf-8", errors="replace")
        raw_events.write_text(
            _redact(stdout + ("\n" + stderr if stderr else ""))
            + f"\n[adapter] Codex subprocess timed out after {timeout_seconds} seconds.\n",
            encoding="utf-8",
        )
        return f"External Codex timed out after {timeout_seconds} seconds.", elapsed, 124
    elapsed = int((time.perf_counter() - started) * 1000)
    stdout = result.stdout.decode("utf-8", errors="replace")
    stderr = result.stderr.decode("utf-8", errors="replace")
    raw_events.write_text(_redact(stdout + ("\n" + stderr if stderr else "")), encoding="utf-8")
    response = last_message.read_text(encoding="utf-8", errors="replace") if last_message.exists() else stdout
    response = _redact(response)
    return response, elapsed, result.returncode


def _record(case_id: str, case: dict[str, Any], response: str, workspace: Path,
            raw_lines: list[str], elapsed: int, returncode: int, run_index: int,
            model_label: str, config_hash: str) -> dict[str, Any]:
    required = case.get("required", {})
    facts = required.get("facts", [])
    forbidden = case.get("forbidden", {})
    artifact_paths = required.get("artifacts", [])
    file_text = "\n".join(
        p.read_text(encoding="utf-8", errors="ignore")
        for p in workspace.rglob("*") if p.is_file()
    )
    retained = sum(_fact_present(str(fact), response, file_text) for fact in facts)
    files = []
    valid = 0
    previewable = 0
    for relative in artifact_paths:
        target = workspace / relative
        if target.is_file() and target.stat().st_size > 0:
            files.append(relative)
            valid += 1
            previewable += 1 if target.suffix.lower() in {".md", ".txt", ".json", ".csv", ".pdf"} else 0
    forbidden_hits = sum(str(pattern) in response for pattern in forbidden.get("response_patterns", []))
    traces, names, tool_calls, tool_errors = _event_tool_traces(raw_lines)
    required_tools = required.get("tools", [])
    required_tool_pass = all(tool in names for tool in required_tools)
    tool_sequence = required.get("tool_sequence", [])
    sequence_pass = True
    if tool_sequence:
        sequence_pass = len(traces) == len(tool_sequence)
        if sequence_pass:
            for actual, wanted in zip(traces, tool_sequence):
                if actual["tool"] != wanted["tool"]:
                    sequence_pass = False
                    break
                if wanted.get("input_contains") and wanted["input_contains"] not in actual["input"]:
                    sequence_pass = False
                    break
                if wanted.get("is_error") is not None and actual["is_error"] != wanted["is_error"]:
                    sequence_pass = False
                    break
    budget_pass = tool_calls <= int(case.get("max_tool_calls", 10**9))
    model_request_pass = 1 <= int(case.get("max_model_requests", 10**9))
    max_repeat = 1
    if traces:
        counts: dict[tuple[str, str], int] = {}
        for trace in traces:
            key = (trace["tool"], trace["input"])
            counts[key] = counts.get(key, 0) + 1
        max_repeat = max(counts.values())
    repeat_pass = max_repeat <= int(case.get("forbidden", {}).get("repeated_call_limit", 10**9))
    artifact_forbidden_hits = sum(
        str(pattern) in "\n".join(
            p.read_text(encoding="utf-8", errors="ignore")
            for p in workspace.rglob("*") if p.is_file()
        ) for pattern in case.get("forbidden", {}).get("artifact_patterns", [])
    )
    passed = (
        returncode == 0 and retained == len(facts) and valid == len(artifact_paths)
        and forbidden_hits == 0 and required_tool_pass and sequence_pass
        and budget_pass and model_request_pass and repeat_pass and artifact_forbidden_hits == 0
    )
    assertion_total = 8 + len(facts) + len(artifact_paths) + len(required_tools) + len(tool_sequence)
    assertion_passes = (
        int(returncode == 0) + int(retained == len(facts)) + int(valid == len(artifact_paths))
        + int(forbidden_hits == 0) + int(required_tool_pass) + int(sequence_pass)
        + int(budget_pass and model_request_pass and repeat_pass and artifact_forbidden_hits == 0)
        + retained + valid + len(required_tools) * int(required_tool_pass)
        + len(tool_sequence) * int(sequence_pass)
    )
    return {
        "schema_version": 1, "run_id": f"external-{case_id}-{int(time.time() * 1000)}-{run_index}",
        "case_id": case_id, "commit": "external", "model": model_label,
        "config_hash": config_hash, "run_index": run_index, "started_at_ms": int(time.time() * 1000),
        "workspace": str(workspace), "final_response": response,
        "hard_gates_passed": passed, "quality_score": assertion_passes / assertion_total if assertion_total else 1.0,
        "dimensions": {"capability": 0.0, "repeatability": 0.0, "state_safety": 0.0,
        "delivery": 0.0, "efficiency": 0.0}, "latency": {"context_ms": 0, "mcp_ms": 0, "ttft_ms": None,
        "ttfr_ms": None, "total_ms": elapsed},
        "usage": {"input": 0, "output": 0, "cache_create": 0, "cache_read": 0},
        "model_requests": 1, "tools": {"calls": tool_calls, "errors": tool_errors, "max_repeat": max_repeat, "names": names},
        "tool_trace": traces, "context": {"compactions": 0, "required_facts": len(facts), "retained_facts": retained},
        "grounding": {"required_evidence": 0, "retrieved_required": 0, "cited_required": 0,
        "forbidden_hits": forbidden_hits + artifact_forbidden_hits, "local_search_calls": 0, "external_search_calls": names.count("search_evidence"),
        "retrieved_evidence_ids": []}, "artifacts": {"required": len(artifact_paths), "valid": valid,
        "previewable": previewable, "files": files}, "assertions": [],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--backend", choices=["codex"], required=True)
    parser.add_argument("--case", required=True)
    parser.add_argument("--repeat", type=int, default=1)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--codex-home", type=Path, default=None,
                        help="isolated CODEX_HOME used by the child Codex process")
    parser.add_argument("--model-label", default=None,
                        help="model/provider label recorded in RunRecord output")
    args = parser.parse_args()
    model_label = args.model_label or os.environ.get("GALEN_EXTERNAL_MODEL", "codex-cli")
    config_hash = "external"
    if args.codex_home is not None:
        config_path = args.codex_home / "config.toml"
        if config_path.is_file():
            config_hash = _hash_file(config_path)[:16]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    if args.case.lower() == "all":
        requested_cases = _case_ids()
    else:
        requested_cases = [item.strip() for item in args.case.split(",") if item.strip()]
    with args.output.open("w", encoding="utf-8") as destination:
        for case_id in requested_cases:
            case = _case(case_id)
            for index in range(1, args.repeat + 1):
                with tempfile.TemporaryDirectory(prefix=f"galen-external-{case_id}-") as directory:
                    workspace = Path(directory)
                    _copy_fixture(case, workspace)
                    last_message = workspace / ".codex-last-message.md"
                    raw_path = args.output.parent / "raw-events"
                    raw_path.mkdir(exist_ok=True)
                    raw_file = raw_path / f"{case_id}-{index}.jsonl"
                    response, elapsed, returncode = _run_codex(
                        case["prompt"], workspace, last_message, raw_file, args.codex_home,
                        int(case.get("timeout_seconds", 300))
                    )
                    lines = raw_file.read_text(encoding="utf-8", errors="ignore").splitlines()
                    record = _record(
                        case_id, case, response, workspace, lines, elapsed, returncode, index,
                        model_label, config_hash
                    )
                    destination.write(json.dumps(record, ensure_ascii=False) + "\n")
                    destination.flush()
                    print(f"{case_id} run={index} pass={record['hard_gates_passed']} total_ms={elapsed}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
