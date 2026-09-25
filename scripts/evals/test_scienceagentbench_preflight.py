from pathlib import Path

from scienceagentbench_preflight import DEFAULT_MANIFEST, inspect


def test_pinned_public_assets_and_runtimes_are_ready() -> None:
    result = inspect(DEFAULT_MANIFEST)

    assert result["benchmark"] == "ScienceAgentBench"
    assert result["split"] == "verified"
    assert result["task_count"] == 102
    assert result["runtime_ready"] is True
    assert result["checks"]["annotation_parquet"]["ok"] is True


def test_official_score_cannot_be_claimed_without_locked_inputs() -> None:
    result = inspect(DEFAULT_MANIFEST)

    expected_archive = Path(result["checks"]["official_verified_archive"]["path"])
    if not expected_archive.exists():
        assert result["official_score_ready"] is False
    assert result["checks"]["same_model_lock"]["required"] == "deepseek-flash"
    assert result["checks"]["codex_controlled_context"]["ok"] is False
    assert result["controlled_core_score_ready"] is False
