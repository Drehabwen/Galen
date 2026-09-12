"""Aggregate architecture-ablation JSONL runs by variant and case."""
from __future__ import annotations

import argparse
import json
import math
import statistics
from pathlib import Path


def load_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def wilson(successes: int, total: int) -> tuple[float, float]:
    if total == 0:
        return 0.0, 0.0
    z = 1.959963984540054
    p = successes / total
    z2 = z * z
    denominator = 1.0 + z2 / total
    centre = p + z2 / (2.0 * total)
    spread = z * math.sqrt(p * (1.0 - p) / total + z2 / (4.0 * total * total))
    return (centre - spread) / denominator, (centre + spread) / denominator


def summarize(rows: list[dict]) -> dict:
    successes = sum(bool(row.get("hard_gates_passed")) for row in rows)
    tools = [float(row.get("tools", {}).get("calls", 0)) for row in rows]
    tool_errors = [float(row.get("tools", {}).get("errors", 0)) for row in rows]
    model_requests = [float(row.get("model_requests", 0)) for row in rows]
    input_tokens = [float(row.get("usage", {}).get("input", 0)) for row in rows]
    output_tokens = [float(row.get("usage", {}).get("output", 0)) for row in rows]
    latency = [float(row.get("latency", {}).get("total_ms", 0)) / 1000 for row in rows]
    lo, hi = wilson(successes, len(rows))
    return {
        "n": len(rows),
        "successes": successes,
        "pass_rate": successes / len(rows) if rows else 0.0,
        "wilson_lower_95": lo,
        "wilson_upper_95": hi,
        "pass_k": (1.0 if successes == len(rows) else 0.0) if rows else 0.0,
        "mean_tools": statistics.mean(tools) if tools else 0.0,
        "mean_tool_errors": statistics.mean(tool_errors) if tool_errors else 0.0,
        "mean_model_requests": statistics.mean(model_requests) if model_requests else 0.0,
        "mean_input_tokens": statistics.mean(input_tokens) if input_tokens else 0.0,
        "mean_output_tokens": statistics.mean(output_tokens) if output_tokens else 0.0,
        "median_latency_s": statistics.median(latency) if latency else 0.0,
        "failures": sorted({
            assertion.get("name", "unknown")
            for row in rows
            for assertion in row.get("assertions", [])
            if not assertion.get("pass", False)
        }),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("run_dir", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--cases", nargs="*", default=[], help="只汇总指定 case；避免部分运行目录混入")
    args = parser.parse_args()
    run_dir = args.run_dir.resolve()
    variants: dict[str, list[dict]] = {}
    selected_cases = {value.upper() for value in args.cases}
    for variant_dir in sorted(path for path in run_dir.iterdir() if path.is_dir()):
        rows = []
        for path in sorted(variant_dir.glob("*.jsonl")):
            if selected_cases and path.stem.upper() not in selected_cases:
                continue
            rows.extend(load_jsonl(path))
        if rows:
            variants[variant_dir.name] = rows
    if not variants:
        raise SystemExit(f"{run_dir} 中没有 JSONL 运行记录")
    output = (args.output or run_dir / "ablation-summary.json").resolve()
    result = {
        "schema_version": 1,
        "run_dir": str(run_dir),
        "variants": {name: summarize(rows) for name, rows in variants.items()},
        "by_case": {
            case_id: {
                name: summarize([row for row in rows if row.get("case_id") == case_id])
                for name, rows in variants.items()
            }
            for case_id in sorted({row.get("case_id") for rows in variants.values() for row in rows})
        },
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    markdown = output.with_suffix(".md")
    lines = [
        "# Galen 架构消融汇总",
        "",
        f"运行目录：`{run_dir}`。汇总单位保留为 variant × case；不要把不同任务卡直接视为同质样本。",
        "",
        "| 变体 | 通过 | Wilson 95% 区间 | Pass^k | 平均工具调用 | 平均输入 token | 平均模型请求 | 中位时延 |",
        "|---|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for name, summary in result["variants"].items():
        lines.append(
            f"| {name} | {summary['successes']}/{summary['n']} | "
            f"[{summary['wilson_lower_95']:.1%}, {summary['wilson_upper_95']:.1%}] | "
            f"{summary['pass_k']:.1f} | {summary['mean_tools']:.2f} | "
            f"{summary['mean_input_tokens']:.0f} | {summary['mean_model_requests']:.2f} | "
            f"{summary['median_latency_s']:.1f} s |"
        )
    lines += ["", "## 按任务卡", "", "| Case | Variant | 通过 | 失败断言 |", "|---|---|---:|---|"]
    for case_id, per_variant in result["by_case"].items():
        for name, summary in per_variant.items():
            failures = ", ".join(summary["failures"]) or "—"
            lines.append(f"| {case_id} | {name} | {summary['successes']}/{summary['n']} | {failures} |")
    lines += ["", "解释边界：当前结果是消融 runner 的首轮数据。正式结论须在同 provider、同工具、每卡至少 5 次且覆盖全部 PCTX/PDATA/PTOOL 后再作。"]
    markdown.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {output} and {markdown}")


if __name__ == "__main__":
    main()
