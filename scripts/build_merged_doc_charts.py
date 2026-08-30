# -*- coding: utf-8 -*-
"""
Galen 完整文档（合并升级版）——全部图表生成脚本
数据来源：output/pdf 下 5 份 PDF 的聚合数值（原值来自 evals/runs 探针 JSON）。
运行环境：系统 Python 3.10（matplotlib）。输出：tmp/charts/*.png
"""
from __future__ import annotations

from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.font_manager as fm
import matplotlib.pyplot as plt
import numpy as np

ROOT = Path(__file__).resolve().parents[1]

# ---- 统一视觉（与项目 reportlab 配色一致） ----
INK = "#17382F"
TEXT = "#405C54"
MUTED = "#7A8E87"
GREEN = "#07866F"
GREEN_2 = "#43AF92"
MINT = "#DDF3EC"
PALE = "#F3F7F5"
LINE = "#D7E3DF"
ORANGE = "#D98242"
RED = "#C65A56"
GRID = "#E4ECE8"

# Anthropic 品牌色主题：Dark #141413 / Light #FAF9F5 / MidGray #B0AEA5 /
# LightGray #E8E6DC / Orange #D97757 / Blue #6A9BCC / Green #788C5D
THEMES = {
    "galen": dict(INK=INK, TEXT=TEXT, MUTED=MUTED, GREEN=GREEN, GREEN_2=GREEN_2,
                  MINT=MINT, PALE=PALE, LINE=LINE, ORANGE=ORANGE, RED=RED, GRID=GRID),
    "anthropic": dict(
        INK="#141413", TEXT="#3D3D3A", MUTED="#8F8D88",
        GREEN="#D97757", GREEN_2="#6A9BCC",
        MINT="#EDEAE2", PALE="#FAF9F5", LINE="#E8E6DC",
        ORANGE="#D97757", RED="#B4552D", GRID="#E0DCD2",
    ),
}

OUT_DIR = ROOT / "tmp" / "charts"


def set_theme(name: str) -> None:
    global OUT_DIR, INK, TEXT, MUTED, GREEN, GREEN_2, MINT, PALE, LINE, ORANGE, RED, GRID
    t = THEMES[name]
    for k in ("INK", "TEXT", "MUTED", "GREEN", "GREEN_2", "MINT", "PALE", "LINE", "ORANGE", "RED", "GRID"):
        globals()[k] = t[k]
    OUT_DIR = ROOT / "tmp" / ("charts-anthropic" if name == "anthropic" else "charts")

FONT = r"C:\Windows\Fonts\msyh.ttc"
fm.fontManager.addfont(FONT)
_FONT_NAME = fm.FontProperties(fname=FONT).get_name()

plt.rcParams.update({
    "font.family": _FONT_NAME,
    "font.size": 9,
    "axes.edgecolor": LINE,
    "axes.linewidth": 0.8,
    "axes.titlesize": 10.5,
    "axes.titleweight": "bold",
    "axes.titlecolor": INK,
    "axes.labelcolor": TEXT,
    "xtick.color": TEXT,
    "ytick.color": TEXT,
    "figure.facecolor": "white",
    "axes.facecolor": "white",
    "savefig.facecolor": "white",
})

FIG_W = 7.0  # 英寸，对应 A4 内容宽度约 172mm
DPI = 200


def style_ax(ax, ylabel: str = "", xlabel: str = ""):
    ax.set_ylabel(ylabel, fontsize=8.5)
    ax.set_xlabel(xlabel, fontsize=8.5)
    ax.spines["top"].set_visible(False)
    ax.spines["right"].set_visible(False)
    ax.tick_params(colors=TEXT, labelsize=8)
    ax.grid(axis="y", color=GRID, linewidth=0.7, zorder=0)
    ax.set_axisbelow(True)


def value_label(ax, bars, fmt="{:.0f}", fs=7.5, dy=1.5, color=TEXT):
    for b in bars:
        h = b.get_height()
        ax.text(b.get_x() + b.get_width() / 2, h + dy, fmt.format(h),
                ha="center", va="bottom", fontsize=fs, color=color, fontweight="bold")


