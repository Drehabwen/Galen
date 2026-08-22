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
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import (
    BaseDocTemplate,
    Frame,
    HRFlowable,
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
SOURCE = ROOT / "docs" / "GALEN_ALPHA_EXPLORATION_GUIDE.md"
OUTPUT = ROOT / "output" / "pdf" / "galen-alpha-exploration-guide-v0.1.0.pdf"

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

PAGE_W, PAGE_H = A4
LEFT = 19 * mm
RIGHT = 19 * mm
TOP = 20 * mm
BOTTOM = 18 * mm


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
            fontSize=31, leading=40, textColor=INK, spaceAfter=12, wordWrap="CJK",
        ),
        "cover_subtitle": ParagraphStyle(
            "cover_subtitle", parent=base["Normal"], fontName="GalenSans",
            fontSize=12, leading=20, textColor=TEXT, spaceAfter=20, wordWrap="CJK",
        ),
        "h2": ParagraphStyle(
            "h2", parent=base["Heading1"], fontName="GalenSans-Bold",
            fontSize=22, leading=30, textColor=INK, spaceAfter=12, wordWrap="CJK",
        ),
        "h3": ParagraphStyle(
            "h3", parent=base["Heading2"], fontName="GalenSans-Bold",
            fontSize=14, leading=20, textColor=GREEN, spaceBefore=10, spaceAfter=7,
            keepWithNext=True, wordWrap="CJK",
        ),
        "h4": ParagraphStyle(
            "h4", parent=base["Heading3"], fontName="GalenSans-Bold",
            fontSize=11.5, leading=17, textColor=INK, spaceBefore=9, spaceAfter=5,
            keepWithNext=True, wordWrap="CJK",
        ),
        "body": ParagraphStyle(
            "body", parent=base["BodyText"], fontName="GalenSans",
            fontSize=9.4, leading=15.2, textColor=TEXT, spaceAfter=6, wordWrap="CJK",
        ),
        "body_bold": ParagraphStyle(
            "body_bold", parent=base["BodyText"], fontName="GalenSans-Bold",
            fontSize=9.4, leading=15.2, textColor=INK, spaceAfter=6, wordWrap="CJK",
        ),
        "small": ParagraphStyle(
            "small", parent=base["BodyText"], fontName="GalenSans",
            fontSize=7.5, leading=11.5, textColor=MUTED, wordWrap="CJK",
        ),
        "callout": ParagraphStyle(
            "callout", parent=base["BodyText"], fontName="GalenSans",
            fontSize=9.2, leading=15, textColor=INK, wordWrap="CJK",
        ),
        "toc": ParagraphStyle(
            "toc", parent=base["BodyText"], fontName="GalenSans-Bold",
            fontSize=10.5, leading=18, textColor=INK, wordWrap="CJK",
        ),
        "code": ParagraphStyle(
            "code", parent=base["BodyText"], fontName="GalenSans",
            fontSize=7.6, leading=11.2, textColor=INK, leftIndent=5, rightIndent=5,
            spaceAfter=1.5, wordWrap="CJK",
        ),
        "table": ParagraphStyle(
            "table", parent=base["BodyText"], fontName="GalenSans",
            fontSize=7.6, leading=11.5, textColor=TEXT, wordWrap="CJK",
        ),
        "table_head": ParagraphStyle(
            "table_head", parent=base["BodyText"], fontName="GalenSans-Bold",
            fontSize=7.8, leading=11.5, textColor=WHITE, wordWrap="CJK",
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
    c.setFont("GalenSans-Bold", 7.2)
    c.setFillColor(GREEN)
    c.drawString(LEFT, PAGE_H - 9.5 * mm, "GALEN / ALPHA EXPLORATION GUIDE")
    c.setFont("GalenSans", 7.2)
    c.setFillColor(MUTED)
    c.drawRightString(PAGE_W - RIGHT, PAGE_H - 9.5 * mm, "v0.1.0 · 2026-08-22")
    c.line(LEFT, 12.5 * mm, PAGE_W - RIGHT, 12.5 * mm)
    c.drawString(LEFT, 8.5 * mm, "仅限受邀 Alpha 体验 · 不输入可识别患者信息")
    c.drawRightString(PAGE_W - RIGHT, 8.5 * mm, f"PAGE {doc.page}")
    c.restoreState()


def cover_background(c, doc) -> None:
    c.saveState()
    c.setFillColor(HexColor("#F5F8F6"))
    c.rect(0, 0, PAGE_W, PAGE_H, fill=1, stroke=0)
    c.setFillColor(INK)
    c.rect(0, 0, PAGE_W, 38 * mm, fill=1, stroke=0)
    c.setFillColor(GREEN)
    c.circle(PAGE_W - 28 * mm, PAGE_H - 26 * mm, 18 * mm, fill=1, stroke=0)
    c.setFillColor(GREEN_2)
    c.circle(PAGE_W - 15 * mm, PAGE_H - 44 * mm, 7 * mm, fill=1, stroke=0)
    c.restoreState()


def callout(text_value: str, style, color=MINT, accent=GREEN) -> Table:
    marker = Paragraph("ALPHA", ParagraphStyle(
        "marker", fontName="GalenSans-Bold", fontSize=7, textColor=WHITE, alignment=TA_CENTER,
    ))
    body = Paragraph(inline(text_value), style)
    table = Table([[marker, body]], colWidths=[17 * mm, 139 * mm], hAlign="LEFT")
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


def cover_story(s: dict[str, ParagraphStyle]) -> list:
    story = [Spacer(1, 47 * mm)]
    story.append(Paragraph("GALEN / ALPHA PROGRAM", s["cover_kicker"]))
    story.append(Paragraph("自由探索手册", s["cover_title"]))
    story.append(Paragraph(
        "不要迁就产品。按自己的习惯使用、改变主意、撞到边界，并把每一个不自然的瞬间记录成可复现的问题。",
        s["cover_subtitle"],
    ))
    cards = [
        [Paragraph("自然使用", s["body_bold"]), Paragraph("自由探索", s["body_bold"]), Paragraph("诚实反馈", s["body_bold"])],
        [Paragraph("用真实语言表达", s["small"]), Paragraph("任选路线与任务", s["small"]), Paragraph("记录预期与现场", s["small"])],
    ]
    table = Table(cards, colWidths=[52 * mm] * 3, rowHeights=[13 * mm, 10 * mm])
    table.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, -1), WHITE),
        ("BOX", (0, 0), (-1, -1), 0.7, LINE),
        ("INNERGRID", (0, 0), (-1, -1), 0.5, LINE),
        ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
        ("LEFTPADDING", (0, 0), (-1, -1), 10),
    ]))
    story.extend([table, Spacer(1, 12 * mm)])
    story.append(callout(
        "本手册面向受邀体验者。只使用虚构、公开或充分去标识化的数据；Galen 输出必须由专业人员复核。",
        s["callout"], color=HexColor("#FFF4E8"), accent=ORANGE,
    ))
    story.append(Spacer(1, 24 * mm))
    story.append(Paragraph("适用版本  v0.1.0  ·  Windows 10 / 11 64 位", s["small"]))
    story.append(Paragraph("更新日期  2026-08-22  ·  面向受邀体验者", s["small"]))
    story.append(PageBreak())
    return story


