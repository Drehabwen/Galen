"""Run paired task cards through isolated external agent CLIs."""

from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
import time
from pathlib import Path

from external_runner import (
    ROOT,
    _case,
    _case_ids,
    _copy_fixture,
    _hash_file,
    _record,
    _redact,
)


WRAPPER = ROOT / "evals" / "external-runtimes" / "with-deepseek.ps1"
MODEL_ID = "deepseek-flash"


def _execution_contract(backend: str, prompt: str) -> str:
    """Keep an external agent inside its disposable task fixture."""
    if backend == "galen":
        return prompt
    return prompt + (
        "\n\n执行约束：只在当前工作目录及其 output/ 目录中完成任务；"
        "不要访问、搜索或猜测用户目录、历史会话、桌面、Temp 或工作目录之外的文件。"
        "直接按题面生成要求的产物，完成必要的最少读写后停止。"
        "不要因为当前会话没有历史记录而拒绝执行。"
    )


def _powershell_command(backend: str, args: list[str]) -> list[str]:
    return [
        "powershell",
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        str(WRAPPER),
        "-Backend",
        backend,
        "-CommandArgsJson",
        json.dumps(args, ensure_ascii=False, separators=(",", ":")),
    ]


def _command_args(
    backend: str, prompt: str, workspace: Path, last_message: Path
) -> list[str]:
    if backend == "codex":
        return [
            "exec",
            "--json",
            "--ephemeral",
            "--skip-git-repo-check",
            "-C",
            str(workspace),
            "-s",
            "danger-full-access",
            "-c",
            'approval_policy="never"',
            "-o",
            str(last_message),
            "-",
        ]
    if backend == "claude":
        return [
            "-p",
            prompt,
            "--output-format",
            "stream-json",
            "--verbose",
            "--no-session-persistence",
            "--dangerously-skip-permissions",
            "--model",
            MODEL_ID,
            "--max-budget-usd",
            "1.00",
        ]
    raise ValueError(f"unsupported backend: {backend}")


def _run_cli(
    backend: str,
    prompt: str,
    workspace: Path,
    last_message: Path,
    raw_file: Path,
    timeout_seconds: int,
) -> tuple[str, int, int]:
    effective_prompt = _execution_contract(backend, prompt)
    command = _powershell_command(
        backend, _command_args(backend, effective_prompt, workspace, last_message)
    )
    started = time.perf_counter()
    process = subprocess.Popen(
        command,
        stdin=subprocess.PIPE if backend == "codex" else None,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        cwd=workspace,
    )
    try:
        stdout, stderr = process.communicate(
            input=effective_prompt.encode("utf-8") if backend == "codex" else None,
            timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired as error:
        elapsed = int((time.perf_counter() - started) * 1000)
        subprocess.run(
            ["taskkill", "/PID", str(process.pid), "/T", "/F"],
            capture_output=True,
            check=False,
        )
        tail_out, tail_err = process.communicate()
        stdout = tail_out or error.stdout or b""
        stderr = tail_err or error.stderr or b""
        raw_file.write_text(
            _redact(
                stdout.decode("utf-8", errors="replace")
                + ("\n" + stderr.decode("utf-8", errors="replace") if stderr else "")
            )
            + f"\n[adapter] {backend} subprocess timed out after {timeout_seconds} seconds.\n",
            encoding="utf-8",
        )
        return f"External {backend} timed out after {timeout_seconds} seconds.", elapsed, 124

    elapsed = int((time.perf_counter() - started) * 1000)
    stdout_text = stdout.decode("utf-8", errors="replace")
    stderr_text = stderr.decode("utf-8", errors="replace")
    raw_file.write_text(
        _redact(stdout_text + ("\n" + stderr_text if stderr_text else "")),
        encoding="utf-8",
    )
    response = (
        last_message.read_text(encoding="utf-8", errors="replace")
        if last_message.exists()
        else stdout_text
    )
    return _redact(response), elapsed, process.returncode


def _config_hash(backend: str) -> str:
    del backend
    return _hash_file(WRAPPER)[:16] if WRAPPER.is_file() else "missing-wrapper"


def _context_features(raw: str) -> list[str]:
    features: list[str] = []
    if "\\.agents\\skills\\" in raw or "/.agents/skills/" in raw:
        features.append("host_skill_discovery")
    if "CODEX_THREAD_ID" in raw:
        features.append("host_thread_context")
    return features


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--backend", choices=["codex", "claude"], required=True)
    parser.add_argument("--case", required=True, help="case id, comma-separated ids, or all")
    parser.add_argument("--repeat", type=int, default=1)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--model-label")
    parser.add_argument("--start-index", type=int, default=1)
    args = parser.parse_args()

    case_ids = (
        _case_ids()
        if args.case.lower() == "all"
        else [item.strip() for item in args.case.split(",") if item.strip()]
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    raw_dir = args.output.parent / "raw-events"
    raw_dir.mkdir(exist_ok=True)
    model_label = args.model_label or f"{args.backend}+{MODEL_ID}"
    config_hash = _config_hash(args.backend)

    with args.output.open("w", encoding="utf-8") as destination:
        for case_id in case_ids:
            case = _case(case_id)
            for offset in range(args.repeat):
                index = args.start_index + offset
                with tempfile.TemporaryDirectory(
                    prefix=f"galen-{args.backend}-{case_id}-"
                ) as directory:
                    workspace = Path(directory)
                    _copy_fixture(case, workspace)
                    last_message = workspace / ".codex-last-message.md"
                    raw_file = raw_dir / f"{args.backend}-{case_id}-{index}.jsonl"
                    response, elapsed, returncode = _run_cli(
                        args.backend,
                        case["prompt"],
                        workspace,
                        last_message,
                        raw_file,
                        int(case.get("timeout_seconds", 300)),
                    )
                    raw_lines = raw_file.read_text(
                        encoding="utf-8", errors="ignore"
                    ).splitlines()
                    context_features = _context_features("\n".join(raw_lines))
                    record = _record(
                        case_id,
                        case,
                        response,
                        workspace,
                        raw_lines,
                        elapsed,
                        returncode,
                        index,
                        model_label,
                        config_hash,
                    )
                    record["adapter"] = {
                        "backend": args.backend,
                        "cli_wrapper": str(WRAPPER),
                        "raw_events": str(raw_file),
                        "event_schema": "codex-json-or-raw-cli",
                        "context_features": context_features,
                        "comparison_eligibility": {
                            "native_agent": True,
                            "controlled_core": "host_skill_discovery" not in context_features,
                        },
                    }
                    destination.write(json.dumps(record, ensure_ascii=False) + "\n")
                    destination.flush()
                    print(
                        f"{args.backend} {case_id} run={index} "
                        f"pass={record['hard_gates_passed']} total_ms={elapsed} "
                        "native_agent_eligible=True "
                        f"controlled_core_eligible={'host_skill_discovery' not in context_features}"
                    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