# ---------------------------------------------------------------- 1
def chart_speed_improvement():
    """改进前后：平均总耗时（E03/E07/E09 × Flash/Pro）"""
    cases = ["E03 Flash", "E03 Pro", "E07 Flash", "E07 Pro", "E09 Flash", "E09 Pro"]
    before = [5100, 9971, 12053, 23102, 15553, 11983]
    after = [4063, 7079, 11315, 14465, 11816, 12022]
    changes = [-20, -29, -6, -37, -24, +0.3]

    fig, ax = plt.subplots(figsize=(FIG_W, 2.55))
    x = np.arange(len(cases))
    w = 0.36
    b1 = ax.bar(x - w / 2, before, w, color=MINT, edgecolor=GREEN_2, linewidth=0.8, label="改进前", zorder=3)
    b2 = ax.bar(x + w / 2, after, w, color=GREEN, label="改进后", zorder=3)
    for i, ch in enumerate(changes):
        ax.text(i + w / 2, after[i] + 400, f"{ch:+.1f}%", ha="center", va="bottom",
                fontsize=7.5, color=RED if ch > 0 else GREEN, fontweight="bold")
    ax.set_xticks(x)
    ax.set_xticklabels(cases, fontsize=7.8)
    ax.set_ylim(0, 27500)
    style_ax(ax, ylabel="平均总耗时（ms）")
    ax.legend(loc="upper left", fontsize=7.5, frameon=False, ncol=2)
    fig.tight_layout()
    fig.savefig(OUT_DIR / "chart_speed_improvement.png", dpi=DPI)
    plt.close(fig)


# ---------------------------------------------------------------- 2/3
def _percentile_chart(filename, title, data, logy=False, note=None):
    """K=20 长尾分位数：P50 / P95 / P99-Max"""
    labels = list(data.keys())
    p50 = [v["p50"] for v in data.values()]
    p95 = [v["p95"] for v in data.values()]
    p99 = [v["p99"] for v in data.values()]

    fig, ax = plt.subplots(figsize=(FIG_W, 2.7))
    x = np.arange(len(labels))
    w = 0.26
    colors = [GREEN_2, GREEN, INK]
    for idx, (vals, name, color) in enumerate(zip([p50, p95, p99], ["P50", "P95", "P99 / Max"], colors)):
        bars = ax.bar(x + (idx - 1) * w, vals, w, color=color, label=name, zorder=3)
        for b, v in zip(bars, vals):
            ax.text(b.get_x() + b.get_width() / 2, v * (1.25 if logy else 1) + (60 if logy else 350),
                    f"{v/1000:.1f}s" if v >= 1000 else f"{v:.0f}ms", ha="center", va="bottom",
                    fontsize=6.6, color=TEXT, rotation=0)
    if logy:
        ax.set_yscale("log")
    ax.set_xticks(x)
    ax.set_xticklabels(labels, fontsize=8)
    ax.set_ylim(10**2 if logy else 0, 10**5.2 if logy else max(p99) * 1.3)
    style_ax(ax, ylabel="耗时（ms，对数轴）" if logy else "耗时（ms）")
    ax.legend(loc="upper left", fontsize=7.5, frameon=False, ncol=3)
    ax.set_title(title, pad=8)
    if note:
        ax.text(0.99, 0.02, note, transform=ax.transAxes, ha="right", va="bottom",
                fontsize=6.8, color=MUTED)
    fig.tight_layout()
    fig.savefig(OUT_DIR / filename, dpi=DPI)
    plt.close(fig)


