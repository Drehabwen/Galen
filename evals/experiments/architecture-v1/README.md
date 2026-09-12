# Galen 架构改进论文：同运行时对比协议 v1

严格的任务定义、主实验/外部产品参考轨、公平性规则、统计方法和路演口径见 [`COMPARISON_PROTOCOL.md`](./COMPARISON_PROTOCOL.md)；机器可读的系统比较矩阵见 [`comparators.json`](./comparators.json)；任务卡冻结哈希见 [`suite_manifest.json`](./suite_manifest.json)。

本目录把 Galen 的产品能力转化为可复现的架构实验。目标不是证明某个模型“更聪明”，而是测量四个系统机制对康复科研任务可靠交付的独立贡献。当前状态为 `runner_hooked_partial_validation`：runner 已接通，PCTX02/PCTX03 已完成 harness 修正后的 5 次重复，尚未覆盖全部 15 张任务卡。

重要口径：评测器不会把 `required.facts` 或 `required.artifacts` 注入模型可见上下文；它们只属于 evaluator-only gold。历史运行在该修正前生成的数字不得回填论文。提交前先运行 `python scripts/validate_architecture_run.py <run-dir>`，只有覆盖矩阵、重复次数、manifest 哈希全部通过的目录，才允许进入论文统计。
Runner 还会把 `run_id + variant` 注入临时工作区标识，避免 Windows 快速复用进程号造成跨变体覆盖误报。
若记录全部在 `model_requests=0` 且 `run_completed=false` 结束，校验器会标记为 provider 不可用（如 402 余额错误）；这类记录只能用于运维排查，不能进入模型或架构排名。
Runner 在单个变体/任务卡发现同样的全零请求后会立即停止后续矩阵，避免在 provider 故障时继续消耗时间和额度。
`manifest.json` 在启动时即写入，状态依次为 `running`、`completed` 或 `provider_unavailable/failed/aborted`；只有 `completed` 才能通过论文数据校验。

## 固定条件

- 任务：`evals/cases/paired/` 的 15 张任务卡（PCTX/PDATA/PTOOL 各 5 张）；
- 输入：同一套脱敏 fixture、同一版本任务契约和同一输出目录布局；
- 模型：同一模型 ID、同一 provider、同一温度和最大输出限制；
- 工具：同一工具 API、同一超时、同一模型请求预算；
- 重复：每张任务卡每个变体至少 5 次，正式版建议 20 次；
- 统计：硬门通过率、Wilson 95% 区间、Pass^5、事实召回、旧范围泄漏、引用可核验率、产物可预览率、工具调用、总时延和失败恢复成本。

## 变体

| 变体 | 关闭项 | 目的 |
|---|---|---|
| `stateless` | 项目状态层 | 测量上下文连续性的增益 |
| `no_data_contract` | RehabID schema、质量状态和溯源 | 测量数据治理层的增益 |
| `no_exec_contract` | 工具白名单、预算和恢复协议 | 测量执行控制层的增益 |
| `no_evidence_link` | 证据到结论的可点击映射 | 测量可核验交付层的增益 |
| `generic_same_runtime` | Galen 专用编排，但保留同 provider/工具 | 公平的通用 Agent 基线 |
| `full_galen` | 全部机制开启 | 最终系统 |

## 硬门

一次运行只有同时满足以下条件才算通过：

1. 必需事实全部保留；
2. 禁止事实和旧范围不泄漏；
3. 必需工具按顺序调用，且不超预算、不循环；
4. 证据任务的关键结论带可打开的 PMID/DOI/来源链接；
5. 产物存在、非空、可预览，且与任务契约一致。

## 分析顺序

先看硬门和按卡分层结果，再看 Wilson 下界与 Pass^5，最后解释工具调用和时延。架构增益报告为相对于 `generic_same_runtime` 的差值；当前已经完成的外部 Codex + DeepSeek 结果仅作为先导，不替代同运行时主实验。

## 实施状态

- [x] 任务矩阵与指标口径固定；
- [x] 现有 15 卡先导对比和 Python 统计脚本；
- [x] LaTeX 架构论文初稿；
- [x] 在 eval runner 中增加上述架构开关；
- [ ] 完成同 provider、同工具的 6 变体正式运行（当前完成 2/15 卡）；
- [ ] 将消融结果回填论文的表 3 和图 6；
- [ ] 发布任务契约、统计脚本和可公开脱敏 fixture。
- [x] 移除 gold label 注入并增加运行覆盖/manifest 校验器；

阶段性结果见 [`RESULTS.md`](./RESULTS.md)。

## 可复现命令

```powershell
# 先校验任务契约，不调用模型
& D:\DEV\Galen-new\evals\experiments\paired-v1\run.ps1 -ValidateOnly

# 每个变体执行同一套 paired 任务；运行目录非空时 runner 会拒绝覆盖
python D:\DEV\Galen-new\scripts\run_architecture_ablation.py --cases-filter PCTX02 --repeat 5 --run-id architecture-v1-pctx02-r5

# 汇总单次运行
python D:\DEV\Galen-new\scripts\analyze_architecture_ablation.py D:\DEV\Galen-new\evals\runs\architecture-v1\architecture-v1-pctx02-r5

# 汇总按任务卡重采样的置信区间
python D:\DEV\Galen-new\scripts\analyze_architecture_pilot.py
```
