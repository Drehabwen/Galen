# Galen 架构消融实验：阶段性结果

## 运行口径

- 变体：`full_galen`、`stateless`、`generic_same_runtime`、`no_data_contract`、`no_exec_contract`、`no_evidence_link`。
- 固定条件：同一 provider、同一模型、同一工具集、同一温度/超时/工具预算、同一任务卡。
- 每个任务卡 5 次重复；主指标为硬门通过率，辅指标为工具调用数和总时延。
- 2026-09-11 起 runner 改为不可覆盖的任务卡运行目录；评测 harness 的状态层改为以 `ACTIVE_RESEARCH_CONTEXT` 替换已归档 transcript，避免把旧对话误当作当前状态。
- 2026-09-11 进一步修正 gold-label 泄漏：`required.facts` 与 `required.artifacts` 不再进入 seed/history 或 active context pack。此前批次仅保留为调试记录，不能作为论文结果；新批次必须通过 `scripts/validate_architecture_run.py`。

## 修正后可比较结果

### PCTX02：单字段修订后的约束一致性

| 变体 | 通过率 | 平均工具调用 | 平均输入 token | 平均模型请求 | 中位时延 |
|---|---:|---:|---:|---:|---:|
| full_galen | 5/5 (100%) | 1.00 | 7,504 | 2.00 | 11.5 s |
| no_data_contract | 5/5 (100%) | 1.00 | 7,343 | 2.00 | 9.9 s |
| no_evidence_link | 5/5 (100%) | 1.00 | 6,920 | 2.00 | 8.8 s |
| no_exec_contract | 4/5 (80%) | 5.60 | 40,511 | 6.60 | 15.5 s |
| stateless | 1/5 (20%) | 1.00 | 35,081 | 2.00 | 12.8 s |
| generic_same_runtime | 0/5 (0%) | 5.00 | 119,338 | 6.00 | 17.9 s |

失败模式：无状态/通用基线出现旧事实“样本量 48”泄漏或遗漏 `96 小时`、协议号；关闭执行契约时出现工具预算超限。

### PCTX03：研究范围切换与旧范围隔离

| 变体 | 通过率 | 平均工具调用 | 平均输入 token | 平均模型请求 | 中位时延 |
|---|---:|---:|---:|---:|---:|
| full_galen | 5/5 (100%) | 1.00 | 7,593 | 2.00 | 11.2 s |
| no_data_contract | 5/5 (100%) | 1.00 | 7,349 | 2.00 | 12.3 s |
| no_evidence_link | 5/5 (100%) | 1.00 | 6,910 | 2.00 | 10.1 s |
| no_exec_contract | 4/5 (80%) | 3.80 | 31,623 | 4.80 | 11.3 s |
| stateless | 1/5 (20%) | 1.00 | 34,837 | 2.00 | 10.6 s |
| generic_same_runtime | 1/5 (20%) | 4.00 | 98,268 | 5.00 | 16.6 s |

失败模式：无状态/通用基线把旧研究范围或旧时间窗写回产物；关闭执行契约时出现工具调用预算超限。

### PDATA05：数据来源与采集条件溯源（探索性部分批次）

该卡的 `PDATA05` 记录已完成 5 次，但原批次在 `stateless` 变体前中止，因此只用于定位失败模式，不用于完整变体排名：

| 变体 | 已完成记录 | 通过率 | 平均工具调用 |
|---|---:|---:|---:|
| full_galen | 5 | 5/5 | 3.00 |
| no_data_contract | 5 | 5/5 | 3.00 |
| no_evidence_link | 5 | 5/5 | 3.00 |
| no_exec_contract | 5 | 4/5 | 4.60 |
| generic_same_runtime | 5 | 2/5 | 6.80 |

这张卡的提示词已经直接点名 schema 字段，因此暂时不能作为“数据契约独立增益”的证据；它主要再次显示执行契约对调用经济性和预算稳定性的影响。

## 初步解释

1. **状态层是当前最清晰的增益来源。** 两张范围/修订卡上，`full_galen` 均为 5/5，而无状态与通用基线均为 1/5 或 0/5；这支持“归档对话不应直接进入当前推理，当前状态必须有权威载体”的架构判断。
2. **执行契约体现为经济性与稳定性。** 关闭执行契约后，PCTX02/PCTX03 的通过率降至 80%，工具调用均值升至 5.60/3.80；完整架构均为单次工具调用。
3. **数据契约与证据链接尚未被这两张卡充分激活。** `no_data_contract` 与 `no_evidence_link` 在范围任务上仍可通过，不能据此声称它们没有价值；需由 PDATA04/PDATA05 与需要逐条引用的 PTOOL02 等卡检验。

## 不应过度解读

- 当前仅覆盖 2 张经过 harness 修正的上下文卡，不是完整 15 卡或 20 次正式发布结果。
- PDATA02、PTOOL04、PCTX03 的早期结果使用旧 harness，只作 runner 烟雾/任务可执行性记录，不纳入上述主结论。
- Wilson 区间在 n=5 时很宽；下一阶段需按矩阵覆盖全部 PCTX/PDATA/PTOOL，再进行 case-level bootstrap、效应量和论文图表汇总。

## Gold-leakage 修正后的公开输入 smoke（非论文结果）

为验证修正后的 harness 仍能完整执行，2026-09-11 对 PCTX01–PCTX05 各进行 6 个变体各 1 次运行（PCTX02/03 与 PCTX04/05 分批完成，PCTX01 因工作区隔离修复后重跑）。五张卡的 `full_galen` 均通过 1/1；无状态/通用变体在部分卡上复现旧范围泄漏，`no_exec_contract` 在部分卡上复现预算超限，PCTX05 则各变体均完成。该批次仅用于 smoke，不用于估计效应量；完整记录、manifest 和覆盖校验见对应 `evals/runs/architecture-v1/architecture-v1-pctx*-public*-smoke-r1*/` 目录。

同一公开输入口径下，另对 PDATA01、PDATA04、PTOOL02、PTOOL04、PTOOL05 各进行 6 个变体各 1 次代表性 smoke。`full_galen` 通过 5/5，`generic_same_runtime` 通过 2/5，`no_data_contract` 通过 4/5，`no_evidence_link` 通过 5/5，`no_exec_contract` 通过 1/5，`stateless` 通过 0/5。由于每张卡只有 1 次，这里只用于暴露失败模式和验证执行链，不作为显著性或泛化结论；记录见 `evals/runs/architecture-v1/architecture-v1-data-tool-representative-smoke-r1/`。

随后按最新任务卡重跑 PCTX02/PCTX03 时，provider 返回 HTTP 402（Insufficient Balance），所有变体均为 `model_requests=0`。该批次已被覆盖校验器判定为 provider unavailable，明确排除出任何性能或可靠性统计。

## 可复现入口

- 运行器：`scripts/run_architecture_ablation.py`
- 统计器：`scripts/analyze_architecture_ablation.py`
- 变体矩阵：`evals/experiments/architecture-v1/matrix.json`
- PCTX02 结果：`evals/runs/architecture-v1/architecture-v1-pctx02-statefix3-r5/ablation-summary.md`
- PCTX03 结果：`evals/runs/architecture-v1/architecture-v1-pctx03-statefix-r5/ablation-summary.md`