def charts_percentiles():
    ttfr = {
        "E07 Flash": {"p50": 711, "p95": 22318, "p99": 22942},
        "E07 Pro": {"p50": 1217, "p95": 2135, "p99": 2223},
        "E09 Flash": {"p50": 739, "p95": 1137, "p99": 1310},
        "E09 Pro": {"p50": 1110, "p95": 2200, "p99": 23135},
    }
    total = {
        "E07 Flash": {"p50": 12620, "p95": 32947, "p99": 35125},
        "E07 Pro": {"p50": 18615, "p95": 25148, "p99": 27162},
        "E09 Flash": {"p50": 11958, "p95": 13780, "p99": 14639},
        "E09 Pro": {"p50": 13975, "p95": 18458, "p99": 34246},
    }
    _percentile_chart("chart_ttfr_percentiles.png",
                      "K=20 长尾验证 · 首个可读响应 TTFR 分位数（n=20/组）", ttfr, logy=True,
                      note="E07 Flash 与 E09 Pro 各出现约 23s 尖峰（P99），跨模型偶发")
    _percentile_chart("chart_total_percentiles.png",
                      "K=20 长尾验证 · 端到端完成时间分位数（n=20/组）", total,
                      note="P50 全部 < 19s；P95/P99 反映用户记住的坏体验")


# ---------------------------------------------------------------- 4
def chart_token_convergence():
    """Token 与执行收敛：Input/Output mean（堆叠）"""
    labels = ["E07 Flash", "E07 Pro", "E09 Flash", "E09 Pro"]
    inp = [5937, 5747, 5637, 5172]
    out = [1198, 1017, 1221, 728]

    fig, ax = plt.subplots(figsize=(FIG_W, 2.55))
    x = np.arange(len(labels))
    b1 = ax.bar(x, inp, 0.5, color=INK, label="输入 Token（mean）", zorder=3)
    b2 = ax.bar(x, out, 0.5, bottom=inp, color=GREEN_2, label="输出 Token（mean）", zorder=3)
    for i, (a, b) in enumerate(zip(inp, out)):
        ax.text(i, a + b + 130, f"{a + b:,}", ha="center", fontsize=7.5, color=INK, fontweight="bold")
        ax.text(i, b - 40, f"{b:,}", ha="center", fontsize=6.8, color="white")
    ax.set_xticks(x)
    ax.set_xticklabels(labels, fontsize=8)
    ax.set_ylim(0, 8000)
    style_ax(ax, ylabel="Token")
    ax.legend(loc="upper right", fontsize=7.5, frameon=False)
    ax.set_title("Token 与执行收敛 · 每次任务固定 2 次模型请求 / 1 次 write_file，工具错误 0", pad=8)
    fig.tight_layout()
    fig.savefig(OUT_DIR / "chart_token_convergence.png", dpi=DPI)
    plt.close(fig)


# ---------------------------------------------------------------- 5
def chart_probe_performance():
    """闭环探针 Run1 / Run2：TTFR、总耗时、Token"""
    fig, axes = plt.subplots(1, 3, figsize=(FIG_W, 2.5), gridspec_kw={"width_ratios": [1, 1, 1.35]})
    runs = ["Run 1", "Run 2"]

    ttfr = [1.187, 0.438]
    total = [32.55, 34.05]
    inp = [10572, 10515]
    out = [1820, 1924]

    # TTFR
    ax = axes[0]
    bars = ax.bar(runs, ttfr, 0.5, color=[GREEN_2, GREEN], zorder=3)
    value_label(ax, bars, fmt="{:.3f}", dy=0.04)
    ax.set_title("首个可读响应 TTFR", pad=7)
    style_ax(ax, ylabel="秒")
    ax.set_ylim(0, 1.6)

    # 总耗时
    ax = axes[1]
    bars = ax.bar(runs, total, 0.5, color=[GREEN_2, GREEN], zorder=3)
    value_label(ax, bars, fmt="{:.2f}", dy=0.6)
    ax.set_title("端到端完成时间", pad=7)
    style_ax(ax, ylabel="秒")
    ax.set_ylim(0, 42)

    # Token
    ax = axes[2]
    b1 = ax.bar(runs, inp, 0.5, color=INK, label="输入", zorder=3)
    b2 = ax.bar(runs, out, 0.5, bottom=inp, color=GREEN_2, label="输出", zorder=3)
    for i in range(2):
        ax.text(i, inp[i] + out[i] + 250, f"{inp[i]+out[i]:,}", ha="center", fontsize=7.5,
                color=INK, fontweight="bold")
    ax.set_title("Token 构成（输入 + 输出）", pad=7)
    style_ax(ax, ylabel="Token")
    ax.set_ylim(0, 15000)
    ax.legend(fontsize=7, frameon=False, loc="upper right")

    fig.suptitle("两次真实 Pro 探针 · 相同场景 / 相同模型 / 独立临时工作区", fontsize=10, color=INK, fontweight="bold", y=1.02)
    fig.tight_layout()
    fig.savefig(OUT_DIR / "chart_probe_performance.png", dpi=DPI, bbox_inches="tight")
    plt.close(fig)


