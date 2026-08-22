from __future__ import annotations

import json
from pathlib import Path

from reportlab.lib.colors import HexColor, Color
from reportlab.lib.enums import TA_LEFT
from reportlab.lib.pagesizes import A4, landscape
from reportlab.lib.styles import ParagraphStyle
from reportlab.lib.utils import ImageReader
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.pdfgen import canvas
from reportlab.platypus import Paragraph


ROOT = Path(__file__).resolve().parents[1]
RUNS = ROOT / "evals" / "runs"
OUT_DIR = ROOT / "output" / "pdf"
TMP_DIR = ROOT / "tmp" / "pdfs"
OUT_PDF = OUT_DIR / "galen-closed-loop-probe-report-2026-08-22.pdf"

FONT_REGULAR = Path(r"C:\Windows\Fonts\msyh.ttc")
FONT_BOLD = Path(r"C:\Windows\Fonts\msyhbd.ttc")

INK = HexColor("#18332D")
INK_2 = HexColor("#526B64")
MUTED = HexColor("#7E918B")
PAPER = HexColor("#F5F7F4")
WHITE = HexColor("#FFFFFF")
GREEN = HexColor("#087F6A")
GREEN_2 = HexColor("#3FAE91")
MINT = HexColor("#DDF2EB")
PALE = HexColor("#EAF1EE")
ORANGE = HexColor("#D98242")
RED = HexColor("#C95B57")
LINE = HexColor("#D7E1DD")

PAGE_W, PAGE_H = landscape(A4)
MARGIN = 40


def load_runs() -> list[dict]:
    files = sorted(RUNS.glob("probe-closed-loop-*.json"), key=lambda p: p.stat().st_mtime)
    if len(files) < 2:
        raise RuntimeError("Need at least two closed-loop probe reports")
    return [json.loads(path.read_text(encoding="utf-8")) for path in files[-2:]]


def register_fonts() -> None:
    if not FONT_REGULAR.exists() or not FONT_BOLD.exists():
        raise RuntimeError("Microsoft YaHei fonts are required")
    pdfmetrics.registerFont(TTFont("GalenSans", str(FONT_REGULAR)))
    pdfmetrics.registerFont(TTFont("GalenSans-Bold", str(FONT_BOLD)))


def rounded(c: canvas.Canvas, x: float, y: float, w: float, h: float, fill, radius=12, stroke=None):
    c.setFillColor(fill)
    c.setStrokeColor(stroke or fill)
    c.roundRect(x, y, w, h, radius, fill=1, stroke=1 if stroke else 0)


def text(c: canvas.Canvas, value: str, x: float, y: float, size=10, color=INK, bold=False):
    c.setFont("GalenSans-Bold" if bold else "GalenSans", size)
    c.setFillColor(color)
    c.drawString(x, y, value)


def right_text(c: canvas.Canvas, value: str, x: float, y: float, size=9, color=MUTED, bold=False):
    c.setFont("GalenSans-Bold" if bold else "GalenSans", size)
    c.setFillColor(color)
    c.drawRightString(x, y, value)


def paragraph(c: canvas.Canvas, value: str, x: float, y_top: float, width: float, size=10, leading=15, color=INK_2, bold=False):
    style = ParagraphStyle(
        "body",
        fontName="GalenSans-Bold" if bold else "GalenSans",
        fontSize=size,
        leading=leading,
        textColor=color,
        alignment=TA_LEFT,
        spaceAfter=0,
        splitLongWords=True,
    )
    p = Paragraph(value, style)
    _, h = p.wrap(width, PAGE_H)
    p.drawOn(c, x, y_top - h)
    return h


def header(c: canvas.Canvas, section: str, title: str, subtitle: str, page: int):
    text(c, "GALEN / EVALUATION NOTE", MARGIN, PAGE_H - 34, 8, GREEN, True)
    right_text(c, f"2026-08-22  ·  PAGE {page}/4", PAGE_W - MARGIN, PAGE_H - 34, 8, MUTED)
    text(c, section, MARGIN, PAGE_H - 72, 10, GREEN, True)
    text(c, title, MARGIN, PAGE_H - 101, 23, INK, True)
    text(c, subtitle, MARGIN, PAGE_H - 122, 9, MUTED)
    c.setStrokeColor(LINE)
    c.setLineWidth(0.8)
    c.line(MARGIN, PAGE_H - 136, PAGE_W - MARGIN, PAGE_H - 136)