def toc_story(s: dict[str, ParagraphStyle]) -> list:
    entries = [
        ("01", "欢迎参与", "如何成为真实而不是顺从的体验者"),
        ("02", "安全边界", "隐私、密钥、工作区与医学责任"),
        ("03", "安装与首次配置", "安装包、模型连接与测试工作区"),
        ("04", "界面与工作模式", "讨论、计划、自动、证据与预览"),
        ("05", "开始探索", "七条可任选的自由探索路线"),
        ("06", "什么算问题", "从阻塞和循环到信任与科研错误"),
        ("07", "出现问题时", "先保存现场，再做最小恢复"),
        ("08", "反馈模板", "提交一个可复现、无敏感信息的问题"),
        ("09", "体验结束后", "六个帮助产品决策的问题"),
        ("10", "已知限制", "Alpha 阶段需要诚实面对的边界"),
    ]
    story = [Paragraph("阅读导航", s["h2"])]
    story.append(Paragraph(
        "不需要从头到尾照做。首次体验先读安全边界和安装配置；使用过程中按兴趣选择探索路线；发现问题时直接跳到反馈模板。",
        s["body"],
    ))
    rows = []
    for number, title, note in entries:
        rows.append([
            Paragraph(number, ParagraphStyle("n", fontName="GalenSans-Bold", fontSize=10, textColor=GREEN)),
            Paragraph(title, s["toc"]),
            Paragraph(note, s["small"]),
        ])
    table = Table(rows, colWidths=[15 * mm, 48 * mm, 91 * mm], rowHeights=[14 * mm] * len(rows))
    table.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, -1), PALE),
        ("ROWBACKGROUNDS", (0, 0), (-1, -1), [PALE, WHITE]),
        ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
        ("LEFTPADDING", (0, 0), (-1, -1), 8),
        ("RIGHTPADDING", (0, 0), (-1, -1), 8),
        ("LINEBELOW", (0, 0), (-1, -2), 0.4, LINE),
        ("BOX", (0, 0), (-1, -1), 0.7, LINE),
    ]))
    story.extend([Spacer(1, 4 * mm), table, PageBreak()])
    return story