# ---------------------------------------------------------------- 6/7
def charts_six_probes():
    """上下文优化：六组探针"""
    stages = ["记忆基础", "工具契约", "Flash 高思考", "Flash 优化后", "Pro 无预算", "Pro 快速通道"]
    dur = [59.7, 91.6, 91.5, 52.7, 82.5, 57.5]
    inp = [18813, 23177, 26610, 17451, 28386, 22472]
    out = [5327, 8348, 7339, 4494, 3628, 2821]
    requests = [6, 6, 7, 5, 6, 6]
    hl = [False, False, False, True, False, True]
    colors_d = [GREEN if h else MINT for h in hl]
    edge_d = [GREEN if h else GREEN_2 for h in hl]

    # 耗时
    fig, ax = plt.subplots(figsize=(FIG_W, 2.7))
    bars = ax.bar(stages, dur, 0.55, color=colors_d, edgecolor=edge_d, linewidth=0.9, zorder=3)
    value_label(ax, bars, fmt="{:.1f}", dy=1.2)
    ax.set_title("六组探针 · 总耗时（同一 9/9 质量门）", pad=8)
    style_ax(ax, ylabel="秒")
    ax.set_ylim(0, 108)
    ax.tick_params(axis="x", labelsize=7.6, rotation=12)
    fig.tight_layout()
    fig.savefig(OUT_DIR / "chart_six_probes_duration.png", dpi=DPI)
    plt.close(fig)

    # Token
    fig, ax = plt.subplots(figsize=(FIG_W, 2.9))
    x = np.arange(len(stages))
    w = 0.36
    b1 = ax.bar(x - w / 2, inp, w, color=INK, label="输入 Token", zorder=3)
    b2 = ax.bar(x + w / 2, out, w, color=GREEN_2, label="输出 Token", zorder=3)
    for i, v in enumerate(inp):
        ax.text(i - w / 2, v + 350, f"{v:,}", ha="center", fontsize=6.6, color=INK)
    for i, v in enumerate(out):
        ax.text(i + w / 2, v + 350, f"{v:,}", ha="center", fontsize=6.6, color=GREEN)
    ax.set_xticks(x)
    ax.set_xticklabels(stages, fontsize=7.6, rotation=12)
    ax.set_ylim(0, 33000)
    style_ax(ax, ylabel="Token")
    ax.legend(fontsize=7.5, frameon=False, loc="upper left")
    ax.set_title("六组探针 · 输入 / 输出 Token", pad=8)
    fig.tight_layout()
    fig.savefig(OUT_DIR / "chart_six_probes_tokens.png", dpi=DPI)
    plt.close(fig)


# ---------------------------------------------------------------- 8
def chart_flash_relative():
    """Flash 优化后相对性能（高思考 = 100）"""
    metrics = ["总耗时", "输入 Token", "输出 Token", "模型请求"]
    after = [57.6, 65.6, 61.2, 71.4]
    drops = [42, 34, 39, 29]

    fig, ax = plt.subplots(figsize=(FIG_W, 2.6))
    y = np.arange(len(metrics))[::-1]
    ax.barh(y, [100] * len(metrics), 0.42, color=MINT, edgecolor=GREEN_2, linewidth=0.8, label="Flash 高思考（基线 100）", zorder=3)
    ax.barh(y + 0.42, after, 0.42, color=GREEN, label="Flash 优化后（相对值）", zorder=3)
    for yi, a, d in zip(y, after, drops):
        ax.text(100.5, yi + 0.42, f"{a:.1f}", va="center", fontsize=8, color=GREEN, fontweight="bold")
        ax.text(2, yi + 0.42, f"↓ {d}%", va="center", fontsize=7.5, color="white", fontweight="bold")
    ax.set_yticks(y + 0.21)
    ax.set_yticklabels(metrics, fontsize=8.5)
    ax.set_xlim(0, 118)
    ax.set_xlabel("相对值（%）", fontsize=8.5)
    ax.spines["top"].set_visible(False)
    ax.spines["right"].set_visible(False)
    ax.tick_params(colors=TEXT, labelsize=8)
    ax.set_title("Flash 优化后 · 同一 9/9 质量门下的相对性能", pad=8)
    ax.legend(fontsize=7.5, frameon=False, loc="lower right")
    fig.tight_layout()
    fig.savefig(OUT_DIR / "chart_flash_relative.png", dpi=DPI)
    plt.close(fig)


