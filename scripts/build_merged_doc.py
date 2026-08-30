# -*- coding: utf-8 -*-
"""
Galen 完整产品与技术文档（合并升级版）——reportlab 渲染引擎。
运行环境：codex-primary-runtime Python 3.12.13（reportlab 4.4.9）。
用法：python scripts/build_merged_doc.py
"""
from __future__ import annotations

import html
import re
from pathlib import Path

from reportlab.lib import colors
from reportlab.lib.colors import HexColor
from reportlab.lib.enums import TA_CENTER, TA_LEFT
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.units import mm
from reportlab.lib.utils import ImageReader
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import (
    BaseDocTemplate,
    Frame,
    HRFlowable,
    Image,
    KeepTogether,
    ListFlowable,
    ListItem,
    PageBreak,
    PageTemplate,
    Paragraph,
    Spacer,
    Table,
    TableStyle,
)

ROOT = Path(__file__).resolve().parents[1]
CHARTS = ROOT / "tmp" / "charts"
OUTPUT = ROOT / "output" / "pdf" / "Galen-完整文档-合并升级版.pdf"

FONT_REGULAR = Path(r"C:\Windows\Fonts\msyh.ttc")
FONT_BOLD = Path(r"C:\Windows\Fonts\msyhbd.ttc")

INK = HexColor("#17382F")
TEXT = HexColor("#405C54")
MUTED = HexColor("#7A8E87")
GREEN = HexColor("#07866F")
GREEN_2 = HexColor("#43AF92")
MINT = HexColor("#DDF3EC")
PALE = HexColor("#F3F7F5")
LINE = HexColor("#D7E3DF")
ORANGE = HexColor("#D98242")
RED = HexColor("#C65A56")
WHITE = colors.white

# Anthropic 品牌色：Dark #141413 / Light #FAF9F5 / MidGray #B0AEA5 /
# LightGray #E8E6DC / Orange #D97757 / Blue #6A9BCC / Green #788C5D
THEMES = {
    "galen": dict(INK="#17382F", TEXT="#405C54", MUTED="#7A8E87", GREEN="#07866F", GREEN_2="#43AF92",
                  MINT="#DDF3EC", PALE="#F3F7F5", LINE="#D7E3DF", ORANGE="#D98242", RED="#C65A56"),
    "anthropic": dict(INK="#141413", TEXT="#4A4A46", MUTED="#8F8D88", GREEN="#D97757", GREEN_2="#6A9BCC",
                      MINT="#EDEAE2", PALE="#FAF9F5", LINE="#E8E6DC", ORANGE="#D97757", RED="#B4552D"),
}
_CURRENT_THEME = {"name": "galen"}


def theme_color(name: str) -> str:
    return THEMES[_CURRENT_THEME["name"]][name]


def set_theme(name: str) -> None:
    global INK, TEXT, MUTED, GREEN, GREEN_2, MINT, PALE, LINE, ORANGE, RED, CHARTS, OUTPUT
    _CURRENT_THEME["name"] = name
    t = THEMES[name]
    INK, TEXT, MUTED = HexColor(t["INK"]), HexColor(t["TEXT"]), HexColor(t["MUTED"])
    GREEN, GREEN_2 = HexColor(t["GREEN"]), HexColor(t["GREEN_2"])
    MINT, PALE, LINE = HexColor(t["MINT"]), HexColor(t["PALE"]), HexColor(t["LINE"])
    ORANGE, RED = HexColor(t["ORANGE"]), HexColor(t["RED"])
    CHARTS = ROOT / "tmp" / ("charts-anthropic" if name == "anthropic" else "charts")
    OUTPUT = ROOT / "output" / "pdf" / (
        "Galen-完整文档-合并升级版-Anthropic.pdf" if name == "anthropic" else "Galen-完整文档-合并升级版.pdf")

PAGE_W, PAGE_H = A4
LEFT = 19 * mm
RIGHT = 19 * mm
TOP = 20 * mm
BOTTOM = 18 * mm
CONTENT_W = PAGE_W - LEFT - RIGHT  # ~172mm


def register_fonts() -> None:
    pdfmetrics.registerFont(TTFont("GalenSans", str(FONT_REGULAR)))
    pdfmetrics.registerFont(TTFont("GalenSans-Bold", str(FONT_BOLD)))


