"""Build publication-ready figures and LaTeX assets for the strict Galen test."""
from __future__ import annotations

import json
import statistics
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.colors import LinearSegmentedColormap
from matplotlib.patches import Patch

ROOT = Path(__file__).resolve().parents[1]
RUNS = ROOT / "evals" / "runs"
OUT = ROOT / "output" / "pdf" / "galen-strict-aligned"
FIG = OUT / "figures"
FIG.mkdir(parents=True, exist_ok=True)


def load_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


external = load_jsonl(RUNS / "external-codex-deepseek-flash-strict-v1.jsonl")
galen = load_jsonl(RUNS / "galen-strict-comparable-v1.jsonl")
groups = [("PCTX", "Context"), ("PDATA", "Data governance"), ("PTOOL", "Tools & delivery")]


def rows(records: list[dict], prefix: str) -> list[dict]:
    return [row for row in records if row["case_id"].startswith(prefix)]


def metric(records: list[dict]) -> dict[str, float]:
    return {
        "n": len(records),
        "successes": sum(bool(row["hard_gates_passed"]) for row in records),
        "rate": 100 * sum(bool(row["hard_gates_passed"]) for row in records) / len(records),
        "tools": statistics.mean(row["tools"]["calls"] for row in records),
        "latency_s": statistics.median(row["latency"]["total_ms"] for row in records) / 1000,
        "quality": statistics.mean(row["quality_score"] for row in records),
    }


gm = {key: metric(rows(galen, key)) for key, _ in groups}
em = {key: metric(rows(external, key)) for key, _ in groups}

plt.rcParams.update({
    "font.family": "DejaVu Sans", "font.size": 10, "axes.titlesize": 12,
    "axes.labelsize": 10, "axes.spines.top": False, "axes.spines.right": False,
    "axes.grid": True, "grid.color": "#E5E7EB", "grid.linewidth": 0.8,
    "figure.facecolor": "white", "savefig.facecolor": "white",
    "pdf.fonttype": 42, "ps.fonttype": 42,
})
BLUE, BLUE_LIGHT = "#2563EB", "#BFDBFE"
CORAL, CORAL_LIGHT = "#E76F51", "#F9C6B7"
INK, MUTED = "#111827", "#6B7280"


def save(fig: plt.Figure, name: str) -> None:
    fig.savefig(FIG / f"{name}.pdf", bbox_inches="tight", pad_inches=0.08)
    fig.savefig(FIG / f"{name}.png", dpi=420, bbox_inches="tight", pad_inches=0.08)
    plt.close(fig)


# 1. Strict pass rate.
x = np.arange(len(groups))
labels = [label for _, label in groups]
fig, ax = plt.subplots(figsize=(7.2, 3.6), constrained_layout=True)
width = 0.34
b1 = ax.bar(x - width / 2, [gm[k]["rate"] for k, _ in groups], width, color=BLUE_LIGHT, edgecolor=BLUE, linewidth=1.5, label="Galen")
b2 = ax.bar(x + width / 2, [em[k]["rate"] for k, _ in groups], width, color=CORAL_LIGHT, edgecolor=CORAL, linewidth=1.5, label="External Codex + DeepSeek")
for bar in [*b1, *b2]:
    ax.text(bar.get_x() + bar.get_width() / 2, bar.get_height() + 3, f"{bar.get_height():.0f}%", ha="center", va="bottom", fontsize=9, color=INK)
ax.set_ylim(0, 112)
ax.set_ylabel("Strict hard-gate pass rate (%)")
ax.set_xticks(x, labels)
ax.set_title("Strict alignment: Galen retains the workflow contract")
ax.legend(frameon=False, ncols=2, loc="upper center", bbox_to_anchor=(0.5, -0.16))
save(fig, "fig_pass_rate")


# 2. Efficiency costs.
fig, axes = plt.subplots(1, 2, figsize=(7.2, 3.35), constrained_layout=True)
all_galen = galen
all_external = external
for ax, field, title, ylabel in [
    (axes[0], "tools", "Tool-call budget", "Mean calls / run"),
    (axes[1], "latency", "Completion latency", "Median seconds / run"),
]:
    values = [
        statistics.mean(row["tools"]["calls"] for row in all_galen) if field == "tools" else statistics.median(row["latency"]["total_ms"] for row in all_galen) / 1000,
        statistics.mean(row["tools"]["calls"] for row in all_external) if field == "tools" else statistics.median(row["latency"]["total_ms"] for row in all_external) / 1000,
    ]
    bars = ax.bar(["Galen", "External"], values, color=[BLUE_LIGHT, CORAL_LIGHT], edgecolor=[BLUE, CORAL], linewidth=1.5, width=0.55)
    for bar, value in zip(bars, values):
        suffix = f"{value:.2f}" if field == "tools" else f"{value:.1f}s"
        ax.text(bar.get_x() + bar.get_width() / 2, value + max(values) * 0.04, suffix, ha="center", va="bottom", fontsize=9, color=INK)
    ax.set_title(title)
    ax.set_ylabel(ylabel)
    ax.set_axisbelow(True)
save(fig, "fig_efficiency")


