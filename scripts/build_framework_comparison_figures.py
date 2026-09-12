"""Build paper figures for the Galen/framework comparison report.

All figures are deterministic and derive directly from RunRecord JSONL files.
The script intentionally keeps the OpenCode pilot separate from the repeated
Galen/Codex comparison because its coverage is different.
"""

from __future__ import annotations

import json
import math
from collections import defaultdict
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np


ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "output" / "pdf" / "galen-framework-comparison" / "figures"

BLUE = "#2563EB"
GREEN = "#16A34A"
RED = "#DC2626"
PURPLE = "#7C3AED"
GRAY = "#525252"
LIGHT = "#F8FAFC"

plt.rcParams.update({
    "font.family": "DejaVu Sans",
    "font.size": 8.5,
    "axes.titlesize": 9.5,
    "axes.labelsize": 8.5,
    "xtick.labelsize": 8,
    "ytick.labelsize": 8,
    "pdf.fonttype": 42,
    "ps.fonttype": 42,
    "svg.fonttype": "none",
})


def load(path: str | Path) -> list[dict]:
    return [json.loads(line) for line in Path(path).read_text(encoding="utf-8").splitlines() if line.strip()]


def wilson(k: int, n: int, z: float = 1.959963984540054) -> tuple[float, float]:
    if n == 0:
        return 0.0, 0.0
    p = k / n
    denom = 1 + z * z / n
    centre = (p + z * z / (2 * n)) / denom
    half = z * math.sqrt((p * (1 - p) + z * z / (4 * n)) / n) / denom
    return max(0.0, centre - half), min(1.0, centre + half)


def save(fig: plt.Figure, stem: str) -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    fig.savefig(OUT / f"{stem}.pdf", bbox_inches="tight", facecolor="white")
    fig.savefig(OUT / f"{stem}.svg", bbox_inches="tight", facecolor="white")
    fig.savefig(OUT / f"{stem}.png", dpi=420, bbox_inches="tight", facecolor="white")
    plt.close(fig)


galen = load(ROOT / "evals" / "runs" / "large-full-galen-20260912-final.jsonl")
codex = load(ROOT / "evals" / "runs" / "large-codex-deepseek-flash-20260912-complete.jsonl")
opencode = load(ROOT / "evals" / "runs" / "pilot-opencode-all-v1.jsonl")
opencode_r5 = load(ROOT / "evals" / "runs" / "pilot-opencode-stratified-r5-20260912.jsonl")


def summary(rows: list[dict]) -> dict:
    n = len(rows)
    k = sum(bool(r["hard_gates_passed"]) for r in rows)
    lo, hi = wilson(k, n)
    return {
        "n": n,
        "k": k,
        "rate": k / n if n else 0.0,
        "lo": lo,
        "hi": hi,
        "calls": float(np.mean([r["tools"]["calls"] for r in rows])) if rows else 0.0,
        "latency_median": float(np.median([r["latency"]["total_ms"] for r in rows]) / 1000) if rows else 0.0,
    }


# Figure 1: success and repeated subset.
repeated = [summary(galen), summary(codex)]
pilot = summary(opencode)
labels = ["Galen\n(n=75)", "Codex + DS\n(n=75)", "OpenCode + DS\n(n=15)"]
stats = repeated + [pilot]
colors = [BLUE, RED, PURPLE]
fig, axes = plt.subplots(1, 2, figsize=(7.0, 3.15), gridspec_kw={"width_ratios": [1.08, 1]})
ax = axes[0]
x = np.arange(3)
rates = np.array([s["rate"] for s in stats]) * 100
low = np.array([s["rate"] - s["lo"] for s in stats]) * 100
high = np.array([s["hi"] - s["rate"] for s in stats]) * 100
ax.bar(x, rates, color=[f"{c}CC" for c in colors], edgecolor=colors, linewidth=1.3, width=0.62,
       yerr=np.vstack([low, high]), capsize=4, error_kw={"elinewidth": 1.0, "ecolor": GRAY})
for i, (v, s) in enumerate(zip(rates, stats)):
    ax.text(i, min(101, v + 5), f"{s['k']}/{s['n']}\n{v:.1f}%", ha="center", va="bottom", fontsize=8, fontweight="bold")
ax.set_ylim(0, 112)
ax.set_ylabel("Hard-gate success (%)")
ax.set_xticks(x, labels)
ax.set_title("A  Protocol-matched completion")
ax.grid(axis="y", color="#E5E7EB", linewidth=0.7)
ax.set_axisbelow(True)
for spine in ["top", "right"]:
    ax.spines[spine].set_visible(False)

ax = axes[1]
subset_groups = ["PCTX01", "PDATA02", "PTOOL01"]
subset_rates = []
for case in subset_groups:
    rows = [r for r in opencode_r5 if r["case_id"] == case]
    subset_rates.append(summary(rows)["rate"] * 100)