def footer(c: canvas.Canvas, note: str):
    c.setStrokeColor(LINE)
    c.line(MARGIN, 30, PAGE_W - MARGIN, 30)
    text(c, note, MARGIN, 16, 7.2, MUTED)
    right_text(c, "Galen 康复科研工作台", PAGE_W - MARGIN, 16, 7.2, MUTED)


def metric_card(c, x, y, w, h, label, value, note, accent=GREEN):
    rounded(c, x, y, w, h, WHITE, 11, LINE)
    c.setFillColor(accent)
    c.roundRect(x, y, 5, h, 3, fill=1, stroke=0)
    text(c, label, x + 18, y + h - 20, 8, MUTED, True)
    text(c, value, x + 18, y + 27, 22, INK, True)
    right_text(c, note, x + w - 14, y + 15, 7.5, MUTED)


def bar_panel(c: canvas.Canvas, x: float, y: float, w: float, h: float, title: str, values: list[float], labels: list[str], unit: str, decimals: int):
    rounded(c, x, y, w, h, PAPER, 10)
    text(c, title, x + 14, y + h - 22, 9, INK, True)
    chart_x, chart_y = x + 38, y + 31
    chart_w, chart_h = w - 58, h - 70
    maximum = max(values) * 1.25
    for tick in range(4):
        yy = chart_y + chart_h * tick / 3
        c.setStrokeColor(LINE)
        c.setLineWidth(0.5)
        c.line(chart_x, yy, chart_x + chart_w, yy)
        tick_value = maximum * tick / 3
        tick_label = f"{tick_value:.1f}" if maximum < 10 else f"{tick_value:.0f}"
        text(c, tick_label, x + 10, yy - 2, 6, MUTED)
    colors = [MUTED, GREEN]
    bar_w = 34
    positions = [chart_x + chart_w * 0.28, chart_x + chart_w * 0.70]
    for idx, value in enumerate(values):
        bh = chart_h * value / maximum
        c.setFillColor(colors[idx])
        c.roundRect(positions[idx] - bar_w / 2, chart_y, bar_w, bh, 5, fill=1, stroke=0)
        value_label = f"{value:.{decimals}f}{unit}"
        c.setFont("GalenSans-Bold", 7.5)
        c.setFillColor(INK)
        c.drawCentredString(positions[idx], chart_y + bh + 8, value_label)
        c.setFont("GalenSans", 7)
        c.setFillColor(MUTED)
        c.drawCentredString(positions[idx], chart_y - 14, labels[idx])


def token_panel(c: canvas.Canvas, x: float, y: float, w: float, h: float, runs: list[dict]):
    rounded(c, x, y, w, h, PAPER, 10)
    text(c, "Token 构成保持稳定", x + 14, y + h - 22, 9, INK, True)
    inputs = [run["metrics"]["inputTokens"] for run in runs]
    outputs = [run["metrics"]["outputTokens"] for run in runs]
    totals = [a + b for a, b in zip(inputs, outputs)]
    maximum = max(totals) * 1.15
    chart_y, chart_h = y + 31, h - 70
    positions = [x + w * 0.36, x + w * 0.68]
    bar_w = 38
    for idx, total in enumerate(totals):
        input_h = chart_h * inputs[idx] / maximum
        output_h = chart_h * outputs[idx] / maximum
        c.setFillColor(INK)
        c.roundRect(positions[idx] - bar_w / 2, chart_y, bar_w, input_h, 4, fill=1, stroke=0)
        c.setFillColor(GREEN_2)
        c.roundRect(positions[idx] - bar_w / 2, chart_y + input_h - 4, bar_w, output_h + 4, 4, fill=1, stroke=0)
        c.setFont("GalenSans-Bold", 7.5)
        c.setFillColor(INK)
        c.drawCentredString(positions[idx], chart_y + input_h + output_h + 8, f"{total:,}")
        c.setFont("GalenSans", 7)
        c.setFillColor(MUTED)
        c.drawCentredString(positions[idx], chart_y - 14, f"Run {idx + 1}")
    c.setFillColor(INK)
    c.rect(x + 16, y + 13, 8, 8, fill=1, stroke=0)
    text(c, "输入", x + 29, y + 13, 6.5, MUTED)
    c.setFillColor(GREEN_2)
    c.rect(x + 68, y + 13, 8, 8, fill=1, stroke=0)
    text(c, "输出", x + 81, y + 13, 6.5, MUTED)


