"""Create a publication-ready diagnostic chart for the Galen smoke runs.

The figure is intentionally diagnostic: every source run is a one-repeat-per-card
smoke, so the chart reports pass counts and mean tool calls without implying a
stable treatment effect.  The script writes editable SVG/PDF plus a 300-DPI PNG
and a compact source-data snapshot next to the figure.
"""

from __future__ import annotations

import json
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.patches import Patch


ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "output" / "pdf" / "galen-architecture-v0.2" / "figures"
OUT.mkdir(parents=True, exist_ok=True)

VARIANTS = [
    ("full_galen", "Full Galen", "full"),
    ("stateless", "Stateless", "ablation"),
    ("no_data_contract", "No data\ncontract", "ablation"),
    ("no_exec_contract", "No exec\ncontract", "ablation"),
    ("no_evidence_link", "No evidence\nlink", "ablation"),
    ("generic_same_runtime", "Generic\nruntime", "baseline"),
]

RUNS = {
    "context": [
        ROOT
        / "evals/runs/architecture-v1/architecture-v1-pctx01-public-context-smoke-r1b/ablation-summary.json",
        ROOT
        / "evals/runs/architecture-v1/architecture-v1-pctx04-05-public-smoke-r1/ablation-summary.json",
    ],
    "data_tool": [
        ROOT
        / "evals/runs/architecture-v1/architecture-v1-data-tool-representative-smoke-r1/ablation-summary.json",
    ],
}

COLORS = {
    "full": "#0F4D92",
    "ablation": "#B64342",
    "baseline": "#767676",
}
HATCHES = {"full": "", "ablation": "///", "baseline": ".."}


def load_summary(path: Path) -> dict:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def aggregate(paths: list[Path]) -> dict[str, dict[str, float]]:
    totals = {
        key: {"n": 0, "successes": 0, "tools_weighted": 0.0, "requests_weighted": 0.0}
        for key, _, _ in VARIANTS
    }
    for path in paths:
        payload = load_summary(path)
        for key, _, _ in VARIANTS:
            row = payload["variants"][key]
            item = totals[key]
            item["n"] += int(row["n"])
            item["successes"] += int(row["successes"])
            item["tools_weighted"] += float(row["mean_tools"]) * int(row["n"])
            item["requests_weighted"] += float(row["mean_model_requests"]) * int(row["n"])
    for item in totals.values():
        item["pass_rate"] = item["successes"] / item["n"] if item["n"] else 0.0
        item["mean_tools"] = item["tools_weighted"] / item["n"] if item["n"] else 0.0
        item["mean_model_requests"] = item["requests_weighted"] / item["n"] if item["n"] else 0.0
    return totals


def style_axes(ax: plt.Axes) -> None:
    ax.spines["top"].set_visible(False)
    ax.spines["right"].set_visible(False)
    ax.spines["left"].set_color("#333333")
    ax.spines["bottom"].set_color("#333333")
    ax.tick_params(axis="both", colors="#333333", labelsize=8, length=3)
    ax.grid(axis="x", color="#E8E8E8", linewidth=0.7, zorder=0)
    ax.set_axisbelow(True)


def draw_bars(ax: plt.Axes, rows: dict[str, dict[str, float]], metric: str, xmax: float, fmt: str) -> None:
    labels = [label for _, label, _ in VARIANTS]
    values = [rows[key][metric] for key, _, _ in VARIANTS]
    y = list(range(len(values)))
    for yi, (key, _, role), value in zip(y, VARIANTS, values):
        ax.barh(
            yi,
            value,
            height=0.58,
            color=COLORS[role],
            edgecolor="#222222",
            linewidth=0.65,
            hatch=HATCHES[role],
            alpha=0.92,
            zorder=3,
        )
        count = f"{rows[key]['successes']}/{rows[key]['n']}" if metric == "pass_rate" else fmt.format(value)
        ax.text(
            min(value + xmax * 0.018, xmax * 0.965),
            yi,
            count,
            va="center",
            ha="left",
            fontsize=8,
            color="#202020",
            fontweight="bold" if key == "full_galen" else "normal",
        )
    ax.set_yticks(y, labels)
    ax.invert_yaxis()
    ax.set_xlim(0, xmax)
    style_axes(ax)


def main() -> None:
    context = aggregate(RUNS["context"])
    data_tool = aggregate(RUNS["data_tool"])

    plt.rcParams.update(
        {
            "font.family": "sans-serif",
            "font.sans-serif": ["Arial", "Helvetica", "DejaVu Sans"],
            "font.size": 9,
            "axes.titleweight": "bold",
            "svg.fonttype": "none",
            "pdf.fonttype": 42,
            "pdf.use14corefonts": False,
        }
    )

    fig, axes = plt.subplots(1, 2, figsize=(7.05, 3.35), gridspec_kw={"wspace": 0.42})
    draw_bars(axes[0], context, "pass_rate", 1.08, "{:.2f}")
    axes[0].set_title("(a) Context integrity", loc="left", fontsize=10, pad=8)
    axes[0].set_xlabel("Hard-gate pass rate", fontsize=8.5)
    axes[0].set_xticks([0, 0.25, 0.5, 0.75, 1.0], ["0", ".25", ".50", ".75", "1.0"])

    draw_bars(axes[1], data_tool, "mean_tools", 10.8, "{:.1f}")
    axes[1].set_title("(b) Execution cost", loc="left", fontsize=10, pad=8)
    axes[1].set_xlabel("Mean tool calls per run", fontsize=8.5)
    axes[1].set_xticks([0, 2, 4, 6, 8, 10], ["0", "2", "4", "6", "8", "10"])

    legend = [
        Patch(facecolor=COLORS["full"], edgecolor="#222222", label="Full Galen"),
        Patch(facecolor=COLORS["ablation"], edgecolor="#222222", hatch=HATCHES["ablation"], label="Ablation"),
        Patch(facecolor=COLORS["baseline"], edgecolor="#222222", hatch=HATCHES["baseline"], label="Generic runtime"),
    ]
    fig.legend(
        handles=legend,
        loc="upper center",
        bbox_to_anchor=(0.5, 1.02),
        ncol=3,
        frameon=False,
        fontsize=8,
        handlelength=1.4,
        columnspacing=1.2,
    )
    fig.subplots_adjust(left=0.22, right=0.98, bottom=0.18, top=0.78)

    stem = OUT / "fig2_architecture_smoke"
    fig.savefig(stem.with_suffix(".pdf"), bbox_inches="tight", pad_inches=0.06)
    fig.savefig(stem.with_suffix(".svg"), bbox_inches="tight", pad_inches=0.06)
    fig.savefig(stem.with_suffix(".png"), dpi=300, bbox_inches="tight", pad_inches=0.06, facecolor="white")
    plt.close(fig)

    snapshot = {
        "figure_id": "fig2_architecture_smoke",
        "protocol": "public-input smoke; one run per card",
        "sources": [str(path.relative_to(ROOT)).replace("\\", "/") for group in RUNS.values() for path in group],
        "context": context,
        "data_tool": data_tool,
    }
    (OUT / "fig2_architecture_smoke_source.json").write_text(
        json.dumps(snapshot, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )


if __name__ == "__main__":
    main()
