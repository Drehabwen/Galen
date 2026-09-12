# Galen × 通用 Agent 对比：先导结果

日期：2026-09-12  
任务卡版本：`evals/cases/paired`（15 张）  
模型配置：DeepSeek v4 Flash；各 CLI 使用隔离配置与同一 API key。  
性质：先导实验，不作为正式统计结论。

## 已执行

| 系统 | 覆盖 | 通过 | 通过率 | 平均总时延 | 工具调用 | 备注 |
|---|---:|---:|---:|---:|---:|---|
| Galen `full_galen` | 15 卡 × 1 | 15 | 100% | 11.0 s | 32 | 当前完整能力基线 |
| OpenCode + DeepSeek | 15 卡 × 1 | 9 | 60% | 31.2 s | 110 | 已完成全套先导 |
| Codex CLI + DeepSeek | PCTX01 × 1 | 0 | — | 51.0 s | 13 | 产物和 6/6 事实均有；启动同步导致工具预算超限 |
| Claude Code + DeepSeek | PCTX01 × 1 | 0 | — | 17.5 s | 1 | 产物生成；严格事实短语为 `12 participants`，未命中卡片要求的 `12 名`；预算上限 $0.20 |

## OpenCode 失败卡

`PCTX02`、`PDATA01`、`PDATA03`、`PTOOL02`、`PTOOL04`、`PTOOL05`。

失败主要落在长上下文、数据时间对齐、工具调用经济性和交付恢复；不是统一的“回答质量”单一问题。每张卡的完整 RunRecord 与原始事件均保留在：

`evals/runs/pilot-opencode-all-v1.jsonl`  
`evals/runs/raw-events/opencode-*.jsonl`

Galen 对齐结果在：

`evals/runs/architecture-v1/architecture-v1-full-pilot-20260912/full_galen/`

随后完成了 OpenCode 的分层重复：PCTX01、PDATA02、PTOOL01 各重复 5 次，共 15 次，15/15 通过；这用于检查三类任务的初步重复性，不与全套 75 次结果合并。

`evals/runs/pilot-opencode-stratified-r5-20260912.jsonl`

## 解释边界

1. 这一轮是每卡一次的先导，不报告显著性、不宣称普适优越性。
2. Galen 与 OpenCode 已覆盖相同 15 张卡；Codex、Claude 目前是适配器 smoke，不能和 15 卡结果直接比较。
3. Claude、OpenCode 的事件格式已做保守归一化；未暴露的 token、工具或检索字段保持空值，不补猜。
4. 正式比较仍按 `evals/experiments/architecture-v1/COMPARISON_PROTOCOL.md` 执行：同一任务卡、同一模型、每卡至少 5 次（论文级结果建议 20 次），并报告 Wilson 区间、配对 bootstrap、McNemar 与成本/时延。

## 结论（仅限本轮）

在当前冻结任务卡上，Galen 的完整能力链路完成 15/15 的先导验收；OpenCode 的全套先导为 9/15，三张代表卡的 5 次重复为 15/15。完整 75 次的 Galen/Codex 重复与 OpenCode 先导/分层重复已经整理进 LaTeX 论文报告：`output/pdf/galen-framework-comparison/galen_framework_comparison_report.pdf`。下一轮仍应把 Codex 的启动事件隔离、Claude 的事实判定与预算策略固定，再扩展到同一覆盖下的多框架重复。
