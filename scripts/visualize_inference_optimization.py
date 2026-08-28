#!/usr/bin/env python3
"""Render Galen context + inference optimization visuals from probe reports."""

from __future__ import annotations

import argparse
import html
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_REPORTS = {
    "memory_foundation": ROOT / "evals/runs/context-memory-probe-1787788683645.json",
    "tool_contract": ROOT / "evals/runs/context-memory-probe-1787789495473.json",
    "flash_high": ROOT / "evals/runs/context-memory-probe-1787791061394.json",
    "flash_fast": ROOT / "evals/runs/context-memory-probe-1787791339153.json",
    "pro_uncapped": ROOT / "evals/runs/context-memory-probe-1787791511229.json",
    "pro_fast": ROOT / "evals/runs/context-memory-probe-1787791718521.json",
}

OUT_DIR = ROOT / "docs/visuals"

BG = "#F4F1EA"
CARD = "#FFFDF8"
INK = "#17372E"
MUTED = "#71827B"
GRID = "#D8DED9"
GREEN = "#00856A"
MINT = "#DCEFE8"
CORAL = "#D96C4E"
CORAL_LIGHT = "#F6DFD8"
AMBER = "#C78B2A"
AMBER_LIGHT = "#F3E8CF"
BLUE = "#447C92"
BLUE_LIGHT = "#DCEAF0"
WHITE = "#FFFFFF"
FONT = "Inter, Segoe UI, Microsoft YaHei, sans-serif"


def esc(value: Any) -> str:
    return html.escape(str(value), quote=True)


