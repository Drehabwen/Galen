"""Build a formal, submission-ready report from Galen's real user feedback forms."""
from pathlib import Path

from docx import Document
from docx.enum.section import WD_SECTION
from docx.enum.style import WD_STYLE_TYPE
from docx.enum.table import WD_ALIGN_VERTICAL
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.oxml import OxmlElement
from docx.oxml.ns import qn
from docx.shared import Cm, Pt

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "output" / "competition-evidence"
OUT.mkdir(parents=True, exist_ok=True)
DOCX = OUT / "Galen_两轮真实科研任务用户测评与结果核验报告.docx"


def set_font(run, name="宋体", size=10.5, bold=False):
    run.font.name = name
    run._element.rPr.rFonts.set(qn("w:eastAsia"), name)
    run._element.rPr.rFonts.set(qn("w:ascii"), "Times New Roman")
    run._element.rPr.rFonts.set(qn("w:hAnsi"), "Times New Roman")
    run.font.size = Pt(size)
    run.font.bold = bold
    run.font.color.rgb = None


def set_cell_border(cell, **kwargs):
    tc_pr = cell._tc.get_or_add_tcPr()
    tc_borders = tc_pr.first_child_found_in("w:tcBorders")
    if tc_borders is None:
        tc_borders = OxmlElement("w:tcBorders")
        tc_pr.append(tc_borders)
    for edge in ("top", "bottom", "left", "right", "insideH", "insideV"):
        if edge not in kwargs:
            continue
        tag = "w:" + edge
        element = tc_borders.find(qn(tag))
        if element is None:
            element = OxmlElement(tag)
            tc_borders.append(element)
        for key, value in kwargs[edge].items():
            element.set(qn("w:" + key), str(value))


def write_cell(cell, text, bold=False, center=False):
    cell.text = ""
    p = cell.paragraphs[0]
    p.alignment = WD_ALIGN_PARAGRAPH.CENTER if center else WD_ALIGN_PARAGRAPH.LEFT
    p.paragraph_format.space_after = Pt(0)
    p.paragraph_format.line_spacing = 1.25
    r = p.add_run(text)
    set_font(r, size=9.5, bold=bold)
    cell.vertical_alignment = WD_ALIGN_VERTICAL.CENTER
    tc_pr = cell._tc.get_or_add_tcPr()
    mar = OxmlElement("w:tcMar")
    for side in ("top", "start", "bottom", "end"):
        node = OxmlElement(f"w:{side}")
        node.set(qn("w:w"), "90")
        node.set(qn("w:type"), "dxa")
        mar.append(node)
    tc_pr.append(mar)


def triline_table(doc, headers, rows, widths):
    table = doc.add_table(rows=1, cols=len(headers))
    table.autofit = False
    for idx, (cell, text, width) in enumerate(zip(table.rows[0].cells, headers, widths)):
        cell.width = Cm(width)
        write_cell(cell, text, bold=True, center=True)
        set_cell_border(cell, top={"val": "single", "sz": "12", "color": "000000"}, bottom={"val": "single", "sz": "6", "color": "000000"})
    for row_index, row in enumerate(rows):
        cells = table.add_row().cells
        for cell, text, width in zip(cells, row, widths):
            cell.width = Cm(width)
            write_cell(cell, text, center=False)
        if row_index == len(rows) - 1:
            for cell in cells:
                set_cell_border(cell, bottom={"val": "single", "sz": "12", "color": "000000"})
    doc.add_paragraph().paragraph_format.space_after = Pt(2)
    return table


def paragraph(doc, text="", bold=False, size=10.5, align=None, before=0, after=7):
    p = doc.add_paragraph()
    if align is not None:
        p.alignment = align
    p.paragraph_format.space_before = Pt(before)
    p.paragraph_format.space_after = Pt(after)
    p.paragraph_format.line_spacing = 1.5
    run = p.add_run(text)
    set_font(run, size=size, bold=bold)
    return p


def heading(doc, text, level=1):
    p = doc.add_paragraph()
    p.paragraph_format.space_before = Pt(13 if level == 1 else 8)
    p.paragraph_format.space_after = Pt(6)
    p.paragraph_format.keep_with_next = True
    run = p.add_run(text)
    set_font(run, size=15 if level == 1 else 12, bold=True)
    return p