def styles() -> dict[str, ParagraphStyle]:
    base = getSampleStyleSheet()
    return {
        "cover_kicker": ParagraphStyle(
            "cover_kicker", parent=base["Normal"], fontName="GalenSans-Bold",
            fontSize=10, leading=14, textColor=GREEN, spaceAfter=8,
        ),
        "cover_title": ParagraphStyle(
            "cover_title", parent=base["Title"], fontName="GalenSans-Bold",
            fontSize=30, leading=39, textColor=WHITE, spaceAfter=12, wordWrap="CJK",
        ),
        "cover_subtitle": ParagraphStyle(
            "cover_subtitle", parent=base["Normal"], fontName="GalenSans",
            fontSize=12, leading=20, textColor=TEXT, spaceAfter=20, wordWrap="CJK",
        ),
        "h1": ParagraphStyle(
            "h1", parent=base["Heading1"], fontName="GalenSans-Bold",
            fontSize=21, leading=29, textColor=INK, spaceAfter=10, wordWrap="CJK",
        ),
        "h2": ParagraphStyle(
            "h2", parent=base["Heading2"], fontName="GalenSans-Bold",
            fontSize=13.5, leading=20, textColor=GREEN, spaceBefore=10, spaceAfter=6,
            keepWithNext=True, wordWrap="CJK",
        ),
        "h3": ParagraphStyle(
            "h3", parent=base["Heading3"], fontName="GalenSans-Bold",
            fontSize=11, leading=16.5, textColor=INK, spaceBefore=8, spaceAfter=4,
            keepWithNext=True, wordWrap="CJK",
        ),
        "body": ParagraphStyle(
            "body", parent=base["BodyText"], fontName="GalenSans",
            fontSize=9.2, leading=15, textColor=TEXT, spaceAfter=5, wordWrap="CJK",
        ),
        "body_bold": ParagraphStyle(
            "body_bold", parent=base["BodyText"], fontName="GalenSans-Bold",
            fontSize=9.2, leading=15, textColor=INK, spaceAfter=5, wordWrap="CJK",
        ),
        "small": ParagraphStyle(
            "small", parent=base["BodyText"], fontName="GalenSans",
            fontSize=7.3, leading=11, textColor=MUTED, wordWrap="CJK",
        ),
        "callout": ParagraphStyle(
            "callout", parent=base["BodyText"], fontName="GalenSans",
            fontSize=9, leading=14.6, textColor=INK, wordWrap="CJK",
        ),
        "toc": ParagraphStyle(
            "toc", parent=base["BodyText"], fontName="GalenSans-Bold",
            fontSize=10, leading=17, textColor=INK, wordWrap="CJK",
        ),
        "toc_sub": ParagraphStyle(
            "toc_sub", parent=base["BodyText"], fontName="GalenSans",
            fontSize=8.4, leading=13.5, textColor=TEXT, leftIndent=12, wordWrap="CJK",
        ),
        "code": ParagraphStyle(
            "code", parent=base["BodyText"], fontName="GalenSans",
            fontSize=7.4, leading=10.8, textColor=INK, leftIndent=5, rightIndent=5,
            spaceAfter=1.2, wordWrap="CJK",
        ),
        "table": ParagraphStyle(
            "table", parent=base["BodyText"], fontName="GalenSans",
            fontSize=7.5, leading=11.2, textColor=TEXT, wordWrap="CJK",
        ),
        "table_head": ParagraphStyle(
            "table_head", parent=base["BodyText"], fontName="GalenSans-Bold",
            fontSize=7.7, leading=11.2, textColor=WHITE, wordWrap="CJK",
        ),
        "caption": ParagraphStyle(
            "caption", parent=base["BodyText"], fontName="GalenSans",
            fontSize=7.4, leading=10.5, textColor=MUTED, alignment=TA_CENTER,
            spaceBefore=2, spaceAfter=8, wordWrap="CJK",
        ),
    }


def inline(value: str) -> str:
    value = html.escape(value.strip())
    value = re.sub(r"`([^`]+)`", r'<font color="#07866F">\1</font>', value)
    value = re.sub(r"\*\*([^*]+)\*\*", r"<b>\1</b>", value)
    return value