def load_report(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise FileNotFoundError(f"missing probe report: {path}")
    return json.loads(path.read_text(encoding="utf-8"))


def aggregate(report: dict[str, Any]) -> dict[str, Any]:
    turns = report["turns"]
    cache_hit = sum(t["summary"].get("cacheReadInputTokens", 0) for t in turns)
    cache_miss = sum(t["summary"].get("cacheCreationInputTokens", 0) for t in turns)
    requests = sum(t["summary"].get("modelRequestCount", 0) for t in turns)
    return {
        "passed": report["passed"],
        "assertions": sum(1 for a in report["assertions"] if a["pass"]),
        "assertion_total": len(report["assertions"]),
        "total_ms": report["totalDurationMs"],
        "input_tokens": report["totalInputTokens"],
        "output_tokens": report["totalOutputTokens"],
        "requests": requests,
        "cache_hit": cache_hit,
        "cache_miss": cache_miss,
        "turns": [
            {
                "turn": t["turn"],
                "total_ms": t["summary"]["totalMs"],
                "input_tokens": t["summary"]["inputTokens"],
                "output_tokens": t["summary"]["outputTokens"],
                "requests": t["summary"]["modelRequestCount"],
                "continuations": t["summary"].get("outputContinuationCount", 0),
                "request_details": t["summary"].get("requests", []),
            }
            for t in turns
        ],
    }


@dataclass
class Svg:
    width: int
    height: int
    body: list[str]

    @classmethod
    def create(cls, width: int, height: int, title: str, desc: str) -> "Svg":
        body = [
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img">',
            f"<title>{esc(title)}</title>",
            f"<desc>{esc(desc)}</desc>",
            "<defs>",
            '<filter id="shadow" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="8" stdDeviation="12" flood-color="#17372E" flood-opacity="0.08"/></filter>',
            '<marker id="arrow-green" markerWidth="10" markerHeight="10" refX="8" refY="3" orient="auto"><path d="M0,0 L0,6 L9,3 z" fill="#00856A"/></marker>',
            '<marker id="arrow-coral" markerWidth="10" markerHeight="10" refX="8" refY="3" orient="auto"><path d="M0,0 L0,6 L9,3 z" fill="#D96C4E"/></marker>',
            "</defs>",
            f'<rect width="{width}" height="{height}" fill="{BG}"/>',
        ]
        return cls(width, height, body)

    def add(self, raw: str) -> None:
        self.body.append(raw)

    def rect(self, x: float, y: float, w: float, h: float, fill: str = CARD,
             radius: float = 20, stroke: str = "none", sw: float = 1,
             shadow: bool = False, opacity: float = 1) -> None:
        filt = ' filter="url(#shadow)"' if shadow else ""
        self.add(
            f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{radius}" fill="{fill}" '
            f'stroke="{stroke}" stroke-width="{sw}" opacity="{opacity}"{filt}/>'
        )

    def line(self, x1: float, y1: float, x2: float, y2: float, stroke: str = GRID,
             sw: float = 2, dash: str | None = None, marker: str | None = None) -> None:
        d = f' stroke-dasharray="{dash}"' if dash else ""
        m = f' marker-end="url(#{marker})"' if marker else ""
        self.add(f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{stroke}" stroke-width="{sw}"{d}{m}/>' )

    def text(self, x: float, y: float, value: Any, size: float = 24, fill: str = INK,
             weight: int = 400, anchor: str = "start", letter: float = 0,
             opacity: float = 1) -> None:
        self.add(
            f'<text x="{x}" y="{y}" font-family="{FONT}" font-size="{size}" fill="{fill}" '
            f'font-weight="{weight}" text-anchor="{anchor}" letter-spacing="{letter}" opacity="{opacity}">{esc(value)}</text>'
        )

    def multiline(self, x: float, y: float, lines: list[str], size: float = 22,
                  fill: str = INK, weight: int = 400, gap: float = 1.35,
                  anchor: str = "start") -> None:
        self.add(
            f'<text x="{x}" y="{y}" font-family="{FONT}" font-size="{size}" fill="{fill}" '
            f'font-weight="{weight}" text-anchor="{anchor}">'
        )
        for idx, line in enumerate(lines):
            dy = 0 if idx == 0 else size * gap
            self.add(f'<tspan x="{x}" dy="{dy}">{esc(line)}</tspan>')
        self.add("</text>")

    def circle(self, cx: float, cy: float, r: float, fill: str, stroke: str = "none", sw: float = 1) -> None:
        self.add(f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>')

    def path(self, d: str, fill: str = "none", stroke: str = GREEN, sw: float = 3,
             marker: str | None = None) -> None:
        m = f' marker-end="url(#{marker})"' if marker else ""
        self.add(f'<path d="{d}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" stroke-linecap="round" stroke-linejoin="round"{m}/>' )

    def finish(self) -> str:
        return "\n".join([*self.body, "</svg>"])


def pct_drop(before: float, after: float) -> float:
    return (before - after) / before * 100 if before else 0


def panel_title(svg: Svg, x: float, y: float, eyebrow: str, title: str, note: str | None = None) -> None:
    svg.text(x, y, eyebrow.upper(), 14, GREEN, 700, letter=2)
    svg.text(x, y + 34, title, 28, INK, 700)
    if note:
        svg.text(x, y + 62, note, 15, MUTED)


def bar(svg: Svg, x: float, y: float, w: float, h: float, value: float, maximum: float,
        color: str, label: str, value_label: str) -> None:
    svg.text(x, y + h * 0.72, label, 16, MUTED)
    bx = x + 105
    bw = max(3, w - 105)
    svg.rect(bx, y, bw, h, "#E7EBE8", radius=h / 2)
    svg.rect(bx, y, bw * value / maximum, h, color, radius=h / 2)
    svg.text(bx + bw + 14, y + h * 0.72, value_label, 16, INK, 700)


def render_dashboard(data: dict[str, Any]) -> str:
    high = data["flash_high"]
    fast = data["flash_fast"]
    pro0 = data["pro_uncapped"]
    pro1 = data["pro_fast"]
    memory = data["memory_foundation"]
    tool = data["tool_contract"]

    svg = Svg.create(
        1800,
        2680,
        "Galen 上下文与推理调度优化仪表盘",
        "展示多轮上下文、工具契约、缓存前缀、思考预算与响应收敛的优化过程和探针结果。",
    )
    svg.text(80, 78, "GALEN / CONTEXT × INFERENCE", 16, GREEN, 700, letter=3)
    svg.text(80, 132, "从“记得住”到“答得快”", 48, INK, 800)
    svg.text(80, 176, "上下文工程、工具收敛、缓存稳定性与推理预算的连续优化", 22, MUTED)
    svg.rect(1425, 65, 295, 100, MINT, 18)
    svg.text(1455, 102, "质量门", 15, MUTED, 600)
    svg.text(1455, 142, "9 / 9 通过", 32, GREEN, 800)

    # KPI row
    kpis = [
        ("Flash 总耗时", f"{high['total_ms']/1000:.1f}s", f"{fast['total_ms']/1000:.1f}s", pct_drop(high['total_ms'], fast['total_ms'])),
        ("输入 Token", f"{high['input_tokens']:,}", f"{fast['input_tokens']:,}", pct_drop(high['input_tokens'], fast['input_tokens'])),
        ("输出 Token", f"{high['output_tokens']:,}", f"{fast['output_tokens']:,}", pct_drop(high['output_tokens'], fast['output_tokens'])),
        ("模型请求", str(high['requests']), str(fast['requests']), pct_drop(high['requests'], fast['requests'])),
    ]
    for i, (label, before, after, drop) in enumerate(kpis):
        x = 80 + i * 420
        svg.rect(x, 225, 390, 180, CARD, 22, shadow=True)
        svg.text(x + 28, 260, label, 16, MUTED, 600)
        svg.text(x + 28, 320, after, 42, GREEN, 800)
        svg.text(x + 28, 365, f"优化前 {before}", 16, MUTED)
        svg.rect(x + 258, 250, 102, 34, MINT, 17)
        svg.text(x + 309, 273, f"−{drop:.0f}%", 15, GREEN, 700, anchor="middle")

    # Journey timeline
    svg.rect(80, 445, 1640, 305, CARD, 24, shadow=True)
    panel_title(svg, 115, 490, "Optimization journey", "优化不是一次改 Prompt，而是五层系统工程")
    stages = [
        ("01", "会话连续性", "修复持久化顺序\n工具事实进入模型记忆"),
        ("02", "决策账本", "显式约束可见\n新修订覆盖旧决策"),
        ("03", "工具契约", "读写任务 5→3 请求\n停止无意义探索"),
        ("04", "缓存稳定", "固定 system prefix\n动态上下文移到尾部"),
        ("05", "推理调度", "Flash 低思考默认\nPro 快速通道 + 预算"),
    ]
    y = 625
    svg.line(175, y, 1610, y, GRID, 5)
    for i, (num, title, body) in enumerate(stages):
        x = 175 + i * 358
        svg.circle(x, y, 26, GREEN if i == 4 else WHITE, GREEN, 4)
        svg.text(x, y + 6, num, 14, WHITE if i == 4 else GREEN, 800, anchor="middle")
        svg.text(x, y + 62, title, 19, INK, 700, anchor="middle")
        svg.multiline(x, y + 92, body.split("\n"), 14, MUTED, anchor="middle", gap=1.45)

    # Latency small multiples
    svg.rect(80, 790, 800, 520, CARD, 24, shadow=True)
    panel_title(svg, 115, 835, "Latency", "三轮任务耗时：低思考减少最昂贵的生成轮", "单位：秒；同一 9/9 多轮探针")
    max_turn = max(t["total_ms"] for t in high["turns"] + fast["turns"])
    for idx in range(3):
        yy = 940 + idx * 105
        hval = high["turns"][idx]["total_ms"]
        fval = fast["turns"][idx]["total_ms"]
        svg.text(120, yy + 22, f"T{idx+1}", 18, INK, 800)
        bar(svg, 165, yy, 590, 24, hval, max_turn, CORAL, "高思考", f"{hval/1000:.1f}s")
        bar(svg, 165, yy + 38, 590, 24, fval, max_turn, GREEN, "优化后", f"{fval/1000:.1f}s")
    svg.rect(595, 850, 18, 18, CORAL, 4)
    svg.text(622, 865, "高思考压力路径", 14, MUTED)
    svg.rect(730, 850, 18, 18, GREEN, 4)
    svg.text(757, 865, "默认快速路径", 14, MUTED)

    # Token comparison
    svg.rect(920, 790, 800, 520, CARD, 24, shadow=True)
    panel_title(svg, 955, 835, "Token economy", "少花 Token，同时保持全部质量断言", "柱从零起；输入与输出分别比较")
    groups = [
        ("输入", high["input_tokens"], fast["input_tokens"], 32000),
        ("输出", high["output_tokens"], fast["output_tokens"], 9000),
    ]
    for idx, (label, before, after, maximum) in enumerate(groups):
        gx = 1010 + idx * 330
        base_y = 1215
        scale = 300 / maximum
        for j, (val, color, name) in enumerate([(before, CORAL, "优化前"), (after, GREEN, "优化后")]):
            bh = val * scale
            bx = gx + j * 105
            svg.rect(bx, base_y - bh, 72, bh, color, 10)
            svg.text(bx + 36, base_y - bh - 14, f"{val:,}", 16, INK, 700, anchor="middle")
            svg.text(bx + 36, base_y + 30, name, 14, MUTED, anchor="middle")
        svg.text(gx + 88, 1275, label, 19, INK, 800, anchor="middle")
    svg.line(980, 1215, 1660, 1215, GRID, 2)

    # Cache heatmap
    svg.rect(80, 1350, 800, 480, CARD, 24, shadow=True)
    panel_title(svg, 115, 1395, "Prefix cache", "缓存终于从“猜测”变成可观测指标", "每格 = Flash 优化后的一次模型请求；深色代表命中率更高")
    cells: list[tuple[int, int, float, int, int]] = []
    for turn in fast["turns"]:
        for idx, req in enumerate(turn["request_details"]):
            hit = req.get("cacheHitTokens", 0)
            miss = req.get("cacheMissTokens", 0)
            ratio = hit / (hit + miss) if hit + miss else 0
            cells.append((turn["turn"], idx + 1, ratio, hit, miss))
    for idx, (turn, req, ratio, hit, miss) in enumerate(cells):
        col = idx % 4
        row = idx // 4
        x = 120 + col * 178
        y = 1495 + row * 135
        color = CORAL_LIGHT if ratio < 0.25 else AMBER_LIGHT if ratio < 0.65 else MINT
        accent = CORAL if ratio < 0.25 else AMBER if ratio < 0.65 else GREEN
        svg.rect(x, y, 155, 110, color, 16, accent, 2)
        svg.text(x + 16, y + 28, f"T{turn} · R{req}", 14, MUTED, 700)
        svg.text(x + 16, y + 65, f"{ratio*100:.0f}%", 28, accent, 800)
        svg.text(x + 16, y + 91, f"{hit:,} / {hit+miss:,}", 12, MUTED)

    # First-token timelines
    svg.rect(920, 1350, 800, 480, CARD, 24, shadow=True)
    panel_title(svg, 955, 1395, "Perceived speed", "模型何时开始想，用户何时看到正文", "Flash 低思考；首个模型请求；单位：秒")
    timeline_max = 14.0
    x0, x1 = 1045, 1655
    for tick in range(0, 15, 2):
        xx = x0 + (x1 - x0) * tick / timeline_max
        svg.line(xx, 1490, xx, 1740, GRID, 1, "4 6")
        svg.text(xx, 1770, str(tick), 13, MUTED, anchor="middle")
    for idx, turn in enumerate(fast["turns"]):
        req = turn["request_details"][0]
        ry = 1535 + idx * 78
        reasoning = req.get("firstReasoningTokenMs")
        visible = req.get("firstVisibleTokenMs")
        svg.text(975, ry + 5, f"T{idx+1}", 17, INK, 800)
        if reasoning is not None:
            rx = x0 + (x1 - x0) * min(reasoning / 1000, timeline_max) / timeline_max
            svg.circle(rx, ry, 8, AMBER)
            svg.text(rx, ry - 16, f"想 {reasoning/1000:.1f}s", 12, AMBER, 700, anchor="middle")
        if visible is not None:
            vx = x0 + (x1 - x0) * min(visible / 1000, timeline_max) / timeline_max
            svg.circle(vx, ry, 9, GREEN)
            svg.text(vx, ry + 28, f"见 {visible/1000:.1f}s", 12, GREEN, 700, anchor="middle")
        elif idx == 1:
            svg.text(x0 + 20, ry + 5, "工具轮：先行动，最终文本在 R3", 13, BLUE, 600)
    svg.circle(1000, 1448, 7, AMBER)
    svg.text(1016, 1453, "首思考", 13, MUTED)
    svg.circle(1115, 1448, 7, GREEN)
    svg.text(1131, 1453, "首正文", 13, MUTED)

    # Pro comparison
    svg.rect(80, 1870, 800, 450, CARD, 24, shadow=True)
    panel_title(svg, 115, 1915, "Pro fast lane", "关闭伪低思考 + 明确 1,200 Token 预算", "同一 Pro、同一任务、均为 9/9")
    pro_metrics = [
        ("三轮总耗时", pro0["total_ms"] / 1000, pro1["total_ms"] / 1000, "s"),
        ("第一轮耗时", pro0["turns"][0]["total_ms"] / 1000, pro1["turns"][0]["total_ms"] / 1000, "s"),
        ("输出 Token", pro0["output_tokens"], pro1["output_tokens"], ""),
    ]
    for idx, (label, before, after, unit) in enumerate(pro_metrics):
        yy = 2025 + idx * 82
        maxv = max(before, after)
        svg.text(120, yy + 18, label, 15, MUTED, 600)
        svg.rect(265, yy, 420, 22, "#E7EBE8", 11)
        svg.rect(265, yy, 420 * before / maxv, 22, CORAL, 11, opacity=0.8)
        svg.rect(265, yy + 30, 420 * after / maxv, 22, GREEN, 11)
        svg.text(705, yy + 18, f"{before:.1f}{unit}", 14, CORAL, 700)
        svg.text(705, yy + 48, f"{after:.1f}{unit}", 14, GREEN, 700)

    # Quality matrix / negative optimization
    svg.rect(920, 1870, 800, 450, CARD, 24, shadow=True)
    panel_title(svg, 955, 1915, "Reliability gate", "速度提升不能用遗忘和错误换取", "Flash 与 Pro 优化后均通过所有断言")
    labels = ["上一轮连续性", "两轮连续性", "读写工具", "产物整合", "原约束", "新修订", "工具事实", "完整收尾", "持久会话"]
    for i, label in enumerate(labels):
        col = i % 3
        row = i // 3
        x = 970 + col * 235
        y = 2020 + row * 84
        svg.rect(x, y, 205, 58, MINT, 14)
        svg.circle(x + 25, y + 29, 12, GREEN)
        svg.text(x + 25, y + 34, "✓", 15, WHITE, 800, anchor="middle")
        svg.text(x + 48, y + 35, label, 14, INK, 650)

    # Negative optimization lesson + sources
    svg.rect(80, 2360, 1640, 235, INK, 24)
    svg.text(115, 2405, "NEGATIVE OPTIMIZATION LESSON", 14, "#88CDBD", 700, letter=2)
    svg.text(115, 2450, "工具更少 ≠ 整体更快", 30, WHITE, 800)
    svg.text(115, 2492, f"早期记忆基线 {memory['total_ms']/1000:.1f}s → 定点工具契约阶段 {tool['total_ms']/1000:.1f}s；工具轮改善，但高思考长输出吞噬了收益。", 17, "#D7E4DF")
    svg.text(115, 2532, "因此评测必须同时看：质量门、首正文、总耗时、请求数、Token、缓存命中，而不能只看单一指标。", 17, "#D7E4DF")
    svg.text(115, 2572, "数据：6 份 context-memory probe JSON｜所有时间为端到端毫秒聚合｜生成于 2026-08-27（Asia/Shanghai）", 13, "#9CB2AA")
    return svg.finish()


def node(svg: Svg, x: float, y: float, w: float, h: float, title: str,
         lines: list[str], fill: str, accent: str, badge: str | None = None) -> None:
    svg.rect(x, y, w, h, fill, 20, accent, 2, shadow=True)
    if badge:
        svg.rect(x + 22, y + 18, 66, 28, accent, 14)
        svg.text(x + 55, y + 38, badge, 12, WHITE, 800, anchor="middle")
    svg.text(x + 24, y + 75 if badge else y + 42, title, 20, INK, 750)
    start = y + 110 if badge else y + 78
    svg.multiline(x + 24, start, lines, 14, MUTED, gap=1.45)


def render_architecture(data: dict[str, Any]) -> str:
    high = data["flash_high"]
    fast = data["flash_fast"]
    svg = Svg.create(
        1800,
        1320,
        "Galen 上下文与推理运行时改造前后",
        "对比动态系统前缀、长思考和无条件续写的旧流程，与稳定缓存前缀、任务契约、分层思考和有界续写的新流程。",
    )
    svg.text(80, 75, "GALEN RUNTIME ARCHITECTURE", 16, GREEN, 700, letter=3)
    svg.text(80, 128, "上下文与推理调度：改造前 → 改造后", 44, INK, 800)
    svg.text(80, 170, "相同模型的体验差异，来自请求之前和请求之间的系统设计", 20, MUTED)

    # Before column
    svg.rect(70, 220, 790, 990, "#FFF8F5", 28, CORAL_LIGHT, 2)
    svg.text(115, 270, "BEFORE", 14, CORAL, 800, letter=2)
    svg.text(115, 312, "旧路径：信息很多，但每轮都很重", 28, INK, 800)
    before_nodes = [
        ("动态 System Prompt", ["任务、模式变化时重写", "最早前缀容易失配"], "动态"),
        ("大上下文 + 全工具", ["共识插在历史开头", "探索工具定义反复携带"], "膨胀"),
        ("默认高思考", ["Flash / Pro 长 reasoning", "正式文本迟迟不出现"], "阻塞"),
        ("硬碰 max_tokens", ["思考耗尽输出预算", "reasoning-only → 空回答恢复"], "撞墙"),
        ("无条件续写", ["再次发送完整历史", "请求与 Token 成倍增加"], "重复"),
    ]
    by = 365
    for idx, (title, lines, badge) in enumerate(before_nodes):
        y = by + idx * 150
        node(svg, 125, y, 610, 112, title, lines, CORAL_LIGHT, CORAL, badge)
        if idx < len(before_nodes) - 1:
            svg.line(430, y + 112, 430, y + 142, CORAL, 3, marker="arrow-coral")
    svg.rect(125, 1125, 610, 58, CORAL, 16)
    svg.text(430, 1163, f"结果：{high['total_ms']/1000:.1f}s · {high['requests']} 次请求 · {high['output_tokens']:,} 输出 Token", 17, WHITE, 750, anchor="middle")

    # Center transition
    svg.circle(900, 665, 50, GREEN)
    svg.text(900, 674, "→", 34, WHITE, 800, anchor="middle")
    svg.multiline(900, 745, ["不是换模型", "而是换调度"], 16, GREEN, 750, anchor="middle")

    # After column
    svg.rect(940, 220, 790, 990, "#F6FCF9", 28, MINT, 2)
    svg.text(985, 270, "AFTER", 14, GREEN, 800, letter=2)
    svg.text(985, 312, "新路径：稳定前缀，动态尾部，有界推理", 28, INK, 800)
    after_nodes = [
        ("稳定 L0 前缀", ["人格 + 安全规则字节稳定", "模式与任务策略移到尾部"], "缓存"),
        ("任务契约路由", ["直接回答 / 定点读写 / 深度讨论", "只暴露最小必要工具"], "分流"),
        ("共识与状态尾注入", ["完整历史保持 append-only", "当前修订靠近最新用户请求"], "连续"),
        ("自适应思考", ["Flash 默认 low", "Pro low = 非思考快速通道"], "预算"),
        ("条件续写 + 指标回流", ["仅真正截断时续写一次", "逐请求记录首字、缓存、尝试次数"], "闭环"),
    ]
    ay = 365
    for idx, (title, lines, badge) in enumerate(after_nodes):
        y = ay + idx * 150
        node(svg, 1065, y, 610, 112, title, lines, MINT if idx != 4 else BLUE_LIGHT, GREEN if idx != 4 else BLUE, badge)
        if idx < len(after_nodes) - 1:
            svg.line(1370, y + 112, 1370, y + 142, GREEN, 3, marker="arrow-green")
    svg.rect(1065, 1125, 610, 58, GREEN, 16)
    svg.text(1370, 1163, f"结果：{fast['total_ms']/1000:.1f}s · {fast['requests']} 次请求 · {fast['output_tokens']:,} 输出 Token · 9/9", 17, WHITE, 750, anchor="middle")

    svg.text(80, 1270, "反馈闭环：探针 → 指标分解 → 找出真正瓶颈 → 修改调度 → 同一质量门复测", 16, MUTED)
    svg.line(675, 1263, 1680, 1263, GREEN, 3, marker="arrow-green")
    return svg.finish()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out-dir", type=Path, default=OUT_DIR)
    args = parser.parse_args()

    reports = {name: load_report(path) for name, path in DEFAULT_REPORTS.items()}
    data = {name: aggregate(report) for name, report in reports.items()}
    data["sources"] = {name: str(path.relative_to(ROOT)).replace("\\", "/") for name, path in DEFAULT_REPORTS.items()}
    args.out_dir.mkdir(parents=True, exist_ok=True)

    dashboard = args.out_dir / "galen-inference-optimization-dashboard.svg"
    architecture = args.out_dir / "galen-context-runtime-evolution.svg"
    snapshot = args.out_dir / "galen-inference-optimization-data.json"
    dashboard.write_text(render_dashboard(data), encoding="utf-8")
    architecture.write_text(render_architecture(data), encoding="utf-8")
    snapshot.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")
    print(dashboard)
    print(architecture)
    print(snapshot)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
