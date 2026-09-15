"""Render the Chinese benchmark figures used by the Context Engine report.

Values are computed directly from the frozen evaluation run records.
"""
from __future__ import annotations

import json
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

ROOT = Path(__file__).resolve().parents[5]
ASSETS = Path(__file__).resolve().parent / "assets"
BLUE, RED, GRID = "#2563EB", "#DC2626", "#E5E7EB"

plt.rcParams.update({
    "font.sans-serif": ["Microsoft YaHei", "SimHei", "DejaVu Sans"],
    "axes.unicode_minus": False,
    "font.size": 10,
    "axes.labelsize": 10,
    "axes.titlesize": 12,
    "pdf.fonttype": 42,
    "ps.fonttype": 42,
})


def load(name: str) -> list[dict]:
    path = ROOT / "evals" / "runs" / name
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def summary(rows: list[dict]) -> tuple[int, int, float, float]:
    passed = sum(bool(row["hard_gates_passed"]) for row in rows)
    calls = float(np.mean([row["tools"]["calls"] for row in rows]))
    latency = float(np.median([row["latency"]["total_ms"] for row in rows]) / 1000)
    return len(rows), passed, calls, latency


galen = summary(load("large-full-galen-20260912-final.jsonl"))
baseline = summary(load("large-codex-deepseek-flash-20260912-complete.jsonl"))
labels = ["Galen\nContext Engine", "基线 CLI Agent\n(Codex + DeepSeek)"]


def finish(ax: plt.Axes) -> None:
    ax.grid(axis="y", color=GRID, linewidth=0.8)
    ax.set_axisbelow(True)
    ax.spines[["top", "right"]].set_visible(False)


def save(fig: plt.Figure, name: str) -> None:
    ASSETS.mkdir(exist_ok=True)
    for ext, dpi in (("pdf", None), ("png", 420)):
        kwargs = {"bbox_inches": "tight", "facecolor": "white"}
        if dpi:
            kwargs["dpi"] = dpi
        fig.savefig(ASSETS / f"{name}.{ext}", **kwargs)
    plt.close(fig)


# Hard-gate completion: only the matched 75-run comparison appears in the report.
fig, ax = plt.subplots(figsize=(6.4, 3.7), layout="constrained")
rates = [galen[1] / galen[0] * 100, baseline[1] / baseline[0] * 100]
bars = ax.bar(np.arange(2), rates, width=0.54,
              color=["#2563EBCC", "#DC2626CC"], edgecolor=[BLUE, RED], linewidth=1.5)
for bar, (n, passed, _, _), rate in zip(bars, [galen, baseline], rates):
    ax.text(bar.get_x() + bar.get_width() / 2, rate + 3.2, f"{passed}/{n}\n{rate:.1f}%",
            ha="center", va="bottom", fontsize=11, fontweight="bold")
ax.set_ylim(0, 112)
ax.set_ylabel("硬门通过率（%）")
ax.set_xticks(np.arange(2), labels)
ax.set_title("冻结配对任务卡上的端到端完成率", fontweight="bold", pad=12)
ax.text(0.5, -0.24, "同一批 15 张任务卡 × 每卡 5 次重复；通过需同时满足必需事实、工具预算与产物验收。",
        transform=ax.transAxes, ha="center", va="top", color="#4B5563", fontsize=8.7)
finish(ax)
save(fig, "fig07-framework-success-zh")


# Cost and latency form a single, tightly matched two-panel figure.
fig, axes = plt.subplots(1, 2, figsize=(7.4, 3.45), layout="constrained")
for ax, values, ylabel, title, fmt in [
    (axes[0], [galen[2], baseline[2]], "平均可观察工具调用（次/运行）", "A  编排开销", "{:.2f}"),
    (axes[1], [galen[3], baseline[3]], "中位端到端时延（秒）", "B  完成时延", "{:.1f} s"),
]:
    bars = ax.bar(np.arange(2), values, width=0.55,
                  color=["#2563EBCC", "#DC2626CC"], edgecolor=[BLUE, RED], linewidth=1.5)
    for bar, value in zip(bars, values):
        ax.text(bar.get_x() + bar.get_width() / 2, value + max(values) * 0.045, fmt.format(value),
                ha="center", va="bottom", fontweight="bold")
    ax.set_xticks(np.arange(2), ["Galen", "基线 CLI Agent"])
    ax.set_ylabel(ylabel)
    ax.set_title(title, fontweight="bold")
    finish(ax)
fig.suptitle("同一科研任务协议下的执行效率", y=1.03, fontsize=13, fontweight="bold")
save(fig, "fig08-framework-efficiency-zh")