def header_footer(c, doc) -> None:
    if doc.page == 1:
        return
    c.saveState()
    c.setStrokeColor(LINE)
    c.setLineWidth(0.6)
    c.line(LEFT, PAGE_H - 13 * mm, PAGE_W - RIGHT, PAGE_H - 13 * mm)
    c.setFont("GalenSans-Bold", 7)
    c.setFillColor(GREEN)
    c.drawString(LEFT, PAGE_H - 9.5 * mm, "GALEN / 完整产品与技术文档")
    c.setFont("GalenSans", 7)
    c.setFillColor(MUTED)
    c.drawRightString(PAGE_W - RIGHT, PAGE_H - 9.5 * mm, "合并升级版 v1.0 · 2026-08-29")
    c.line(LEFT, 12.5 * mm, PAGE_W - RIGHT, 12.5 * mm)
    c.drawString(LEFT, 8.5 * mm, "Galen 科研工作台 · 内部工程评测与用户文档合集")
    c.drawRightString(PAGE_W - RIGHT, 8.5 * mm, f"PAGE {doc.page}")
    c.restoreState()


# ---------------------------------------------------------------- 通用组件

def P(story, text, style=None, bold=False):
    story.append(Paragraph(inline(text), style or (styles()["body_bold"] if bold else styles()["body"])))


def H1(story, text):
    story.append(Paragraph(inline(text), styles()["h1"]))
    story.append(HRFlowable(width="100%", thickness=1.1, color=GREEN, spaceAfter=8))


def H2(story, text):
    story.append(Paragraph(inline(text), styles()["h2"]))


def H3(story, text):
    story.append(Paragraph(inline(text), styles()["h3"]))


def BULLETS(story, items, numbered=False):
    flow = []
    for item in items:
        if isinstance(item, tuple):
            head, rest = item
            value = f"**{head}**　{rest}"
        else:
            value = item
        flow.append(ListItem(Paragraph(inline(value), styles()["body"]), leftIndent=8))
    story.append(ListFlowable(
        flow,
        bulletType="1" if numbered else "bullet",
        start="circle" if not numbered else None,
        leftIndent=16,
        bulletFontName="GalenSans" if not numbered else "GalenSans-Bold",
        bulletFontSize=7 if not numbered else 8,
        bulletColor=GREEN,
        spaceAfter=5,
    ))


def CALLOUT(story, text_value, marker="GALEN", color=MINT, accent=GREEN, small=False):
    marker_p = Paragraph(marker, ParagraphStyle(
        "marker", fontName="GalenSans-Bold", fontSize=6.5, textColor=WHITE, alignment=TA_CENTER,
    ))
    body = Paragraph(inline(text_value), styles()["small"] if small else styles()["callout"])
    table = Table([[marker_p, body]], colWidths=[15 * mm, CONTENT_W - 15 * mm], hAlign="LEFT")
    table.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (0, 0), accent),
        ("BACKGROUND", (1, 0), (1, 0), color),
        ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
        ("LEFTPADDING", (0, 0), (0, 0), 4),
        ("RIGHTPADDING", (0, 0), (0, 0), 4),
        ("TOPPADDING", (0, 0), (-1, -1), 8),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 8),
        ("LEFTPADDING", (1, 0), (1, 0), 10),
        ("RIGHTPADDING", (1, 0), (1, 0), 10),
        ("BOX", (0, 0), (-1, -1), 0.6, LINE),
    ]))
    story.extend([table, Spacer(1, 2.5 * mm)])


def TBL(story, rows, col_widths=None, header=True, font_scale=1.0):
    s = styles()
    data = []
    for row_idx, row in enumerate(rows):
        style = s["table_head"] if (header and row_idx == 0) else s["table"]
        data.append([Paragraph(inline(cell), style) for cell in row])
    widths = col_widths or [CONTENT_W / len(rows[0])] * len(rows[0])
    if sum(widths) > CONTENT_W + 0.01:
        scale = CONTENT_W / sum(widths)
        widths = [w * scale for w in widths]
    table = Table(data, colWidths=widths, repeatRows=1 if header else 0, hAlign="LEFT")
    style_cmds = [
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("GRID", (0, 0), (-1, -1), 0.5, LINE),
        ("LEFTPADDING", (0, 0), (-1, -1), 6),
        ("RIGHTPADDING", (0, 0), (-1, -1), 6),
        ("TOPPADDING", (0, 0), (-1, -1), 5),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 5),
    ]
    if header:
        style_cmds.append(("BACKGROUND", (0, 0), (-1, 0), INK))
        style_cmds.append(("ROWBACKGROUNDS", (0, 1), (-1, -1), [WHITE, PALE]))
    else:
        style_cmds.append(("ROWBACKGROUNDS", (0, 0), (-1, -1), [WHITE, PALE]))
    table.setStyle(TableStyle(style_cmds))
    story.extend([table, Spacer(1, 2.5 * mm)])