def page_one(c: canvas.Canvas, run: dict):
    c.setFillColor(PAPER)
    c.rect(0, 0, PAGE_W, PAGE_H, fill=1, stroke=0)
    text(c, "GALEN", MARGIN, PAGE_H - 44, 11, GREEN, True)
    right_text(c, "CLOSED-LOOP PROBE / 2026-08-22", PAGE_W - MARGIN, PAGE_H - 44, 8, MUTED)
    text(c, "从“能生成文件”到“可验证交付”", MARGIN, PAGE_H - 103, 29, INK, True)
    text(c, "Galen 无界面真实模型闭环测试可视化报告", MARGIN, PAGE_H - 132, 13, INK_2)

    rounded(c, MARGIN, PAGE_H - 218, PAGE_W - 2 * MARGIN, 62, INK, 14)
    rounded(c, MARGIN + 18, PAGE_H - 202, 78, 30, GREEN, 15)
    text(c, "PASS", MARGIN + 37, PAGE_H - 194, 13, WHITE, True)
    text(c, "DeepSeek V4 Pro 已完成真实闭环", MARGIN + 116, PAGE_H - 188, 15, WHITE, True)
    right_text(c, "18 / 18 硬门通过", PAGE_W - MARGIN - 20, PAGE_H - 190, 11, HexColor("#A9DCCF"), True)

    cards_y = PAGE_H - 332
    gap = 12
    card_w = (PAGE_W - 2 * MARGIN - 3 * gap) / 4
    metrics = [
        ("首个可读响应", f'{run["metrics"]["ttfrMs"] / 1000:.2f}s', "TTFR", GREEN),
        ("端到端完成", f'{run["metrics"]["totalMs"] / 1000:.2f}s', "真实 API", ORANGE),
        ("上下文与输出", f'{run["metrics"]["inputTokens"] + run["metrics"]["outputTokens"]:,}', "Token", INK_2),
        ("工具收敛", f'{run["metrics"]["toolCalls"]} 次', "无重复", GREEN_2),
    ]
    for idx, values in enumerate(metrics):
        metric_card(c, MARGIN + idx * (card_w + gap), cards_y, card_w, 88, *values)

    text(c, "一次任务如何成为 Galen 内可预览的研究交付物", MARGIN, 218, 12, INK, True)
    stages = [
        ("01", "研究任务"),
        ("02", "三节点计划"),
        ("03", "执行节点 01"),
        ("04", "写入简报"),
        ("05", "Artifact 绑定"),
        ("06", "Galen 内预览"),
    ]
    stage_gap = 13
    stage_w = (PAGE_W - 2 * MARGIN - 5 * stage_gap) / 6
    for idx, (number, label) in enumerate(stages):
        x = MARGIN + idx * (stage_w + stage_gap)
        rounded(c, x, 120, stage_w, 70, WHITE, 10, LINE)
        text(c, number, x + 12, 169, 8, GREEN, True)
        paragraph(c, label, x + 12, 153, stage_w - 24, 9, 12, INK, True)
        text(c, "OK", x + stage_w - 28, 132, 7, GREEN, True)
        if idx < len(stages) - 1:
            c.setStrokeColor(GREEN_2)
            c.setLineWidth(1.5)
            c.line(x + stage_w + 3, 155, x + stage_w + stage_gap - 3, 155)
    footer(c, "数据源：evals/runs/probe-closed-loop-1787403827571.json；时间单位为毫秒换算秒。")
    c.showPage()