ax.bar(np.arange(3), subset_rates, color=f"{PURPLE}CC", edgecolor=PURPLE, linewidth=1.3, width=0.6)
for i, v in enumerate(subset_rates):
    ax.text(i, v + 4, f"{v:.0f}%", ha="center", va="bottom", fontsize=8, fontweight="bold")
ax.set_ylim(0, 112)
ax.set_ylabel("Success (%)")
ax.set_xticks(np.arange(3), subset_groups)
ax.set_title("B  OpenCode repeated subset\n(5 runs/card)")
ax.grid(axis="y", color="#E5E7EB", linewidth=0.7)
ax.set_axisbelow(True)
for spine in ["top", "right"]:
    ax.spines[spine].set_visible(False)
fig.suptitle("Hard-gate success under the frozen paired-card protocol", y=1.02, fontsize=11, fontweight="bold")
fig.text(0.01, -0.02, "Error bars: Wilson 95% intervals. OpenCode pilot and repeated subset are shown separately from the 75-run repeated comparison.", fontsize=7.4, color=GRAY)
save(fig, "fig1_success")


# Figure 2: calls and latency for the two fully repeated systems.
fig, axes = plt.subplots(1, 2, figsize=(6.9, 2.95))
names = ["Galen", "Codex + DS"]
summaries = [summary(galen), summary(codex)]
for ax, key, ylabel, title, fmt in [
    (axes[0], "calls", "Mean observable tool calls / run", "A  Orchestration load", "{:.2f}"),
    (axes[1], "latency_median", "Median total latency (s)", "B  End-to-end latency", "{:.1f}s"),
]:
    vals = [s[key] for s in summaries]
    bars = ax.bar(np.arange(2), vals, color=[f"{BLUE}CC", f"{RED}CC"], edgecolor=[BLUE, RED], linewidth=1.3, width=0.55)
    for b, v in zip(bars, vals):
        ax.text(b.get_x() + b.get_width() / 2, b.get_height() + max(vals) * 0.035, fmt.format(v), ha="center", va="bottom", fontweight="bold")
    ax.set_xticks(np.arange(2), names)
    ax.set_ylabel(ylabel)
    ax.set_title(title)
    ax.grid(axis="y", color="#E5E7EB", linewidth=0.7)
    ax.set_axisbelow(True)
    for spine in ["top", "right"]:
        ax.spines[spine].set_visible(False)
fig.suptitle("Efficiency of the same research-task protocol", y=1.02, fontsize=11, fontweight="bold")
fig.text(0.01, -0.03, "Galen and Codex are both repeated 15-card × 5-run datasets. Tool calls are conservative event-normalized counts.", fontsize=7.4, color=GRAY)
save(fig, "fig2_efficiency")


# Figure 3: per-case pass rates, with coverage shown in the row labels.
cases = [f"PCTX0{i}" for i in range(1, 6)] + [f"PDATA0{i}" for i in range(1, 6)] + [f"PTOOL0{i}" for i in range(1, 6)]
def per_case(rows: list[dict], denom: int | None = None) -> list[float]:
    grouped: dict[str, list[dict]] = defaultdict(list)
    for r in rows:
        grouped[r["case_id"]].append(r)
    out = []
    for c in cases:
        rs = grouped.get(c, [])
        n = denom if denom is not None else len(rs)
        out.append(sum(bool(r["hard_gates_passed"]) for r in rs) / n if n else np.nan)
    return out

matrix = np.array([per_case(galen), per_case(codex), per_case(opencode)])
fig, ax = plt.subplots(figsize=(8.0, 2.35))
im = ax.imshow(matrix, vmin=0, vmax=1, cmap="RdYlGn", aspect="auto")
ax.set_yticks(np.arange(3), ["Galen\n(n=5/card)", "Codex + DS\n(n=5/card)", "OpenCode + DS\n(n=1/card)"])
ax.set_xticks(np.arange(len(cases)), cases, rotation=45, ha="right")
ax.set_title("Per-card hard-gate success (coverage-aware)", fontsize=10.5, fontweight="bold", pad=10)
for i in range(matrix.shape[0]):
    for j in range(matrix.shape[1]):
        val = matrix[i, j]
        if np.isfinite(val):
            text = f"{val:.0%}" if val not in (0, 1) else ("1" if val == 1 else "0")
            ax.text(j, i, text, ha="center", va="center", fontsize=7.3, color="black")
        else:
            ax.text(j, i, "—", ha="center", va="center", fontsize=8, color=GRAY)
ax.set_xlabel("Frozen paired task card")
fig.colorbar(im, ax=ax, fraction=0.018, pad=0.015, label="Pass rate")
fig.subplots_adjust(bottom=0.30)
fig.text(0.01, -0.16, "The OpenCode row is a one-pass pilot; it is not pooled with the repeated Galen/Codex estimates.", fontsize=7.4, color=GRAY)
save(fig, "fig3_failure_boundary")


if __name__ == "__main__":
    print(f"Wrote figures to {OUT}")