def IMG(story, filename, width_mm=(CONTENT_W - 8 * mm) / mm, caption=None):
    path = CHARTS / filename
    if not path.exists():
        raise FileNotFoundError(path)
    ir = ImageReader(str(path))
    pw, ph = ir.getSize()  # 像素尺寸，避免触发 Image 懒加载副作用
    w = width_mm * mm
    h = w * ph / pw
    if h > 92 * mm:  # 控制单图高度
        h = 92 * mm
        w = h * pw / ph
    img = Image(str(path), width=w, height=h)
    story.append(Spacer(1, 1.5 * mm))
    story.append(img)
    if caption:
        story.append(Paragraph(caption, styles()["caption"]))
    else:
        story.append(Spacer(1, 1.5 * mm))


def CODEBOX(story, lines, title=None):
    s = styles()
    if title:
        story.append(Paragraph(title, s["h3"]))
    flowables = []
    for line in lines:
        if not line:
            flowables.append(Spacer(1, 2.2))
        else:
            flowables.append(Paragraph(html.escape(line), s["code"]))
    box = Table([[flowables]], colWidths=[CONTENT_W], hAlign="LEFT")
    box.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, -1), PALE),
        ("BOX", (0, 0), (-1, -1), 0.7, LINE),
        ("LEFTPADDING", (0, 0), (-1, -1), 9),
        ("RIGHTPADDING", (0, 0), (-1, -1), 9),
        ("TOPPADDING", (0, 0), (-1, -1), 8),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 8),
    ]))
    story.extend([box, Spacer(1, 3 * mm)])


def _hexstr(color) -> str:
    if isinstance(color, str):
        return color
    return "#" + color.hexval()[2:]


def KPI(story, items, cols=None):
    """items: [(label, value, note, accent)]"""
    cols = cols or len(items)
    data = [[]]
    widths = []
    for label, value, note, accent in items:
        acc = HexColor(accent) if isinstance(accent, str) else accent
        label_p = Paragraph(label, ParagraphStyle(
            "k_label", fontName="GalenSans-Bold", fontSize=7.2, textColor=MUTED, wordWrap="CJK",
        ))
        value_p = Paragraph(f'<font color="{_hexstr(acc)}">{value}</font>',
                            ParagraphStyle("k_value", fontName="GalenSans-Bold", fontSize=14, leading=18, textColor=INK, wordWrap="CJK"))
        note_p = Paragraph(note, ParagraphStyle(
            "k_note", fontName="GalenSans", fontSize=6.8, leading=10, textColor=MUTED, wordWrap="CJK",
        ))
        data[0].append([label_p, value_p, note_p])
        widths.append(CONTENT_W / cols)
    table = Table(data, colWidths=widths)
    cmds = [
        ("BACKGROUND", (0, 0), (-1, -1), WHITE),
        ("BOX", (0, 0), (-1, -1), 0.8, LINE),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("LEFTPADDING", (0, 0), (-1, -1), 8),
        ("RIGHTPADDING", (0, 0), (-1, -1), 8),
        ("TOPPADDING", (0, 0), (-1, -1), 8),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 8),
    ]
    for i, (_, _, _, accent) in enumerate(items):
        acc = HexColor(accent) if isinstance(accent, str) else accent
        cmds.append(("LINEBEFORE", (i, 0), (i, 0), 0, WHITE) if i else ("LINEBEFORE", (0, 0), (0, 0), 0, WHITE))
        cmds.append(("LINEAFTER", (i, 0), (i, 0), 2.5, acc))
    table.setStyle(TableStyle(cmds))
    story.extend([table, Spacer(1, 3 * mm)])