def reviewer_form(doc, reviewer_label):
    heading(doc, f"附录 B 结果复核表 {reviewer_label}", 1)
    paragraph(doc, "用途：对同一批真实任务产物进行独立复核。请依据原始任务、Galen 生成结果、链接与数据回执打分，并独立完成本表。")
    paragraph(doc, "评审人：__________________    身份或专业：__________________    日期：__________________", size=10)
    paragraph(doc, "评分规则：每项 1 分为满足，0 分为不满足，NA 为该任务不适用。两份复核表汇总后计算总体一致率与 Cohen’s kappa。", size=10)
    rows = [
        ["U01", "糖尿病膳食干预检索", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "____/5"],
        ["U02", "糖尿病足溃疡综述", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "____/5"],
        ["U03", "针灸实验课题设计", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "____/5"],
        ["U04", "皮肤针美容证据梳理", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "____/5"],
        ["U05", "卒中上肢康复研究", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "____/5"],
        ["U06", "推拿运动性疲劳", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "____/5"],
        ["U07", "短道速滑疲劳选题", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "____/5"],
        ["U08", "TMS 脑卒中系统综述", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "□1 □0 □NA", "____/5"],
    ]
    triline_table(doc, ["编号", "任务", "意图匹配", "证据可开", "数据/条件保留", "产物可用", "结论可追溯", "合计"], rows, [1.0, 3.1, 2.0, 1.8, 2.6, 1.8, 2.1, 1.2])
    paragraph(doc, "备注（如判 0 分，请注明所在位置或截图编号）：", size=10, after=3)
    for _ in range(2):
        paragraph(doc, "________________________________________________________________________________________", size=10, after=4)


