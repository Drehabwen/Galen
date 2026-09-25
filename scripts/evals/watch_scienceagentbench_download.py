"""Idempotently process ScienceAgentBench after its browser download finishes."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import py_compile
import shutil
import subprocess
import sys
import tempfile
import time
import zipfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DOWNLOADS = Path.home() / "Downloads"
BENCH_ROOT = ROOT / "evals" / "public-benchmarks"
EXPECTED_ARCHIVE = BENCH_ROOT / "downloads" / "benchmark_verified.zip"
EXTRACTED = BENCH_ROOT / "downloads" / "benchmark_verified"
STATE_PATH = BENCH_ROOT / "watch-state.json"
RUN_ROOT = ROOT / "evals" / "runs" / "scienceagentbench-after-download"
PASSWORD = b"scienceagentbench"
STABLE_SECONDS = 45


def _now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()


def _load_state() -> dict[str, Any]:
    if not STATE_PATH.exists():
        return {"schema_version": 1, "phase": "waiting_download", "updated_at": _now()}
    return json.loads(STATE_PATH.read_text(encoding="utf-8"))


def _save(state: dict[str, Any]) -> None:
    state["updated_at"] = _now()
    STATE_PATH.parent.mkdir(parents=True, exist_ok=True)
    temporary = STATE_PATH.with_suffix(".tmp")
    temporary.write_text(json.dumps(state, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    os.replace(temporary, STATE_PATH)


def _candidate() -> Path | None:
    candidates = []
    if EXPECTED_ARCHIVE.exists():
        candidates.append(EXPECTED_ARCHIVE)
    if DOWNLOADS.exists():
        candidates.extend(DOWNLOADS.glob("benchmark_verified*.zip"))
    files = [path for path in candidates if path.is_file()]
    return max(files, key=lambda path: path.stat().st_mtime) if files else None


def _download_is_partial(candidate: Path) -> bool:
    partial_suffixes = (".crdownload", ".part", ".download", ".tmp")
    stems = {candidate.name, candidate.stem}
    return any(
        item.is_file()
        and item.name.startswith(tuple(stems))
        and item.name.lower().endswith(partial_suffixes)
        for item in candidate.parent.iterdir()
    )


def _stable(candidate: Path, state: dict[str, Any]) -> bool:
    stat = candidate.stat()
    fingerprint = {"path": str(candidate), "size": stat.st_size, "mtime_ns": stat.st_mtime_ns}
    previous = state.get("candidate")
    if previous != fingerprint:
        state["candidate"] = fingerprint
        state["stable_since_epoch"] = time.time()
        state["phase"] = "waiting_stable"
        _save(state)
        return False
    stable_since = float(state.get("stable_since_epoch", time.time()))
    old_enough = time.time() - max(stable_since, stat.st_mtime) >= STABLE_SECONDS
    return old_enough and not _download_is_partial(candidate)


def _validate_zip(path: Path) -> dict[str, Any]:
    with zipfile.ZipFile(path) as archive:
        archive.setpassword(PASSWORD)
        members = [item for item in archive.infolist() if not item.is_dir()]
        if not members:
            raise ValueError("Downloaded ZIP contains no files")
        root = Path("C:/scienceagentbench-archive-root")
        for member in archive.infolist():
            target = (root / member.filename).resolve()
            if os.path.commonpath([str(root), str(target)]) != str(root):
                raise ValueError(f"Unsafe ZIP member path: {member.filename}")
        probe = min(members, key=lambda item: item.file_size)
        with archive.open(probe, pwd=PASSWORD) as stream:
            stream.read(min(probe.file_size, 4096))
        return {
            "members": len(archive.infolist()),
            "files": len(members),
            "compressed_bytes": path.stat().st_size,
            "integrity_check": "member paths and password checked; CRC verified during extraction",
        }


def _safe_extract(path: Path, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix="sab-extract-", dir=destination.parent))
    try:
        with zipfile.ZipFile(path) as archive:
            root = temporary.resolve()
            for member in archive.infolist():
                target = (temporary / member.filename).resolve()
                if os.path.commonpath([str(root), str(target)]) != str(root):
                    raise ValueError(f"Unsafe ZIP member path: {member.filename}")
            archive.extractall(temporary, pwd=PASSWORD)
        if destination.exists():
            shutil.rmtree(destination)
        os.replace(temporary, destination)
    except Exception:
        shutil.rmtree(temporary, ignore_errors=True)
        raise


def _find_benchmark_root() -> Path:
    candidates = [EXTRACTED, *EXTRACTED.rglob("benchmark")]
    for candidate in candidates:
        if (candidate / "eval_programs").is_dir() and (candidate / "gold_programs").is_dir():
            return candidate
    for eval_dir in EXTRACTED.rglob("eval_programs"):
        candidate = eval_dir.parent
        if (candidate / "gold_programs").is_dir():
            return candidate
    raise FileNotFoundError("Could not locate benchmark/eval_programs and benchmark/gold_programs")


def _run(command: list[str], cwd: Path = ROOT, timeout: int = 180) -> dict[str, Any]:
    completed = subprocess.run(
        command,
        cwd=cwd,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
        check=False,
    )
    return {
        "command": command,
        "returncode": completed.returncode,
        "stdout_tail": completed.stdout[-4000:],
        "stderr_tail": completed.stderr[-4000:],
    }


def _stage_existing_predictions() -> dict[str, Any]:
    sources = {
        "codex": ROOT / "evals/runs/scienceagentbench-public-generation-smoke/codex/instance-1/program.py",
        "claude": ROOT / "evals/runs/scienceagentbench-public-generation-smoke-claude-retry/claude/instance-1/program.py",
    }
    staged: dict[str, Any] = {}
    for backend, source in sources.items():
        destination = RUN_ROOT / "pred_programs" / backend / "pred_clintox_nn.py"
        destination.parent.mkdir(parents=True, exist_ok=True)
        if source.exists():
            shutil.copy2(source, destination)
            py_compile.compile(str(destination), doraise=True)
            staged[backend] = {"ok": True, "path": str(destination), "bytes": destination.stat().st_size}
        else:
            staged[backend] = {"ok": False, "reason": "source program missing"}
    return staged


def process_once() -> dict[str, Any]:
    state = _load_state()
    if state.get("phase") == "complete":
        return state
    candidate = _candidate()
    if candidate is None:
        state["phase"] = "waiting_download"
        _save(state)
        return state
    if not _stable(candidate, state):
        return state

    try:
        state["phase"] = "validating_archive"
        _save(state)
        EXPECTED_ARCHIVE.parent.mkdir(parents=True, exist_ok=True)
        if candidate.resolve() != EXPECTED_ARCHIVE.resolve():
            copying = EXPECTED_ARCHIVE.with_suffix(".zip.copying")
            shutil.copy2(candidate, copying)
            os.replace(copying, EXPECTED_ARCHIVE)
        state["archive"] = _validate_zip(EXPECTED_ARCHIVE)

        state["phase"] = "extracting"
        _save(state)
        if not EXTRACTED.exists():
            _safe_extract(EXPECTED_ARCHIVE, EXTRACTED)
        benchmark = _find_benchmark_root()
        state["benchmark_root"] = str(benchmark)
        state["asset_counts"] = {
            "eval_programs": len(list((benchmark / "eval_programs").glob("*.py"))),
            "gold_programs": len(list((benchmark / "gold_programs").glob("*.py"))),
            "dataset_files": len([path for path in benchmark.rglob("*") if path.is_file()]),
        }

        state["phase"] = "running_tests"
        _save(state)
        RUN_ROOT.mkdir(parents=True, exist_ok=True)
        state["predictions"] = _stage_existing_predictions()
        state["tests"] = {
            "preflight": _run([sys.executable, "scripts/evals/scienceagentbench_preflight.py"]),
            "preflight_tests": _run(
                [sys.executable, "-m", "pytest", "scripts/evals/test_scienceagentbench_preflight.py", "-q"]
            ),
        }
        docker = shutil.which("docker")
        conda = shutil.which("conda")
        state["official_execution"] = {
            "started": False,
            "docker": docker,
            "conda": conda,
            "reason": (
                "Official assets validated and predictions staged. Full program execution needs "
                "Docker or Conda; neither runtime was available when the monitor completed."
                if not docker and not conda
                else "Evaluator runtime detected; manual launch retained to avoid an unbounded image build."
            ),
        }
        state["phase"] = "complete"
    except Exception as error:
        state["phase"] = "failed"
        state["error"] = f"{type(error).__name__}: {error}"
    _save(state)
    return state


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--once", action="store_true", help="perform one idempotent monitor pass")
    parser.parse_args()
    state = process_once()
    print(json.dumps(state, ensure_ascii=False, indent=2))
    return 1 if state.get("phase") == "failed" else 0


if __name__ == "__main__":
    raise SystemExit(main())