def GRID_GATES(story, gates, cols=6):
    """✓ 硬门矩阵"""
    rows = []
    for i in range(0, len(gates), cols):
        rows.append(gates[i:i + cols])
    data = []
    for row in rows:
        cells = []
        for gate in row:
            mark = Paragraph("✓", ParagraphStyle(
                "tick", fontName="GalenSans-Bold", fontSize=7.5, textColor=GREEN, alignment=TA_CENTER,
            ))
            label = Paragraph(gate, ParagraphStyle(
                "gate", fontName="GalenSans", fontSize=6.9, leading=10, textColor=INK, wordWrap="CJK",
            ))
            cells.append(Table([[mark, label]], colWidths=[7 * mm, (CONTENT_W / cols - 7 * mm) * 0.95]))
        data.append(cells)
    widths = [CONTENT_W / cols] * cols
    table = Table(data, colWidths=widths)
    cmds = [
        ("BACKGROUND", (0, 0), (-1, -1), MINT),
        ("GRID", (0, 0), (-1, -1), 0.6, LINE),
        ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
        ("LEFTPADDING", (0, 0), (-1, -1), 6),
        ("RIGHTPADDING", (0, 0), (-1, -1), 6),
        ("TOPPADDING", (0, 0), (-1, -1), 5),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 5),
    ]
    table.setStyle(TableStyle(cmds))
    story.extend([table, Spacer(1, 2.5 * mm)])


def FLOW(story, steps, gap_mm=3.2):
    """阶段链条卡片（闭环 7 步等）"""
    n = len(steps)
    gap = gap_mm * mm
    w = (CONTENT_W - (n - 1) * gap) / n
    cells = []
    for idx, (num, title, note) in enumerate(steps):
        inner = Table([
            [Paragraph(num, ParagraphStyle("fnum", fontName="GalenSans-Bold", fontSize=8, textColor=WHITE, alignment=TA_CENTER))],
            [Paragraph(title, ParagraphStyle("ft", fontName="GalenSans-Bold", fontSize=7.6, leading=10.5, textColor=INK, wordWrap="CJK"))],
            [Paragraph(note, ParagraphStyle("fn", fontName="GalenSans", fontSize=6.4, leading=9, textColor=MUTED, wordWrap="CJK"))],
        ], colWidths=[w])
        inner.setStyle(TableStyle([
            ("BACKGROUND", (0, 0), (0, 0), GREEN if idx % 2 == 0 else INK),
            ("TOPPADDING", (0, 0), (0, 0), 3),
            ("BOTTOMPADDING", (0, 0), (0, 0), 3),
            ("LEFTPADDING", (0, 0), (-1, -1), 4),
            ("RIGHTPADDING", (0, 0), (-1, -1), 4),
            ("TOPPADDING", (0, 1), (0, 2), 3),
        ]))
        outer = Table([[inner]], colWidths=[w])
        outer.setStyle(TableStyle([
            ("BOX", (0, 0), (-1, -1), 0.7, LINE),
            ("BACKGROUND", (0, 0), (-1, -1), PALE),
            ("LEFTPADDING", (0, 0), (-1, -1), 2),
            ("RIGHTPADDING", (0, 0), (-1, -1), 2),
            ("TOPPADDING", (0, 0), (-1, -1), 4),
            ("BOTTOMPADDING", (0, 0), (-1, -1), 4),
        ]))
        cells.append(outer)
    row = Table([cells], colWidths=[w] * n)
    row.setStyle(TableStyle([
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("LEFTPADDING", (0, 0), (-1, -1), 0),
        ("RIGHTPADDING", (0, 0), (-1, -1), 0),
    ]))
    story.extend([row, Spacer(1, 2.5 * mm)])


def PART(story, number, title, subtitle, bullets=None):
    """部分分隔横幅（占一页顶部，后续内容另起页）"""
    body = [Paragraph(f"PART {number}", ParagraphStyle(
        "pn", fontName="GalenSans-Bold", fontSize=11, textColor=GREEN_2, spaceAfter=6)),
        Paragraph(title, ParagraphStyle(
            "pt", fontName="GalenSans-Bold", fontSize=25, leading=34, textColor=WHITE, spaceAfter=8, wordWrap="CJK"))]
    if subtitle:
        body.append(Paragraph(subtitle, ParagraphStyle(
            "ps", fontName="GalenSans", fontSize=9.5, leading=15, textColor=PALE, wordWrap="CJK")))
    if bullets:
        for b in bullets:
            body.append(Paragraph("· " + b, ParagraphStyle(
                "pb", fontName="GalenSans", fontSize=8.2, leading=13, textColor=PALE, wordWrap="CJK")))
    inner = Table([[body]], colWidths=[CONTENT_W])
    inner.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, -1), INK),
        ("LEFTPADDING", (0, 0), (-1, -1), 18),
        ("RIGHTPADDING", (0, 0), (-1, -1), 18),
        ("TOPPADDING", (0, 0), (-1, -1), 26),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 26),
        ("LINEBELOW", (0, 0), (-1, 0), 4, GREEN_2),
    ]))
    story.append(inner)
    story.append(Spacer(1, 8 * mm))