# ---------------------------------------------------------------- 9
def chart_cache_hit():
    """Flash 优化后 · 逐请求缓存命中率"""
    reqs = [f"R{i+1}" for i in range(5)]
    hits = [39, 96, 23, 30, 25]  # 96% 一次，其余 23-39%
    colors = [MINT] * 5
    colors[1] = GREEN
    fig, ax = plt.subplots(figsize=(FIG_W, 2.3))
    bars = ax.bar(reqs, hits, 0.5, color=colors, edgecolor=[GREEN_2] * 5, linewidth=0.8, zorder=3)
    value_label(ax, bars, fmt="{:.0f}", dy=1.5)
    ax.set_ylim(0, 112)
    style_ax(ax, ylabel="缓存命中率（%）")
    ax.set_title("Flash 优化后 · 各模型请求的缓存命中率（前缀命中，非会话总量）", pad=8)
    ax.text(0.98, 0.95, "一次请求 96%，其余 23%–39%：缓存可观测，但动态尾部仍可能破坏稳定前缀",
            transform=ax.transAxes, ha="right", va="top", fontsize=7, color=MUTED)
    fig.tight_layout()
    fig.savefig(OUT_DIR / "chart_cache_hit.png", dpi=DPI)
    plt.close(fig)


# ---------------------------------------------------------------- 10
def chart_research_flow():
    """产品使用说明 · 科研任务闭环流程图"""
    steps = [
        ("输入科研任务", "自然语言描述\n研究目标"),
        ("确认计划", "AI 生成计划\n人工把关"),
        ("节点自动执行", "检索 / 提取\n分析 / 写作"),
        ("自动整合成文", "证据链整合\n生成成果"),
        ("成果预览", "Galen 内审阅\n产物库登记"),
    ]
    fig, ax = plt.subplots(figsize=(FIG_W, 1.95))
    ax.axis("off")
    n = len(steps)
    for i, (title, sub) in enumerate(steps):
        x = i / (n - 1)
        color = GREEN if i in (0, 4) else INK
        box = ax.text(x, 0.5, f"{title}\n{sub}", ha="center", va="center", fontsize=8.2,
                      color="white", fontweight="bold",
                      bbox=dict(boxstyle="round,pad=0.55", fc=color, ec="none"))
        if i < n - 1:
            ax.annotate("", xy=(x + 1 / (n - 1) - 0.035, 0.5), xytext=(x + 0.035, 0.5),
                        arrowprops=dict(arrowstyle="-|>", color=GREEN_2, lw=1.8))
    ax.set_title("科研任务闭环：从“想法”到“可预览成果”自动推进，人工只需在关键节点把关", fontsize=10, color=INK, pad=10)
    fig.tight_layout()
    fig.savefig(OUT_DIR / "chart_research_flow.png", dpi=DPI)
    plt.close(fig)


def main():
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--theme", choices=list(THEMES), default="galen")
    args = parser.parse_args()
    set_theme(args.theme)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    chart_speed_improvement()
    charts_percentiles()
    chart_token_convergence()
    chart_probe_performance()
    charts_six_probes()
    chart_flash_relative()
    chart_cache_hit()
    chart_research_flow()
    for p in sorted(OUT_DIR.glob("*.png")):
        print(p, p.stat().st_size)
    print("charts done ->", OUT_DIR)


if __name__ == "__main__":
    main()