def build():
    doc = Document()
    section = doc.sections[0]
    section.top_margin, section.bottom_margin = Cm(2.3), Cm(2.2)
    section.left_margin, section.right_margin = Cm(2.5), Cm(2.5)
    normal = doc.styles["Normal"]
    normal.font.name = "宋体"
    normal._element.rPr.rFonts.set(qn("w:eastAsia"), "宋体")
    normal.font.size = Pt(10.5)

    # Header/footer: restrained black, no decorative rules.
    hp = section.header.paragraphs[0]
    hp.alignment = WD_ALIGN_PARAGRAPH.RIGHT
    set_font(hp.add_run("Galen 两轮真实科研任务用户测评与结果核验报告"), size=8)
    fp = section.footer.paragraphs[0]
    fp.alignment = WD_ALIGN_PARAGRAPH.CENTER
    set_font(fp.add_run("Galen 竞赛材料  2026 年 9 月"), size=8)

    paragraph(doc, "Galen 两轮真实科研任务用户测评与结果核验报告", bold=True, size=19, align=WD_ALIGN_PARAGRAPH.CENTER, before=70, after=12)
    paragraph(doc, "基于 17 份真实任务反馈的版本迭代验证与双人独立复核", size=12, align=WD_ALIGN_PARAGRAPH.CENTER, after=38)
    paragraph(doc, "报告用途", bold=True, size=11, after=4)
    paragraph(doc, "本报告将两轮回收的 Galen 用户测评反馈表转化为评审可阅读的应用验证材料。正文呈现真实任务、问题基线、版本迭代、量化评分、关键能力、典型产物与结果核验流程；原始问卷保留为可追溯底稿。", after=10)
    paragraph(doc, "核心结论", bold=True, size=11, after=4)
    paragraph(doc, "两轮共整理 17 份真实科研任务反馈。上一轮反馈明确了成果预览、文献检索、上下文与任务控制等关键问题；本轮在 8 份真实任务中继续验证改进方向，其中 7 份含完整五维评分，科研意图理解平均得分 4.29 分。报告末附双人独立复核表，用于形成正式的一致性对比数据。", after=10)
    paragraph(doc, "测评区间：2026 年 9 月 1 日至 9 月 13 日    两轮任务反馈：17    当前轮有效五维评分：7", size=10, after=0)
    doc.add_page_break()

    heading(doc, "1 评测设计与材料来源")
    paragraph(doc, "测评对象为使用 Galen 完成真实科研任务的用户，任务覆盖检索策略构建、系统综述准备、课题设计、康复研究选题、数据与证据整理等场景。每位用户记录任务目标、期望产物、五维体验评分、问题复现步骤与最有价值结果。两轮反馈用于建立真实问题基线，并持续验证版本迭代。")
    triline_table(doc, ["项目", "说明"], [
        ["样本来源", "两轮共 17 份 Galen 用户测评反馈表：上一轮 9 份，本轮 8 份"],
        ["任务性质", "真实科研任务，覆盖检索、证据整理、课题设计与康复研究场景"],
        ["量表", "科研意图理解、上下文连续性、结果可核验、任务执行稳定性、整体体验，1 至 5 分"],
        ["分析单位", "以每份有效反馈表为一个任务记录；本轮未填写评分项不计入均值"],
        ["后续复核", "两名评审对同一任务产物独立评分，形成一致率与一致性统计结果"],
    ], [3.2, 12.4])
    doc.add_page_break()
    heading(doc, "2 上一轮反馈形成的问题基线")
    paragraph(doc, "上一轮 9 份真实任务反馈来自 Galen 测试版、v0.1.3 与 v3.0 等版本。该轮反馈为当前版本的产品收敛提供了直接依据：不是泛泛优化，而是围绕用户完成科研任务时真正中断的环节进行改进。")
    triline_table(doc, ["上一轮真实问题", "用户任务中的具体表现", "本轮对应的验证重点"], [
        ["成果预览与交付访问", "PDF 或文档无法完整滚动查看，文件路径无法直接打开，成果预览影响对结果的理解。", "验证 Galen 内部全文预览、产物链接与正式 PDF 交付。"],
        ["检索质量与证据覆盖", "文献与主题脱节、时效性未提示、检索结果混入无关文献或无法导出。", "验证可打开来源、检索结果可追溯、文献筛选与成果引用链。"],
        ["任务过程控制", "工作区前置阻断、错误任务难以停止、跳转后任务中断或节点顺序错乱。", "验证项目状态、细粒度进度、任务队列与连续研究委托。"],
        ["上下文与新课题切换", "已确认条件在多轮对话中遗漏，切换研究主题时旧上下文残留。", "验证项目级研究状态、范围记录与研究委托重置。"],
        ["证据与图表呈现", "证据脉络面板为空，缺少研究产物可视化与标准化图表。", "验证证据脉络、来源链接、SVG 可视化与研究成果预览。"],
    ], [3.4, 6.5, 5.7])
    paragraph(doc, "上一轮原始底稿：反馈.docx。该材料含每项问题的复现步骤、期望结果、实际发生情况与用户原始评价。", size=9.5)
    doc.add_page_break()

    heading(doc, "3 本轮真实任务覆盖")
    triline_table(doc, ["编号", "任务主题", "目标产物", "版本或模式"], [
        ["U01", "2 型糖尿病膳食干预系统综述", "PubMed、CNKI 检索式及逻辑说明", "v0.8.2，讨论"],
        ["U02", "糖尿病足溃疡系统综述", "PubMed 检索策略与文献初步分类", "v0.2.0，讨论"],
        ["U03", "针灸实验课题设计", "课题思路与实验方案", "v0.2.0，自动"],
        ["U04", "皮肤针美容证据梳理", "中医皮肤针相关证据与成果预览", "v0.2.0，自动"],
        ["U05", "卒中后上肢训练强度研究", "数据与文献参考", "v0.2.0"],
        ["U06", "推拿治疗运动性疲劳", "疗效讨论或结果", "v0.1.4，自动"],
        ["U07", "短道速滑运动性疲劳选题", "可执行的小方向与理由", "v3.0，计划"],
        ["U08", "TMS 联合康复训练系统综述", "检索、证据分级及质量评价框架", "v0.1.0，计划与自动"],
    ], [1.0, 4.0, 7.2, 3.4])
    doc.add_page_break()

    heading(doc, "4 本轮用户五维评分结果")
    paragraph(doc, "下表基于 7 份含完整评分的问卷计算均值。评分 1 分为很差，5 分为很好；评分反映用户完成真实任务后的直接体验。")
    triline_table(doc, ["维度", "有效样本", "平均分", "观察"], [
        ["科研意图理解", "7", "4.29", "能够较快理解选题与任务目标，是当前体验中最稳定的优势。"],
        ["上下文连续性", "7", "3.71", "多轮检索条件、既定筛选标准与长对话状态仍需持续保持。"],
        ["结果可信与可核验", "7", "3.86", "用户认可可用初稿与证据整理，但期待更直接的来源与产物访问。"],
        ["任务执行稳定性", "7", "3.43", "工具环境、节点顺序与中止能力是影响闭环体验的主要问题。"],
        ["整体体验", "7", "3.71", "用户认可其作为科研任务起点与推进工具的价值。"],
    ], [4.0, 2.6, 2.2, 8.0])
    paragraph(doc, "说明：陈思琪的反馈表未勾选五维评分，因此不纳入本节均值；该任务的文字反馈仍纳入问题归纳。", size=9.5)
    doc.add_page_break()

    heading(doc, "5 本轮典型发现与迭代依据")
    triline_table(doc, ["发现", "真实反馈表现", "形成的产品改进方向"], [
        ["多轮条件保持", "检索词、年龄与研究类型限定在后续轮次出现遗漏。", "以项目状态保存已确认条件，并在修改前向用户展示当前范围。"],
        ["任务过程可控", "出现工具故障、节点提前执行、错误任务难以中止等情况。", "提供更细粒度的任务进度、停止入口与依赖顺序控制。"],
        ["产物预览与访问", "成果预览滑动、跳转文件位置与全文查看路径被多次提及。", "统一 PDF 预览、产物链接与文件定位入口。"],
        ["科研数据粒度", "康复量表子项、文献去重和数据解析仍影响研究闭环。", "完善量表结构化解析、去重规则与数据接入回执。"],
        ["对话负担", "一次抛出多个关键问题、答案过长或重点不聚焦会增加负担。", "采用单问题推进、先给结论、再按需展开的 PI 对话方式。"],
    ], [3.2, 6.1, 7.5])
    doc.add_page_break()

    heading(doc, "6 结果核验与一致性评测方案")
    paragraph(doc, "从本报告的 8 个真实任务中抽取可访问的最终产物，由两名评审独立判断。每人约 20 至 30 分钟即可完成，能够形成任务质量与结果一致性的量化证据。")
    triline_table(doc, ["步骤", "执行内容", "产出"], [
        ["1", "为 U01 至 U08 分别整理任务原文、最终结果、相关链接、截图或数据回执。", "8 个标准化证据包"],
        ["2", "两名同学不互相讨论，分别填写附录 B 的复核表。", "两份独立人工评分表"],
        ["3", "按任务逐项比较两位评分：相同为一致，不同为不一致；计算总一致率。", "人工一致率"],
        ["4", "若需要更正式的统计，按每个二分类条目计算 Cohen’s kappa。", "一致性统计表"],
        ["5", "Galen 自动整理链接、文件与数据回执，形成结构化证据包。", "系统核验附表"],
    ], [1.2, 10.0, 5.6])
    doc.add_page_break()
    heading(doc, "7 统一评分准则", 2)
    triline_table(doc, ["条目", "判定为 1 分的标准"], [
        ["意图匹配", "最终结果直接回答了该用户的核心科研任务，不偏离题目。"],
        ["证据可开", "报告中的文献、文件或链接可被打开和定位。"],
        ["数据或条件保留", "已确认的对象、筛选条件或数据范围在最终结果中得到保留。"],
        ["产物可用", "检索式、研究方案、表格、报告或 PDF 可被用户直接用于下一步工作。"],
        ["结论可追溯", "结果中的关键结论能回溯到已给出的来源、数据或任务记录。"],
    ], [4.2, 12.6])
    paragraph(doc, "一致率 = 两名评审给出相同判定的条目数 ÷ 两名评审共同完成判定的条目数。若采用 Cohen’s kappa，则对两名评审的 0/1 判定计算。", size=9.5)
    doc.add_page_break()

    reviewer_form(doc, "独立评审表 请打印两份")

    appendix_c_heading = heading(doc, "附录 C Galen 结构化核验记录", 1)
    appendix_c_heading.paragraph_format.page_break_before = True
    paragraph(doc, "本页由 Galen 自动整理，用于核验交付物是否存在、是否可打开、是否有对应数据回执，并作为双人复核时的统一证据索引。")
    rows = [[f"U0{i}", "□存在 □缺失", "□可开 □不可开", "□有 □无", "□已核验 □待核验", "________________"] for i in range(1, 9)]
    triline_table(doc, ["编号", "最终产物", "链接或文件", "数据回执", "状态", "证据位置"], rows, [1.2, 2.7, 2.8, 2.2, 2.4, 4.2])
    paragraph(doc, "原始资料索引：反馈.docx（上一轮 9 份任务反馈）；新建 DOCX 文档(2).docx（本轮 8 份任务反馈）。两份材料均保留任务描述、评分、问题复现记录与用户评价。", size=9.5)

    doc.save(DOCX)
    print(DOCX)


if __name__ == "__main__":
    build()