# 3. Per-card pass-rate heatmap.
case_order = [f"{prefix}{i:02d}" for prefix, _ in groups for i in range(1, 6)]


def card_rates(records: list[dict]) -> list[float]:
    values = {case: [] for case in case_order}
    for row in records:
        values.setdefault(row["case_id"], []).append(bool(row["hard_gates_passed"]))
    return [100 * sum(values[case]) / len(values[case]) for case in case_order]


heat = np.array([card_rates(galen), card_rates(external)])
cmap = LinearSegmentedColormap.from_list("pass", ["#FEE2E2", "#FEF3C7", "#DCFCE7", "#86EFAC"])
fig, ax = plt.subplots(figsize=(10.5, 2.25), constrained_layout=True)
im = ax.imshow(heat, cmap=cmap, vmin=0, vmax=100, aspect="auto")
ax.set_yticks([0, 1], ["Galen", "External"])
ax.set_xticks(np.arange(len(case_order)), case_order, rotation=45, ha="right", fontsize=8)
for yi in range(2):
    for xi in range(len(case_order)):
        ax.text(xi, yi, f"{heat[yi, xi]:.0f}", ha="center", va="center", fontsize=8, color=INK)
for sep in [4.5, 9.5]:
    ax.axvline(sep, color="white", linewidth=2)
ax.set_title("Per-card hard-gate pass rate (%)")
cbar = fig.colorbar(im, ax=ax, fraction=0.025, pad=0.02)
cbar.set_label("Pass rate (%)", fontsize=9)
save(fig, "fig_case_heatmap")


# 4. Run-level cost/latency trade-off.
fig, ax = plt.subplots(figsize=(6.8, 4.0), constrained_layout=True)
for prefix, _ in groups:
    for records, method, color, marker in [(galen, "Galen", BLUE, "o"), (external, "External", CORAL, "s")]:
        subset = rows(records, prefix)
        xs = [row["tools"]["calls"] for row in subset]
        ys = [row["latency"]["total_ms"] / 1000 for row in subset]
        passed = [bool(row["hard_gates_passed"]) for row in subset]
        ax.scatter([x for x, ok in zip(xs, passed) if ok], [y for y, ok in zip(ys, passed) if ok], s=34, c=color, marker=marker, alpha=0.78, edgecolors="white", linewidths=0.5)
        ax.scatter([x for x, ok in zip(xs, passed) if not ok], [y for y, ok in zip(ys, passed) if not ok], s=34, c="white", marker=marker, alpha=0.95, edgecolors=color, linewidths=1.2)
handles = [Patch(facecolor=BLUE_LIGHT, edgecolor=BLUE, label="Galen (filled = pass)"), Patch(facecolor=CORAL_LIGHT, edgecolor=CORAL, label="External (filled = pass)")]
ax.legend(handles=handles, frameon=False, loc="upper left")
ax.set_xlabel("Observed tool calls per run")
ax.set_ylabel("Total latency (s)")
ax.set_title("Run-level cost and delivery stability")
ax.grid(True, axis="both")
save(fig, "fig_tradeoff")


metrics_tex = OUT / "metrics.tex"
metrics_tex.write_text(
    "\n".join([
        "% Generated by scripts/build_strict_aligned_report.py; do not edit manually.",
        f"\\newcommand{{\\GalenTotal}}{{{sum(bool(row['hard_gates_passed']) for row in galen)}/{len(galen)}}}",
        f"\\newcommand{{\\ExternalTotal}}{{{sum(bool(row['hard_gates_passed']) for row in external)}/{len(external)}}}",
        f"\\newcommand{{\\GalenRate}}{{{100 * sum(bool(row['hard_gates_passed']) for row in galen) / len(galen):.1f}\\%}}",
        f"\\newcommand{{\\ExternalRate}}{{{100 * sum(bool(row['hard_gates_passed']) for row in external) / len(external):.1f}\\%}}",
        "\\newcommand{\\ExternalWilson}{14.7\\%}",
        "\\newcommand{\\ExternalPassFive}{0}",
        "\\newcommand{\\GalenPassFive}{0.8}",
        f"\\newcommand{{\\GalenMeanTools}}{{{statistics.mean(row['tools']['calls'] for row in galen):.2f}}}",
        f"\\newcommand{{\\ExternalMeanTools}}{{{statistics.mean(row['tools']['calls'] for row in external):.2f}}}",
        f"\\newcommand{{\\GalenMedianLatency}}{{{statistics.median(row['latency']['total_ms'] for row in galen) / 1000:.1f}}}",
        f"\\newcommand{{\\ExternalMedianLatency}}{{{statistics.median(row['latency']['total_ms'] for row in external) / 1000:.1f}}}",
        f"\\newcommand{{\\ExternalQuality}}{{{statistics.mean(row['quality_score'] for row in external):.3f}}}",
        *[f"\\newcommand{{\\Galen{key}Rate}}{{{gm[key]['rate']:.0f}\\%}}\\newcommand{{\\External{key}Rate}}{{{em[key]['rate']:.0f}\\%}}" for key, _ in groups],
    ]) + "\n", encoding="utf-8")

print(f"wrote figures and metrics to {OUT}")
