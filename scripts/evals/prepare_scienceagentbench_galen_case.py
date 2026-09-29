#!/usr/bin/env python3
"""Prepare a Galen EvalCase using the exact contract-aware public prompt."""

from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path

try:
    from .scienceagentbench_generation_smoke import _prompt, _task
except ImportError:  # direct script execution
    from scienceagentbench_generation_smoke import _prompt, _task

try:
    from .scienceagentbench_contract import DEFAULT_CONTRACT
except ImportError:  # direct script execution
    from scienceagentbench_contract import DEFAULT_CONTRACT


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--instance", type=int, default=1)
    args = parser.parse_args()
    root = args.output.resolve()
    if root.exists():
        raise FileExistsError(f"refusing to overwrite Galen case directory: {root}")
    cases = root / "cases"
    fixture = root / "fixtures/clintox"
    cases.mkdir(parents=True)
    fixture.mkdir(parents=True)
    shutil.copy2(DEFAULT_CONTRACT, fixture / "task-contract.json")
    prompt = _prompt(_task(args.instance))
    if "'''" in prompt:
        raise ValueError("prompt cannot be represented as a TOML literal multiline string")
    case = f'''schema_version = 1
id = "SABCLINTOX01"
name = "ScienceAgentBench ClinTox contract-aware generation"
suite = "scienceagentbench-contract-competition"
risk_tier = "standard"
prompt = ''' + "'''\n" + prompt + "\n'''\n" + '''fixture = "fixtures/clintox"
timeout_seconds = 600
max_model_requests = 12
max_tool_calls = 16
max_human_interventions = 0

[required]
artifacts = ["program.py"]
tools = ["write_file"]

[forbidden]
repeated_call_limit = 3
artifact_patterns = ["D:\\\\Users\\\\DORAT", "benchmark/eval_programs/gold_results"]
response_patterns = ["cannot complete", "unable to complete"]
'''
    (cases / "sab_clintox01.toml").write_text(case, encoding="utf-8")
    (root / "prompt.txt").write_text(prompt, encoding="utf-8")
    (root / "metadata.json").write_text(
        json.dumps(
            {
                "schema_version": 1,
                "instance_id": args.instance,
                "contract": str(DEFAULT_CONTRACT),
                "prompt_bytes": len(prompt.encode("utf-8")),
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    print(root)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
