"""Fast contract validation independent of Inspect model execution."""

from __future__ import annotations

from pathlib import Path

from galen_agent_eval.scenarios import load_scenarios


def main() -> None:
    scenarios = load_scenarios()
    repo_root = Path(__file__).resolve().parents[3]
    case_stems = [
        path.stem.lower()
        for path in (repo_root / "evals" / "cases").glob("*.toml")
    ]
    missing = [
        scenario.galen_case_id
        for scenario in scenarios
        if not any(
            stem == scenario.galen_case_id.lower()
            or stem.startswith(f"{scenario.galen_case_id.lower()}_")
            for stem in case_stems
        )
    ]
    if missing:
        raise SystemExit(f"missing native Galen cases: {', '.join(missing)}")
    print(f"OK: {len(scenarios)} scenarios, private/public separation, native cases resolved")


if __name__ == "__main__":
    main()
