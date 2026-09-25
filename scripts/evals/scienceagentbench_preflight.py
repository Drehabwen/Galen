"""Validate pinned inputs before running ScienceAgentBench."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_MANIFEST = ROOT / "evals" / "public-benchmarks" / "scienceagentbench-manifest.json"


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest().upper()


def _native_runtime(package_dir: Path, executable: str) -> Path | None:
    candidates = sorted(package_dir.rglob(executable))
    return candidates[0] if candidates else None


def _version(executable: Path | None) -> str | None:
    if executable is None:
        return None
    completed = subprocess.run(
        [str(executable), "--version"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=30,
        check=False,
    )
    output = (completed.stdout or completed.stderr).strip()
    return output if completed.returncode == 0 else None


def inspect(manifest_path: Path) -> dict[str, Any]:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    runtime_root = ROOT / "evals" / "external-runtimes" / "node_modules"
    codex = _native_runtime(runtime_root / "@openai", "codex.exe")
    claude = _native_runtime(runtime_root / "@anthropic-ai" / "claude-code", "claude.exe")

    parquet_info = manifest["annotation_dataset"]
    parquet = ROOT / parquet_info["local_file"]
    archive_info = manifest["official_artifacts"]
    archive = ROOT / archive_info["expected_local_path"]
    upstream = ROOT / "evals" / "public-benchmarks" / "ScienceAgentBench"
    model_id = manifest["same_model_track"]["model_id"]
    wrapper = ROOT / "evals" / "external-runtimes" / "with-deepseek.ps1"
    pilot = ROOT / "evals" / "agent" / "baselines" / "framework_pilot.py"
    wrapper_text = wrapper.read_text(encoding="utf-8") if wrapper.exists() else ""
    pilot_text = pilot.read_text(encoding="utf-8") if pilot.exists() else ""

    checks = {
        "codex_runtime": {
            "ok": codex is not None,
            "path": str(codex) if codex else None,
            "version": _version(codex),
        },
        "claude_runtime": {
            "ok": claude is not None,
            "path": str(claude) if claude else None,
            "version": _version(claude),
        },
        "annotation_parquet": {
            "ok": parquet.exists() and _sha256(parquet) == parquet_info["sha256"].upper(),
            "path": str(parquet),
            "sha256": _sha256(parquet) if parquet.exists() else None,
            "expected_rows": parquet_info["rows"],
        },
        "upstream_checkout": {
            "ok": (upstream / ".git").exists(),
            "path": str(upstream),
            "commit": manifest["upstream"]["commit"],
        },
        "same_model_lock": {
            "ok": (
                f'model = "{model_id}"' in wrapper_text
                and f'MODEL_ID = "{model_id}"' in pilot_text
            ),
            "configured": model_id,
            "required": manifest["same_model_track"]["model_id"],
            "source": "benchmark-owned wrapper and adapter",
        },
        "official_verified_archive": {
            "ok": archive.exists(),
            "path": str(archive),
            "access_status": archive_info["access_status"],
        },
        "codex_controlled_context": {
            "ok": manifest["same_model_track"]["adapter_smoke"].get(
                "codex_host_context_isolation"
            ) == "passed",
            "status": manifest["same_model_track"]["adapter_smoke"].get(
                "codex_host_context_isolation", "unknown"
            ),
        },
    }
    runtime_ready = all(
        checks[name]["ok"]
        for name in ("codex_runtime", "claude_runtime", "annotation_parquet", "upstream_checkout")
    )
    native_agent_score_ready = (
        runtime_ready
        and checks["same_model_lock"]["ok"]
        and checks["official_verified_archive"]["ok"]
    )
    controlled_core_score_ready = (
        native_agent_score_ready and checks["codex_controlled_context"]["ok"]
    )
    return {
        "benchmark": manifest["benchmark"],
        "split": parquet_info["split"],
        "task_count": parquet_info["rows"],
        "runtime_ready": runtime_ready,
        "official_score_ready": native_agent_score_ready,
        "native_agent_score_ready": native_agent_score_ready,
        "controlled_core_score_ready": controlled_core_score_ready,
        "checks": checks,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--require-official-ready", action="store_true")
    args = parser.parse_args()

    result = inspect(args.manifest.resolve())
    rendered = json.dumps(result, ensure_ascii=False, indent=2)
    print(rendered)
    if args.output:
        output = args.output.resolve()
        if output.exists():
            raise FileExistsError(f"Refusing to overwrite existing report: {output}")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(rendered + "\n", encoding="utf-8")
    if args.require_official_ready and not result["official_score_ready"]:
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
