"""Statistical analysis for the Galen architecture pilot.

The unit of resampling is a task card (not an individual run), which avoids
pretending that five repeats of the same card are five independent research
questions. The script writes machine-readable JSON and a short Markdown
summary next to the aligned report.
"""
from __future__ import annotations

import argparse
import json
import math
import random
import statistics
from pathlib import Path


GROUPS = ("PCTX", "PDATA", "PTOOL")


def load_jsonl(path: Path) -> list[dict]:
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            rows.append(json.loads(line))
    return rows


def wilson(successes: int, total: int, z: float = 1.959963984540054) -> tuple[float, float]:
    if not total:
        return 0.0, 0.0
    p = successes / total
    z2 = z * z
    denom = 1.0 + z2 / total
    centre = p + z2 / (2.0 * total)
    spread = z * math.sqrt((p * (1.0 - p) / total) + z2 / (4.0 * total * total))
    return (centre - spread) / denom, (centre + spread) / denom


def by_case(records: list[dict]) -> dict[str, list[dict]]:
    out: dict[str, list[dict]] = {}
    for row in records:
        out.setdefault(row["case_id"], []).append(row)
    for rows in out.values():
        rows.sort(key=lambda row: (int(row.get("run_index", 0)), row.get("run_id", "")))
    return out


def case_rate(rows: list[dict]) -> float:
    return sum(bool(row["hard_gates_passed"]) for row in rows) / len(rows)


def percentile(values: list[float], p: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    pos = max(0.0, min(1.0, p)) * (len(ordered) - 1)
    lo, hi = math.floor(pos), math.ceil(pos)
    if lo == hi:
        return ordered[lo]
    weight = pos - lo
    return ordered[lo] * (1.0 - weight) + ordered[hi] * weight


def bootstrap_case_difference(
    galen_cases: dict[str, list[dict]],
    external_cases: dict[str, list[dict]],
    repeats: int = 20_000,
    seed: int = 20260911,
) -> dict[str, float]:
    case_ids = sorted(set(galen_cases) & set(external_cases))
    diffs = [case_rate(galen_cases[k]) - case_rate(external_cases[k]) for k in case_ids]
    rng = random.Random(seed)
    samples = []
    for _ in range(repeats):
        draw = [rng.choice(diffs) for _ in diffs]
        samples.append(statistics.mean(draw))
    return {
        "case_count": len(case_ids),
        "mean_difference": statistics.mean(diffs),
        "ci95_low": percentile(samples, 0.025),
        "ci95_high": percentile(samples, 0.975),
        "case_differences": {case_id: round(diff, 6) for case_id, diff in zip(case_ids, diffs)},
    }


def paired_discordance(galen_cases: dict[str, list[dict]], external_cases: dict[str, list[dict]]) -> dict[str, int]:
    concordant_pass = concordant_fail = galen_only = external_only = 0
    for case_id in sorted(set(galen_cases) & set(external_cases)):
        g_rows, e_rows = galen_cases[case_id], external_cases[case_id]
        for g, e in zip(g_rows, e_rows):
            gp, ep = bool(g["hard_gates_passed"]), bool(e["hard_gates_passed"])
            if gp and ep:
                concordant_pass += 1
            elif not gp and not ep:
                concordant_fail += 1
            elif gp:
                galen_only += 1
            else:
                external_only += 1
    return {
        "concordant_pass": concordant_pass,
        "concordant_fail": concordant_fail,
        "galen_only_pass": galen_only,
        "external_only_pass": external_only,
    }


def aggregate(records: list[dict]) -> dict[str, float | int]:
    successes = sum(bool(row["hard_gates_passed"]) for row in records)
    lo, hi = wilson(successes, len(records))
    tools = [float(row["tools"]["calls"]) for row in records]
    latency = [float(row["latency"]["total_ms"]) / 1000.0 for row in records]
    return {
        "n": len(records),
        "successes": successes,
        "pass_rate": successes / len(records),
        "wilson_lower_95": lo,
        "wilson_upper_95": hi,
        "mean_tools": statistics.mean(tools),
        "median_latency_s": statistics.median(latency),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--galen", type=Path, default=Path("evals/runs/galen-strict-comparable-v1.jsonl"))
    parser.add_argument("--external", type=Path, default=Path("evals/runs/external-codex-deepseek-flash-strict-v1.jsonl"))
    parser.add_argument("--output", type=Path, default=Path("output/pdf/galen-strict-aligned/architecture_pilot_statistics.json"))
    args = parser.parse_args()

    galen = load_jsonl(args.galen)
    external = load_jsonl(args.external)
    galen_cases, external_cases = by_case(galen), by_case(external)
    result = {
        "schema_version": 1,
        "unit_of_resampling": "case_id",
        "galen": aggregate(galen),
        "external": aggregate(external),
        "bootstrap_case_pass_rate_difference": bootstrap_case_difference(galen_cases, external_cases),
        "paired_repeat_discordance": paired_discordance(galen_cases, external_cases),
        "groups": {},
    }
    for prefix in GROUPS:
        g_rows = [row for row in galen if row["case_id"].startswith(prefix)]
        e_rows = [row for row in external if row["case_id"].startswith(prefix)]
        result["groups"][prefix] = {
            "galen": aggregate(g_rows),
            "external": aggregate(e_rows),
        }

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    md = args.output.with_suffix(".md")
    g, e = result["galen"], result["external"]
    boot = result["bootstrap_case_pass_rate_difference"]
    disc = result["paired_repeat_discordance"]
    lines = [
        "# Galen 架构先导实验统计摘要",
        "",
        "重采样单位：任务卡（15 张），而非单次重复运行。bootstrap 重复 20,000 次。",
        "",
        "| 系统 | 通过率 | Wilson 95% 区间 | 平均工具调用 | 中位总时延 |",
        "|---|---:|---:|---:|---:|",
        f"| Galen | {g['pass_rate']:.1%} | [{g['wilson_lower_95']:.1%}, {g['wilson_upper_95']:.1%}] | {g['mean_tools']:.2f} | {g['median_latency_s']:.1f} s |",
        f"| External Codex + DeepSeek | {e['pass_rate']:.1%} | [{e['wilson_lower_95']:.1%}, {e['wilson_upper_95']:.1%}] | {e['mean_tools']:.2f} | {e['median_latency_s']:.1f} s |",
        "",
        f"按任务卡计算的通过率差异均值为 **{boot['mean_difference']:.1%}**，bootstrap 95% 区间为 **[{boot['ci95_low']:.1%}, {boot['ci95_high']:.1%}]**。",
        f"按相同 case/run_index 对齐的探索性结果：Galen 独有通过 {disc['galen_only_pass']} 次，外部独有通过 {disc['external_only_pass']} 次，共同通过 {disc['concordant_pass']} 次，共同失败 {disc['concordant_fail']} 次。",
        "",
        "该统计用于架构先导验证；由于两组运行时尚未完全同构，不能解释为底层模型的普遍排名。",
    ]
    md.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {args.output} and {md}")


if __name__ == "__main__":
    main()
