"""Run one or more paired cards through the locally isolated agent CLIs.

This is a deliberately small adapter for the first comparison pass.  It keeps
the same cards/fixtures as ``external_runner.py`` and records raw CLI output
without pretending that every framework exposes the same event schema.
"""

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
DEFAULT_MODEL = "deepseek/deepseek-v4-flash"


def _execution_contract(backend: str, prompt: str) -> str:
    """Keep external agents inside the disposable fixture.

    The paired cards are workspace tasks.  Without this small adapter-level
    contract, a fresh session can waste its budget looking for a prior chat or
    files under the user's profile, which is neither useful work nor a fair
    comparison of the task itself.
    """
    if backend == "galen":
        return prompt
    return prompt + (
        "\n\n执行约束：只在当前工作目录及其 output/ 下完成本任务；不要访问、搜索或猜测用户目录、"
        "历史会话、桌面、Temp 或工作目录之外的文件。直接按题面写出要求的产物，完成必要的最少读写后停止；"
        "不要因为当前会话没有历史记录而拒绝执行。"
    )


def _powershell_command(backend: str, args: list[str]) -> list[str]:
    # Passing the arguments as a native argv list avoids interpolating prompts
    # into a shell command.  PowerShell's remaining-arguments binder forwards
    # them to the isolated wrapper and then to the selected CLI.
    return [
        "powershell", "-NoProfile", "-ExecutionPolicy", "Bypass",
        "-File", str(WRAPPER), "-Backend", backend,
        "-CommandArgsJson", json.dumps(args, ensure_ascii=False, separators=(",", ":")),
    ]


def _command_args(backend: str, prompt: str, workspace: Path, last_message: Path) -> list[str]:
    if backend == "codex":
        return [
            # The isolated external baseline must not spend turns syncing the
            # desktop plugin marketplace.  This is infrastructure noise, not
            # task work, and it previously caused the first Codex turns to
            # fail before the model could touch the fixture.
            "--disable", "plugins",
            "exec", "--json", "--ephemeral", "--skip-git-repo-check",
            "-C", str(workspace), "-s", "danger-full-access",
            "-c", 'approval_policy="never"', "-o", str(last_message), "-",
        ]
    if backend == "claude":
        return [
            "-p", prompt, "--output-format", "stream-json",
            "--verbose", "--no-session-persistence", "--dangerously-skip-permissions",
            # 0.20 USD routinely stopped immediately after a successful Write
            # call.  A small, fixed headroom keeps the baseline comparable
            # while allowing the required artifact-confirmation turn.
            "--model", "deepseek-v4-flash", "--max-budget-usd", "0.35",
        ]
    if backend == "opencode":
        return [
            "run", "--format", "json", "--pure", "--model", DEFAULT_MODEL,
            "--agent", "build", "--variant", "low", "--dir", str(workspace),
            prompt,
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
            input=(effective_prompt.encode("utf-8") if backend == "codex" else None),
            timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired as exc:
        elapsed = int((time.perf_counter() - started) * 1000)
        # On Windows a CLI can spawn a child runtime that outlives the parent
        # process.  Kill the process tree so a timed-out card cannot block the
        # rest of a large repeat matrix.
        subprocess.run(
            ["taskkill", "/PID", str(process.pid), "/T", "/F"],
            capture_output=True,
        )
        tail_out, tail_err = process.communicate()
        stdout = tail_out or exc.stdout or b""
        stderr = tail_err or exc.stderr or b""
        raw_file.write_text(
            _redact(stdout.decode("utf-8", errors="replace") +
                    ("\n" + stderr.decode("utf-8", errors="replace") if stderr else ""))
            + f"\n[adapter] {backend} subprocess timed out after {timeout_seconds} seconds.\n",
            encoding="utf-8",
        )
        return f"External {backend} timed out after {timeout_seconds} seconds.", elapsed, 124
    elapsed = int((time.perf_counter() - started) * 1000)
    stdout = stdout.decode("utf-8", errors="replace")
    stderr = stderr.decode("utf-8", errors="replace")
    raw = stdout + ("\n" + stderr if stderr else "")
    raw_file.write_text(_redact(raw), encoding="utf-8")
    if last_message.exists():
        response = last_message.read_text(encoding="utf-8", errors="replace")
    else:
        response = stdout
    # Claude/OpenCode stream formats differ; retain their complete output as
    # the final response so file/fact assertions remain comparable.
    return _redact(response), elapsed, process.returncode


def _config_hash(backend: str) -> str:
    config_dir = ROOT / "evals" / "external-runtimes" / "config"
    if backend == "codex":
        path = config_dir / "codex" / "config.toml"
    elif backend == "claude":
        path = config_dir / "claude" / "deepseek-env.ps1"
    else:
        path = config_dir / "opencode" / "opencode.json"
    return _hash_file(path)[:16] if path.is_file() else "external"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--backend", choices=["codex", "claude", "opencode"], required=True)
    parser.add_argument("--case", required=True, help="case id, comma-separated ids, or all")
    parser.add_argument("--repeat", type=int, default=1)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--model-label", default=None)
    parser.add_argument("--start-index", type=int, default=1,
                        help="starting repeat index; useful for resuming a timed-out card")
    args = parser.parse_args()

    case_ids = _case_ids() if args.case.lower() == "all" else [
        item.strip() for item in args.case.split(",") if item.strip()
    ]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    raw_dir = args.output.parent / "raw-events"
    raw_dir.mkdir(exist_ok=True)
    model_label = args.model_label or f"{args.backend}+deepseek-v4-flash"
    config_hash = _config_hash(args.backend)

    with args.output.open("w", encoding="utf-8") as destination:
        for case_id in case_ids:
            case = _case(case_id)
            for offset in range(args.repeat):
                index = args.start_index + offset
                with tempfile.TemporaryDirectory(prefix=f"galen-{args.backend}-{case_id}-") as directory:
                    workspace = Path(directory)
                    _copy_fixture(case, workspace)
                    last_message = workspace / ".codex-last-message.md"
                    raw_file = raw_dir / f"{args.backend}-{case_id}-{index}.jsonl"
                    response, elapsed, returncode = _run_cli(
                        args.backend, case["prompt"], workspace, last_message,
                        raw_file, int(case.get("timeout_seconds", 300)),
                    )
                    raw_lines = raw_file.read_text(encoding="utf-8", errors="ignore").splitlines()
                    record = _record(
                        case_id, case, response, workspace, raw_lines, elapsed,
                        returncode, index, model_label, config_hash,
                    )
                    # Add adapter provenance while retaining RunRecord v1 shape.
                    record["adapter"] = {
                        "backend": args.backend,
                        "cli_wrapper": str(WRAPPER),
                        "raw_events": str(raw_file),
                        "event_schema": "codex-json-or-raw-cli",
                    }
                    destination.write(json.dumps(record, ensure_ascii=False) + "\n")
                    destination.flush()
                    print(
                        f"{args.backend} {case_id} run={index} "
                        f"pass={record['hard_gates_passed']} total_ms={elapsed}"
                    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