# ---------------------------------------------------------------- 封面 / 目录

def cover_story(s: dict[str, ParagraphStyle]) -> list:
    story = []
    band = Table([[
        Paragraph("GALEN / COMPLETE DOCUMENT", s["cover_kicker"]),
    ]], colWidths=[CONTENT_W])
    band.setStyle(TableStyle([("LEFTPADDING", (0, 0), (-1, -1), 0)]))
    story.append(Spacer(1, 8 * mm))
    story.append(band)

    title_block = Table([[
        Paragraph("Galen", ParagraphStyle("t1", fontName="GalenSans-Bold", fontSize=44, leading=52, textColor=INK)),
        Paragraph("科研品味驱动的医学科研助手", ParagraphStyle(
            "t2", fontName="GalenSans-Bold", fontSize=19, leading=27, textColor=GREEN, wordWrap="CJK")),
        Paragraph("完整产品与技术文档（合并升级版）", ParagraphStyle(
            "t3", fontName="GalenSans-Bold", fontSize=24, leading=33, textColor=INK, spaceBefore=10, wordWrap="CJK")),
        Spacer(1, 6),
        Paragraph("从产品使用、Alpha 自由探索，到上下文工程优化、闭环验证与发布前评测——五个文档合并重编为一册。",
                  s["cover_subtitle"]),
    ]], colWidths=[CONTENT_W])
    title_block.setStyle(TableStyle([("LEFTPADDING", (0, 0), (-1, -1), 0)]))
    story.append(Spacer(1, 26 * mm))
    story.append(title_block)

    cards = [
        [Paragraph("第一部分", s["body_bold"]), Paragraph("产品使用说明", s["body_bold"]), Paragraph("安装、首次启动、科研任务闭环与常见问题", s["small"])],
        [Paragraph("第二部分", s["body_bold"]), Paragraph("Alpha 自由探索手册", s["body_bold"]), Paragraph("面向受邀体验者的安全边界、探索路线与问题反馈", s["small"])],
        [Paragraph("第三部分", s["body_bold"]), Paragraph("工程评测报告", s["body_bold"]), Paragraph("上下文优化、闭环探针、改进与发布前验证", s["small"])],
    ]
    cards_table = Table(cards, colWidths=[CONTENT_W / 3] * 3, rowHeights=[12 * mm, 10 * mm, 16 * mm])
    cards_table.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, -1), WHITE),
        ("BOX", (0, 0), (-1, -1), 0.7, LINE),
        ("INNERGRID", (0, 0), (-1, -1), 0.5, LINE),
        ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
        ("LEFTPADDING", (0, 0), (-1, -1), 10),
        ("RIGHTPADDING", (0, 0), (-1, -1), 10),
    ]))
    story.extend([Spacer(1, 10 * mm), cards_table, Spacer(1, 14 * mm)])

    story.append(CALLOUT_RAW(
        "本合集的数值型数据一律以图表呈现，文字说明类信息保留表格。所有性能数字来自真实模型探针（evals/runs），"
        "Galen 生成的科研内容必须由具备相应专业能力的人复核。",
        s["callout"], color=HexColor("#FFF4E8"), accent=ORANGE,
    ))
    story.append(Spacer(1, 20 * mm))
    story.append(Paragraph("适用版本  v0.1.0  ·  Windows 10 / 11 64 位", s["small"]))
    story.append(Paragraph("更新日期  2026-08-29  ·  内部工程评测与用户文档", s["small"]))
    story.append(PageBreak())
    return story


