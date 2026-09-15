"""Build the compact, judge-facing evidence bundle for submission item 07.

The bundle is generated only from frozen task cards and saved JSONL runs.  It
does not call a model and never edits the source experiment records.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import shutil
from pathlib import Path


CASES = {
    "PCTX03": ("pctx03_scope_switch.toml", None),
    "PDATA04": ("pdata04_conflict.toml", "conflicting-sources.md"),
    "PTOOL02": ("ptool02_evidence_task.toml", "study-constraints.md"),
}


def read_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def write_json(path: Path, payload: object) -> None:
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def failed_assertions(record: dict) -> list[str]:
    return [str(item.get("name")) for item in record.get("assertions", []) if not item.get("pass")]


def choose_pair(galen: list[dict], codex: list[dict], case_id: str) -> tuple[dict, dict]:
    g_rows = {int(r["run_index"]): r for r in galen if r["case_id"] == case_id}
    c_rows = {int(r["run_index"]): r for r in codex if r["case_id"] == case_id}
    for index in sorted(set(g_rows) & set(c_rows)):
        if g_rows[index].get("hard_gates_passed") and not c_rows[index].get("hard_gates_passed"):
            return g_rows[index], c_rows[index]
    index = sorted(set(g_rows) & set(c_rows))[0]
    return g_rows[index], c_rows[index]


def copy_galen_artifact(record: dict, target: Path) -> str:
    files = record.get("artifacts", {}).get("files", [])
    if not files:
        return "未声明产物"
    relative = Path(files[0])
    source = Path(record["workspace"]) / relative
    if source.exists():
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        return target.name
    return f"原工作区已清理；产物声明保留于 galen-run.json：{relative.as_posix()}"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    repo = args.repo.resolve()
    output = args.output.resolve()
    evidence = output / "evidence-data"
    raw_dir = evidence / "raw-results"
    cases_dir = evidence / "representative-cases"
    raw_dir.mkdir(parents=True, exist_ok=True)
    cases_dir.mkdir(parents=True, exist_ok=True)

    galen_src = repo / "evals/runs/large-full-galen-20260912-final.jsonl"
    codex_src = repo / "evals/runs/large-codex-deepseek-flash-20260912-complete.jsonl"
    galen = read_jsonl(galen_src)
    codex = read_jsonl(codex_src)
    shutil.copy2(galen_src, raw_dir / "galen-75-runs.jsonl")
    shutil.copy2(codex_src, raw_dir / "codex-deepseek-75-runs.jsonl")
    shutil.copy2(repo / "evals/runs/large-comparison-20260912-final.md", raw_dir / "final-statistical-summary.md")

    matrix_rows: list[dict[str, str]] = []
    for case_id, (card_name, fixture_name) in CASES.items():
        case_dir = cases_dir / case_id
        case_dir.mkdir(parents=True, exist_ok=True)
        shutil.copy2(repo / "evals/cases/paired" / card_name, case_dir / "task-card.toml")
        if fixture_name:
            shutil.copy2(repo / "evals/fixtures/paired/common/inputs" / fixture_name, case_dir / fixture_name)
        g, c = choose_pair(galen, codex, case_id)
        write_json(case_dir / "galen-run.json", g)
        write_json(case_dir / "codex-run.json", c)
        (case_dir / "galen-final-response.md").write_text(g.get("final_response", "") + "\n", encoding="utf-8")
        (case_dir / "codex-final-response.md").write_text(c.get("final_response", "") + "\n", encoding="utf-8")
        artifact_note = copy_galen_artifact(g, case_dir / "galen-artifact.md")
        comparison = f"""# {case_id} 代表性案例核验

配对重复编号：{g['run_index']}

| 系统 | 硬门 | 工具调用 | 总时延 | 失败断言 |
|---|---:|---:|---:|---|
| Galen | {'通过' if g['hard_gates_passed'] else '未通过'} | {g['tools']['calls']} | {g['latency']['total_ms']/1000:.1f} s | {', '.join(failed_assertions(g)) or '—'} |
| Codex + DeepSeek | {'通过' if c['hard_gates_passed'] else '未通过'} | {c['tools']['calls']} | {c['latency']['total_ms']/1000:.1f} s | {', '.join(failed_assertions(c)) or '见 codex-run.json'} |

Galen 产物：{artifact_note}

核验顺序：先读 `task-card.toml`，再对照两份 `*-run.json` 的 `assertions`、`tool_trace` 与 `artifacts` 字段，最后查看双方最终回答和 Galen 实际产物。
"""
        (case_dir / "README.md").write_text(comparison, encoding="utf-8")
        matrix_rows.append({
            "case_id": case_id,
            "run_index": str(g["run_index"]),
            "galen_pass": str(bool(g["hard_gates_passed"])).lower(),
            "codex_pass": str(bool(c["hard_gates_passed"])).lower(),
            "galen_tools": str(g["tools"]["calls"]),
            "codex_tools": str(c["tools"]["calls"]),
            "folder": f"evidence-data/representative-cases/{case_id}",
        })

    with (evidence / "representative-case-index.csv").open("w", encoding="utf-8-sig", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(matrix_rows[0]))
        writer.writeheader()
        writer.writerows(matrix_rows)

    manifest_files = sorted(p for p in evidence.rglob("*") if p.is_file() and p.name != "SHA256SUMS.txt")
    (evidence / "SHA256SUMS.txt").write_text(
        "\n".join(f"{sha256(path)}  {path.relative_to(evidence).as_posix()}" for path in manifest_files) + "\n",
        encoding="utf-8",
    )
    print(f"Built judge evidence bundle: {evidence}")


if __name__ == "__main__":
    main()
