# Galen 严格对齐实验总结

日期：2026-09-11  
运行对象：外部 Codex CLI + 隔离 DeepSeek provider  
模型标签：`deepseek/deepseek-v4-flash`  
任务：`evals/cases/paired` 全部 15 张任务卡（PCTX01–05、PDATA01–05、PTOOL01–05）  
重复：每张卡 5 次，共 75 次；fixture 与 Galen 正式实验相同。

## 1. 严格验收口径

每次运行同时检查：

- 进程正常结束；
- 任务卡要求的事实全部保留；
- 必需产物存在、非空且可预览；
- 禁止事实/禁词不出现在回答和产物中；
- 必需工具出现，且工具顺序与任务卡一致；
- 工具调用不超过任务预算，重复调用不超过上限；
- 模型请求数不超过任务预算。

工具调用来自 Codex 的可观察 JSON 事件。`file_change` 归一化为 `write_file`，读写型命令归一化为 `read_file`/`write_file`，`web_search` 归一化为 `search_evidence`；无法观察到的调用不补记。

## 2. 结果

| 任务组 | Galen（候选/当前正式运行） | 外部 Codex + DeepSeek（严格） | Galen 平均工具调用 | 外部平均工具调用 | Galen 中位总时延 | 外部中位总时延 |
|---|---:|---:|---:|---:|---:|---:|
| 上下文 PCTX | 25/25 | 9/25 | 1.20 | 13.40 | 9.1 s | 31.2 s |
| 数据治理 PDATA | 23/25 | 2/25 | 2.20 | 9.28 | 12.3 s | 38.8 s |
| 工具与交付 PTOOL | 24/25 | 6/25 | 2.48 | 12.24 | 9.7 s | 29.8 s |
| **合计** | **72/75（96.0%）** | **17/75（22.7%）** | **1.96** | **11.64** | — | **32.2 s** |

外部严格运行的 Wilson 95% 成功率下界为 14.7%，Pass^5 为 0；外部 58 次未通过硬门。其内容质量均值约 0.869，但这不能抵消工具预算、顺序或范围安全失败。

作为同样 75 次记录的汇总，Galen 合并结果的 Wilson 95% 下界为 88.9%，Pass^5 为 0.8；Galen 仍有 3 次历史功能卡未通过，因此这里没有把结果写成“100% 完美”。

## 3. 观察到的失败模式

1. **上下文修订与范围切换**：外部 agent 能写出看似完整的文档，但容易把旧版本事实带回当前产物，或在探索临时目录时触发范围安全门。
2. **数据治理**：字段规范、缺失/重复识别、时间轴和溯源任务常能给出解释，却没有在限定调用预算内完成规定产物或保留全部约束。
3. **证据检索**：PTOOL02 平均约 32 次工具调用，出现 240 秒超时；“搜到了内容”不等于按任务卡完成可交付、可核验的证据流程。
4. **失败恢复**：PTOOL04 未稳定满足“失败一次—切换备用输入—完成交付”的精确序列。

## 4. 可以支持的结论

在这组固定的康复科研任务上，Galen 的可验证优势是**上下文状态、数据治理和工具编排的可控性**：同一模型标签下，Galen 更少的工具调用带来了更高的硬门通过率和更短的完成时延。优势不是“模型智力更高”的证明，而是 Galen 把研究约束、范围、工具预算和交付状态做成了可执行协议。

## 5. 不能过度外推的部分

- 这不是所有任务、所有模型或所有 provider 的普遍排名；
- Galen 使用内部编排运行，外部组使用 Codex CLI 的隔离 provider，运行时仍不同；
- 外部适配器的工具语义是基于可观察事件的保守归一化，不能替代同一底层 API 的完全同构实验；
- 结果适合支撑“科研工作流控制层”的产品叙事，不足以单独宣称通用 agent 全面优于其他框架。

## 6. 文件

- 原始严格结果：`external-codex-deepseek-flash-strict-v1.jsonl`
- 同口径 Galen 75 次合并结果：`galen-strict-comparable-v1.jsonl`（上下文候选 25 次 + 工具/数据正式运行 50 次）
- Galen 正式报告：`paired-v1/20260911-formal-v1/report.md`
- 适配器与口径：`evals/agent/baselines/external_runner.py`、`evals/agent/baselines/README.md`

## 7. LaTeX 与 Python 复现

- LaTeX 报告源码：`output/pdf/galen-strict-aligned/galen_strict_aligned_report.tex`
- Python 可视化脚本：`scripts/build_strict_aligned_report.py`
- 指标宏文件：`output/pdf/galen-strict-aligned/metrics.tex`
- 图表目录：`output/pdf/galen-strict-aligned/figures/`
- 编译产物：`output/pdf/galen-strict-aligned/galen_strict_aligned_report.pdf`
- 架构论文初稿：`output/pdf/galen-architecture-v0.2/galen_architecture_paper.tex`
- 架构论文 PDF：`output/pdf/galen-architecture-v0.2/galen_architecture_paper.pdf`
- 架构先导统计脚本：`scripts/analyze_architecture_pilot.py`
- 架构先导统计输出：`output/pdf/galen-strict-aligned/architecture_pilot_statistics.json`、`architecture_pilot_statistics.md`

重新生成图表：

```powershell
python scripts/build_strict_aligned_report.py
```

随后在 `output/pdf/galen-strict-aligned/` 中用 XeLaTeX 编译两遍即可更新 PDF。