def priority_table(rows: list[list[str]], s: dict[str, ParagraphStyle]) -> Table:
    data = []
    for row_idx, row in enumerate(rows):
        style = s["table_head"] if row_idx == 0 else s["table"]
        data.append([Paragraph(inline(cell), style) for cell in row])
    table = Table(data, colWidths=[18 * mm, 49 * mm, 87 * mm], repeatRows=1, hAlign="LEFT")
    table.setStyle(TableStyle([
        ("BACKGROUND", (0, 0), (-1, 0), INK),
        ("ROWBACKGROUNDS", (0, 1), (-1, -1), [WHITE, PALE]),
        ("VALIGN", (0, 0), (-1, -1), "TOP"),
        ("GRID", (0, 0), (-1, -1), 0.5, LINE),
        ("LEFTPADDING", (0, 0), (-1, -1), 7),
        ("RIGHTPADDING", (0, 0), (-1, -1), 7),
        ("TOPPADDING", (0, 0), (-1, -1), 7),
        ("BOTTOMPADDING", (0, 0), (-1, -1), 7),
    ]))
    return table


def parse_manual(s: dict[str, ParagraphStyle]) -> list:
    lines = SOURCE.read_text(encoding="utf-8").splitlines()
    start = next(i for i, line in enumerate(lines) if line.startswith("## 1."))
    lines = lines[start:]
    story: list = []
    i = 0
    first_h2 = True
    while i < len(lines):
        line = lines[i].rstrip()
        if not line or line == "---":
            i += 1
            continue
        if line.startswith("## "):
            if not first_h2:
                story.append(PageBreak())
            first_h2 = False
            story.append(Paragraph(inline(line[3:]), s["h2"]))
            story.append(HRFlowable(width="100%", thickness=1.1, color=GREEN, spaceAfter=8))
            i += 1
            continue
        if line.startswith("### "):
            story.append(Paragraph(inline(line[4:]), s["h3"]))
            i += 1
            continue
        if line.startswith("#### "):
            story.append(Paragraph(inline(line[5:]), s["h4"]))
            i += 1
            continue
        if line.startswith("> "):
            quote = []
            while i < len(lines) and lines[i].startswith("> "):
                quote.append(lines[i][2:].strip())
                i += 1
            story.extend([callout(" ".join(quote), s["callout"]), Spacer(1, 3 * mm)])
            continue
        if line.startswith("```"):
            language = line[3:].strip()
            i += 1
            code_lines = []
            while i < len(lines) and not lines[i].startswith("```"):
                code_lines.append(lines[i])
                i += 1
            i += 1
            title = "可复制反馈模板" if language == "markdown" else "示例"
            story.append(Paragraph(title, s["h4"]))
            code_flowables = []
            for code_line in code_lines:
                if not code_line:
                    code_flowables.append(Spacer(1, 2.5))
                else:
                    code_flowables.append(Paragraph(html.escape(code_line), s["code"]))
            box = Table([[code_flowables]], colWidths=[154 * mm], hAlign="LEFT")
            box.setStyle(TableStyle([
                ("BACKGROUND", (0, 0), (-1, -1), PALE),
                ("BOX", (0, 0), (-1, -1), 0.7, LINE),
                ("LEFTPADDING", (0, 0), (-1, -1), 9),
                ("RIGHTPADDING", (0, 0), (-1, -1), 9),
                ("TOPPADDING", (0, 0), (-1, -1), 8),
                ("BOTTOMPADDING", (0, 0), (-1, -1), 8),
            ]))
            story.append(box)
            story.append(Spacer(1, 3 * mm))
            continue
        if line.startswith("|"):
            table_lines = []
            while i < len(lines) and lines[i].startswith("|"):
                table_lines.append(lines[i])
                i += 1
            rows = []
            for idx, table_line in enumerate(table_lines):
                cells = [cell.strip() for cell in table_line.strip("|").split("|")]
                if idx == 1 and all(set(cell) <= {"-", ":"} for cell in cells):
                    continue
                rows.append(cells)
            story.extend([priority_table(rows, s), Spacer(1, 3 * mm)])
            continue
        if re.match(r"^- ", line):
            items = []
            while i < len(lines) and re.match(r"^- ", lines[i]):
                items.append(ListItem(Paragraph(inline(lines[i][2:]), s["body"]), leftIndent=8))
                i += 1
            story.append(ListFlowable(items, bulletType="bullet", start="circle", leftIndent=16, bulletFontName="GalenSans", bulletFontSize=7, bulletColor=GREEN, spaceAfter=4))
            continue
        if re.match(r"^\d+\. ", line):
            items = []
            while i < len(lines) and re.match(r"^\d+\. ", lines[i]):
                value = re.sub(r"^\d+\. ", "", lines[i])
                items.append(ListItem(Paragraph(inline(value), s["body"]), leftIndent=8))
                i += 1
            story.append(ListFlowable(items, bulletType="1", leftIndent=18, bulletFontName="GalenSans-Bold", bulletFontSize=8, bulletColor=GREEN, spaceAfter=4))
            continue
        paragraph_lines = [line]
        i += 1
        while i < len(lines):
            next_line = lines[i].rstrip()
            if not next_line:
                break
            if next_line.startswith(("#", ">", "```", "|", "- ")) or re.match(r"^\d+\. ", next_line):
                break
            paragraph_lines.append(next_line)
            i += 1
        story.append(Paragraph(inline(" ".join(paragraph_lines)), s["body"]))
    return story


def build() -> Path:
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    register_fonts()
    s = styles()
    doc = BaseDocTemplate(
        str(OUTPUT), pagesize=A4, leftMargin=LEFT, rightMargin=RIGHT,
        topMargin=TOP, bottomMargin=BOTTOM,
        title="Galen Alpha 自由探索手册",
        author="Galen Alpha Program",
        subject="受邀体验者安装、探索、安全边界与问题反馈指南",
    )
    content_frame = Frame(LEFT, BOTTOM, PAGE_W - LEFT - RIGHT, PAGE_H - TOP - BOTTOM, id="content")
    cover_frame = Frame(LEFT, 16 * mm, PAGE_W - LEFT - RIGHT, PAGE_H - 28 * mm, id="cover")
    doc.addPageTemplates([
        PageTemplate(id="cover", frames=[cover_frame], onPage=cover_background, autoNextPageTemplate="main"),
        PageTemplate(id="main", frames=[content_frame], onPage=header_footer),
    ])
    story = cover_story(s) + toc_story(s) + parse_manual(s)
    doc.build(story)
    print(OUTPUT)
    return OUTPUT


if __name__ == "__main__":
    build()