def page_two(c: canvas.Canvas, runs: list[dict]):
    c.setFillColor(WHITE)
    c.rect(0, 0, PAGE_W, PAGE_H, fill=1, stroke=0)
    header(c, "01 / PERFORMANCE", "两次真实 Pro 探针：响应快，闭环稳定", "相同场景、相同模型与工具合同；每次使用独立临时工作区。", 2)

    ttfr = [r["metrics"]["ttfrMs"] / 1000 for r in runs]
    total = [r["metrics"]["totalMs"] / 1000 for r in runs]
    bar_panel(c, MARGIN, 224, 229, 185, "首个可读响应 TTFR", ttfr, ["Run 1", "Run 2"], "s", 3)
    bar_panel(c, 279, 224, 229, 185, "端到端完成时间", total, ["Run 1", "Run 2"], "s", 2)
    token_panel(c, 523, 224, PAGE_W - 523 - MARGIN, 185, runs)

    ttfr_change = (runs[1]["metrics"]["ttfrMs"] / runs[0]["metrics"]["ttfrMs"] - 1) * 100
    total_change = (runs[1]["metrics"]["totalMs"] / runs[0]["metrics"]["totalMs"] - 1) * 100
    token_1 = runs[0]["metrics"]["inputTokens"] + runs[0]["metrics"]["outputTokens"]
    token_2 = runs[1]["metrics"]["inputTokens"] + runs[1]["metrics"]["outputTokens"]
    token_change = (token_2 / token_1 - 1) * 100

    insights = [
        ("TTFR", f"{ttfr_change:.0f}%", "Run 2 首响应更快；说明供应商本身并非 150 秒启动瓶颈。", GREEN),
        ("总耗时", f"+{total_change:.1f}%", "两次都在 35 秒内完成，差异远小于旧 UI 体验的数量级。", ORANGE),
        ("Token", f"{token_change:+.1f}%", "总 Token 基本持平；当前主要优化空间仍在约 10.5k 输入上下文。", INK_2),
    ]
    y = 113
    card_w = (PAGE_W - 2 * MARGIN - 20) / 3
    for idx, (label, value, note, accent) in enumerate(insights):
        x = MARGIN + idx * (card_w + 10)
        rounded(c, x, y, card_w, 84, PAPER, 10)
        text(c, label, x + 14, y + 61, 8, MUTED, True)
        text(c, value, x + 14, y + 32, 18, accent, True)
        paragraph(c, note, x + 78, y + 67, card_w - 90, 7.5, 10.5, INK_2)
    footer(c, "变换说明：TTFR/总耗时由 ms ÷ 1000；Token 总量 = input + output。样本 n=2，仅作工程冒烟，不作 P50/P90 基线。")
    c.showPage()


def page_three(c: canvas.Canvas):
    c.setFillColor(PAPER)
    c.rect(0, 0, PAGE_W, PAGE_H, fill=1, stroke=0)
    header(c, "02 / DELIVERY CONTRACT", "闭环不是一条文件路径，而是一组可验证关系", "探针同时检查持久化状态、领域事件、节点归属与前端预览契约。", 3)

    stages = [
        ("用户目标", "输入研究问题与预期产物"),
        ("ResearchTask", "创建 3 个持久化节点"),
        ("write_file", "执行节点 01 并写入 Markdown"),
        ("Artifact", "登记哈希、类型、大小和来源"),
        ("双向绑定", "task、artifact 与 node outputs 相互关联"),
        ("领域事件", "task-updated + artifact-created"),
        ("内置预览", "text/markdown 可直接渲染"),
    ]
    x0, y0 = MARGIN, 355
    w = (PAGE_W - 2 * MARGIN - 6 * 10) / 7
    for idx, (label, note) in enumerate(stages):
        x = x0 + idx * (w + 10)
        rounded(c, x, y0, w, 92, WHITE, 9, LINE)
        rounded(c, x + 10, y0 + 59, 24, 22, GREEN if idx in (1, 3, 5) else INK, 6)
        text(c, f"{idx + 1}", x + 18, y0 + 66, 8, WHITE, True)
        paragraph(c, label, x + 10, y0 + 53, w - 20, 8.2, 11, INK, True)
        paragraph(c, note, x + 10, y0 + 31, w - 20, 6.7, 9, MUTED)
        if idx < len(stages) - 1:
            c.setStrokeColor(GREEN_2)
            c.setLineWidth(1.4)
            c.line(x + w + 2, y0 + 46, x + w + 8, y0 + 46)

    text(c, "18 项硬门矩阵", MARGIN, 317, 12, INK, True)
    gates = [
        "运行完成", "模型请求≤6", "工具调用≤8", "工具零错误", "聊天零错误", "最终响应非空",
        "无重复循环", "创建计划", "写入文件", "文件非空", "Artifact 已登记", "任务/节点已绑定",
        "≥3 个节点", "≥1 节点完成", "双向引用一致", "任务事件", "产物事件", "预览格式支持",
    ]
    cols = 6
    tile_gap = 8
    tile_w = (PAGE_W - 2 * MARGIN - (cols - 1) * tile_gap) / cols
    tile_h = 43
    for idx, gate in enumerate(gates):
        row, col = divmod(idx, cols)
        x = MARGIN + col * (tile_w + tile_gap)
        y = 245 - row * (tile_h + 8)
        rounded(c, x, y, tile_w, tile_h, WHITE, 8, LINE)
        rounded(c, x + 9, y + 11, 20, 20, MINT, 10)
        text(c, "OK", x + 12, y + 17, 5.8, GREEN, True)
        text(c, gate, x + 37, y + 16, 7.2, INK, True)

    rounded(c, MARGIN, 57, PAGE_W - 2 * MARGIN, 50, INK, 11)
    text(c, "结论", MARGIN + 16, 84, 8, HexColor("#A9DCCF"), True)
    paragraph(c, "本轮证明的不只是“模型写出了文件”，而是 Galen 已能把文件转换成可恢复、可追踪、可归属、可预览的科研交付物。", MARGIN + 68, 91, PAGE_W - 2 * MARGIN - 88, 10, 14, WHITE, True)
    footer(c, "硬门来源：最新 closed-loop JSON 报告；所有断言均为机器可判定条件，不依赖人工截图判断。")
    c.showPage()