def CALLOUT_RAW(text_value: str, style, color=MINT, accent=GREEN) -> Table:
    marker = Paragraph("GALEN", ParagraphStyle(
        "marker", fontName="GalenSans-Bold", fontSize=7, textColor=WHITE, alignment=TA_CENTER,
    ))
    body = Paragraph(inline(text_value), style)
    table = Table([[marker, body]], colWidths=[17 * mm, CONTENT_W - 17 * mm], hAlign="LEFT")
    table.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (0, 0), accent),
        ("BACKGROUND", (1, 0), (1, 0), color),
        ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
        ("LEFTPADDING", (0, 0), (0, 0), 5),
        ("RIGHTPADDING", (0, 0), (0, 0), 5),
        ("TOPPADDING", (0, 0), (-1, -1), 9),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 9),
        ("LEFTPADDING", (1, 0), (1, 0), 10),
        ("RIGHTPADDING", (1, 0), (1, 0), 10),
        ("BOX", (0, 0), (-1, -1), 0.6, LINE),
    ]))
    return table


def toc_story(s: dict[str, ParagraphStyle]) -> list:
    story = [Paragraph("阅读导航", s["h1"])]
    story.append(Paragraph(
        "本册按阅读对象组织：使用者先读第一、二部分；工程与评测细节在第三部分。数值型数据一律用图，"
        "文字说明类信息保留表格。", s["body"]))

    def part_row(num, title, entries):
        story.append(Paragraph(f"PART {num} · {title}", s["toc"]))
        for note in entries:
            story.append(Paragraph(note, s["toc_sub"]))

    part_row("01", "产品使用说明", [
        "1 安装  ·  2 首次启动（三步设置）  ·  3 界面速览",
        "4 核心使用流程：科研任务闭环  ·  5 模型与思考强度",
        "6 PubMed 文献检索  ·  7 持久化与记忆  ·  8 常见问题  ·  9 反馈模板",
    ])
    story.append(Spacer(1, 3 * mm))
    part_row("02", "Alpha 自由探索手册", [
        "1 欢迎参与  ·  2 安全边界  ·  3 安装与首次配置  ·  4 界面与工作模式",
        "5 开始探索（A-G 七条路线）  ·  6 什么算问题（P0-P3）",
        "7 出现问题时  ·  8 问题反馈模板  ·  9 体验结束后  ·  10 已知限制",
    ])
    story.append(Spacer(1, 3 * mm))
    part_row("03", "工程评测报告", [
        "报告一  上下文工程与推理调度优化（08-27）：六组探针量化 → 图",
        "报告二  无界面闭环探针（08-22）：性能对比、18 项硬门、决策",
        "报告三  改进与发布前验证（08-29）：速度/TTFR/Token 收敛 → 图",
    ])
    story.append(PageBreak())
    return story


# ---------------------------------------------------------------- 主构建

def build(theme: str = "galen") -> Path:
    set_theme(theme)
    from merged_doc_content import part1, part2, part3

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    if not CHARTS.exists():
        raise FileNotFoundError(f"缺少图表目录，请先运行 scripts/build_merged_doc_charts.py: {CHARTS}")
    register_fonts()
    s = styles()
    doc = BaseDocTemplate(
        str(OUTPUT), pagesize=A4, leftMargin=LEFT, rightMargin=RIGHT,
        topMargin=TOP, bottomMargin=BOTTOM,
        title="Galen 完整产品与技术文档（合并升级版）",
        author="Galen Engineering",
        subject="产品使用说明 · Alpha 探索手册 · 上下文优化 · 闭环探针 · 发布前评测",
    )
    content_frame = Frame(LEFT, BOTTOM, PAGE_W - LEFT - RIGHT, PAGE_H - TOP - BOTTOM, id="content")
    doc.addPageTemplates([
        PageTemplate(id="main", frames=[content_frame], onPage=header_footer),
    ])
    story = cover_story(s) + toc_story(s) + part1(s) + [PageBreak()] + part2(s) + [PageBreak()] + part3(s)
    doc.build(story)
    print(OUTPUT)
    return OUTPUT


if __name__ == "__main__":
    import sys
    # content 模块 `from build_merged_doc import ...` 时直接复用本实例，避免双重加载导致主题/全局不同步
    sys.modules["build_merged_doc"] = sys.modules[__name__]
    import argparse
    ap = argparse.ArgumentParser(description="Galen 完整文档（合并升级版）")
    ap.add_argument("--theme", choices=list(THEMES), default="galen")
    args = ap.parse_args()
    build(args.theme)
