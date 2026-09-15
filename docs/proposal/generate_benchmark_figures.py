from pathlib import Path

import matplotlib.pyplot as plt
from matplotlib import font_manager


OUT = Path(__file__).parent / "images"
OUT.mkdir(exist_ok=True)


def chinese_font() -> str:
    candidates = ["Microsoft YaHei", "SimHei", "Noto Sans CJK SC", "Noto Sans SC"]
    installed = {f.name for f in font_manager.fontManager.ttflist}
    return next((name for name in candidates if name in installed), "DejaVu Sans")


FONT = chinese_font()
plt.rcParams.update({
    "font.family": FONT,
    "axes.unicode_minus": False,
    "font.size": 9,
    "axes.titlesize": 11,
    "axes.labelsize": 9,
    "figure.facecolor": "white",
    "axes.facecolor": "#F7F9FA",
    "axes.edgecolor": "#C9D1D6",
    "axes.spines.top": False,
    "axes.spines.right": False,
    "grid.color": "#DDE3E6",
    "grid.linewidth": 0.7,
})

blue = "#2F6FEB"
red = "#D85757"
ink = "#183042"


fig, ax = plt.subplots(figsize=(7.2, 3.5), layout="constrained")
names = ["Galen", "Codex CLI"]
values = [96.0, 22.7]
bars = ax.bar(names, values, width=0.52, color=["#BFD2FA", "#F3C4C4"],
              edgecolor=[blue, red], linewidth=1.8)
ax.set_ylim(0, 108)
ax.set_ylabel("任务成功率（%）")
ax.set_title("统一任务条件下的科研任务完成率", color=ink, weight="bold")
ax.grid(axis="y", zorder=0)
ax.set_axisbelow(True)
for bar, value, count in zip(bars, values, ["72/75", "17/75"]):
    ax.text(bar.get_x() + bar.get_width() / 2, value + 3,
            f"{value:.1f}%\n{count}", ha="center", va="bottom", weight="bold", color=ink)
ax.text(0.99, 0.02, "同一任务卡 · 同一输入 · 同一评分规则", transform=ax.transAxes,
        ha="right", va="bottom", color="#61717C", fontsize=8)
fig.savefig(OUT / "fig07-framework-success-zh.pdf", bbox_inches="tight")
fig.savefig(OUT / "fig07-framework-success-zh.png", dpi=320, bbox_inches="tight")
plt.close(fig)


fig, axes = plt.subplots(1, 2, figsize=(7.2, 3.5), layout="constrained")
metrics = [
    ("平均可观察工具调用", "调用次数", [1.96, 11.64]),
    ("中位端到端耗时", "秒", [9.8, 32.2]),
]
for ax, (title, ylabel, values) in zip(axes, metrics):
    bars = ax.bar(names, values, width=0.55, color=["#BFD2FA", "#F3C4C4"],
                  edgecolor=[blue, red], linewidth=1.6)
    ax.set_title(title, color=ink, weight="bold")
    ax.set_ylabel(ylabel)
    ax.grid(axis="y")
    ax.set_axisbelow(True)
    ax.set_ylim(0, max(values) * 1.25)
    for bar, value in zip(bars, values):
        ax.text(bar.get_x() + bar.get_width() / 2, value + max(values) * 0.04,
                f"{value:g}", ha="center", va="bottom", weight="bold", color=ink)
fig.suptitle("相同科研任务协议下的执行经济性", color=ink, weight="bold", fontsize=12)
fig.savefig(OUT / "fig08-framework-efficiency-zh.pdf", bbox_inches="tight")
fig.savefig(OUT / "fig08-framework-efficiency-zh.png", dpi=320, bbox_inches="tight")
plt.close(fig)

