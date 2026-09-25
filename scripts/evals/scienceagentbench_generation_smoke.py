"""Run a non-scored ScienceAgentBench public-prompt generation smoke."""

from __future__ import annotations

import argparse
import ast
import json
import subprocess
import sys
import time
from pathlib import Path

import pyarrow.parquet as pq


ROOT = Path(__file__).resolve().parents[2]
DATASET = ROOT / "evals/public-benchmarks/data/ScienceAgentBench-default-verified-0000.parquet"
UPSTREAM_AGENT = ROOT / "evals/public-benchmarks/ScienceAgentBench/agent.py"
WRAPPER = ROOT / "evals/external-runtimes/with-deepseek.ps1"


def _official_prompt(name: str) -> str:
    tree = ast.parse(UPSTREAM_AGENT.read_text(encoding="utf-8"))
    for node in tree.body:
        if isinstance(node, ast.Assign):
            if any(isinstance(target, ast.Name) and target.id == name for target in node.targets):
                return ast.literal_eval(node.value)
    raise KeyError(f"Official prompt constant not found: {name}")


def _task(instance_id: int) -> dict[str, object]:
    for row in pq.read_table(DATASET).to_pylist():
        if int(row["instance_id"]) == instance_id:
            return row
    raise KeyError(f"Unknown instance_id: {instance_id}")


def _prompt(row: dict[str, object]) -> str:
    return "\n\n".join(
        [
            _official_prompt("SYSTEM_PROMPT"),
            "Here's the user request you need to work on:\n" + str(row["task_inst"]),
            "Domain knowledge:\n" + str(row["domain_knowledge"]),
            "Dataset directory structure:\n```\n" + str(row["dataset_folder_tree"]) + "\n```",
            "Dataset preview:\n" + str(row["dataset_preview"]),
            _official_prompt("FORMAT_PROMPT"),
            (
                "This is a public-prompt generation-only diagnostic: the full dataset is not present. "
                "Do not fabricate execution results and do not install dependencies. Write the complete "
                f"Python program to program.py. It must save its eventual task output to {row['output_fname']}. "
                "The filesystem artifact is authoritative. After writing it, reply with exactly DONE; "
                "do not repeat the program in the final response."
            ),
        ]
    )


def _command(backend: str, prompt: str, workspace: Path) -> tuple[list[str], bytes | None]:
    if backend == "codex":
        args = [
            "exec", "--json", "--ephemeral", "--skip-git-repo-check",
            "-C", str(workspace), "-s", "danger-full-access",
            "-c", 'approval_policy="never"', "-",
        ]
        stdin = prompt.encode("utf-8")
    else:
        args = [
            "-p", prompt, "--output-format", "stream-json", "--verbose",
            "--no-session-persistence", "--dangerously-skip-permissions",
            "--model", "deepseek-flash", "--max-budget-usd", "1.00",
        ]
        stdin = None
    command = [
        "powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(WRAPPER),
        "-Backend", backend, "-CommandArgsJson", json.dumps(args, ensure_ascii=False, separators=(",", ":")),
    ]
    return command, stdin


def _run(backend: str, row: dict[str, object], output: Path, timeout: int) -> dict[str, object]:
    workspace = output / backend / f"instance-{row['instance_id']}"
    workspace.mkdir(parents=True, exist_ok=False)
    prompt = _prompt(row)
    (workspace / "prompt.txt").write_text(prompt, encoding="utf-8")
    command, stdin = _command(backend, prompt, workspace)
    started = time.perf_counter()
    try:
        completed = subprocess.run(
            command,
            input=stdin,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=workspace,
            timeout=timeout,
            check=False,
        )
        returncode = completed.returncode
        stdout = completed.stdout.decode("utf-8", errors="replace")
        stderr = completed.stderr.decode("utf-8", errors="replace")
    except subprocess.TimeoutExpired as error:
        returncode = 124
        stdout = (error.stdout or b"").decode("utf-8", errors="replace")
        stderr = (error.stderr or b"").decode("utf-8", errors="replace") + "\nTIMEOUT"
    elapsed_ms = round((time.perf_counter() - started) * 1000)
    raw = stdout + ("\n" + stderr if stderr else "")
    (workspace / "raw-events.log").write_text(raw, encoding="utf-8")
    context_features = []
    if "\\.agents\\skills\\" in raw or "/.agents/skills/" in raw:
        context_features.append("host_skill_discovery")
    program = workspace / "program.py"
    return {
        "instance_id": row["instance_id"],
        "backend": backend,
        "model": "deepseek-flash",
        "returncode": returncode,
        "elapsed_ms": elapsed_ms,
        "program_created": program.is_file(),
        "program_bytes": program.stat().st_size if program.is_file() else 0,
        "context_features": context_features,
        "comparison_eligibility": {
            "native_agent": True,
            "controlled_core": "host_skill_discovery" not in context_features,
        },
        "official_score": None,
        "classification": "generation_only_diagnostic",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--instance", type=int, default=1)
    parser.add_argument("--backend", choices=("codex", "claude", "both"), default="both")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=300)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        raise FileExistsError(f"Refusing to overwrite run directory: {output}")
    output.mkdir(parents=True)
    row = _task(args.instance)
    backends = ("codex", "claude") if args.backend == "both" else (args.backend,)
    records = [_run(backend, row, output, args.timeout) for backend in backends]
    (output / "summary.json").write_text(
        json.dumps(records, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(records, ensure_ascii=False, indent=2))
    return 0 if all(record["returncode"] == 0 for record in records) else 1


if __name__ == "__main__":
    sys.exit(main())