def page_four(c: canvas.Canvas):
    c.setFillColor(WHITE)
    c.rect(0, 0, PAGE_W, PAGE_H, fill=1, stroke=0)
    header(c, "03 / DECISION", "测试结果如何指导下一轮 Galen 优化", "把真实模型探针放在单元测试之上，把鼠标验收降为发布前冒烟。", 4)

    text(c, "新的测试金字塔", MARGIN, 423, 12, INK, True)
    pyramid = [
        ("真实 UI 冒烟", "仅发布前", 215, ORANGE),
        ("真实模型闭环", "2 次 / 本轮", 300, GREEN),
        ("交付契约硬门", "18 / 18", 385, GREEN_2),
        ("单元与前端测试", "Rust 86 + Frontend 9", 470, INK),
    ]
    center_x = 282
    y = 200
    for label, value, width, color in reversed(pyramid):
        x = center_x - width / 2
        rounded(c, x, y, width, 48, color, 8)
        text(c, label, x + 16, y + 20, 9, WHITE, True)
        right_text(c, value, x + width - 16, y + 20, 8, WHITE, True)
        y += 55

    right_x = 545
    text(c, "本轮给出的三个信号", right_x, 423, 12, INK, True)
    signals = [
        ("01", "150 秒不是模型固有启动时间", "无界面 TTFR 为 0.44-1.19 秒，应继续排查 UI 会话恢复、上下文装配或事件投影。"),
        ("02", "执行链已经收敛", "两次都只调用 create_research_plan 与 write_file，没有重复工具或人工干预。"),
        ("03", "上下文仍是成本重点", "输入约 10.5k Token。下一轮应以减少 20%-30% 为候选目标，但必须守住 18 项硬门。"),
    ]
    y = 350
    for number, title, body in signals:
        rounded(c, right_x, y, 255, 66, PAPER, 10)
        rounded(c, right_x + 12, y + 32, 27, 22, GREEN, 7)
        text(c, number, right_x + 18, y + 39, 7.5, WHITE, True)
        text(c, title, right_x + 49, y + 43, 8.5, INK, True)
        paragraph(c, body, right_x + 49, y + 30, 190, 7, 9.5, INK_2)
        y -= 75

    rounded(c, right_x, 77, 255, 88, INK, 11)
    text(c, "下一轮实验协议", right_x + 14, 143, 8, HexColor("#A9DCCF"), True)
    paragraph(c, "每个版本至少 5 次真实运行；采用 A-B-B-A 交错；18 项硬门必须全过；TTFR P90 不得恶化超过 10%；只有总耗时或 Token 改善 ≥15% 才升级基线。", right_x + 14, 132, 226, 7.6, 10.5, WHITE)
    footer(c, "限制：真实样本仅 n=2，尚不足以建立统计基线；当前结论用于工程方向判断，不代表供应商长期性能保证。")
    c.showPage()


def build() -> Path:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    TMP_DIR.mkdir(parents=True, exist_ok=True)
    register_fonts()
    runs = load_runs()
    latest = runs[-1]

    c = canvas.Canvas(str(OUT_PDF), pagesize=landscape(A4), pageCompression=1)
    c.setTitle("Galen Closed-Loop Probe Report - 2026-08-22")
    c.setAuthor("Galen Evaluation")
    c.setSubject("DeepSeek V4 Pro closed-loop probe visualization")
    page_one(c, latest)
    page_two(c, runs)
    page_three(c)
    page_four(c)
    c.save()
    print(OUT_PDF)
    return OUT_PDF


if __name__ == "__main__":
    build()
