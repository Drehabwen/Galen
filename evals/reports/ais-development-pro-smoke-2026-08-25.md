# Galen AIS 开发集 DeepSeek V4 Pro Smoke

> 由 Galen Eval 自动生成。原始 JSON/JSONL 是事实源，本报告用于审阅与演示。

## Agent 端到端可靠性

| 指标 | 结果 |
|---|---:|
| 运行数 | 24 |
| 硬门通过 | 20/24 |
| 成功率 | 83.3% |
| Wilson 95% 下界 | 64.1% |
| pass^5 | N/A |
| Galen Agent Index | 62.4 |
| 总耗时 P50 / P95 | 19211 / 33872 ms |
| TTFR P50 / P95 | 1419 / 1966 ms |
| Token 平均值 | 12601 |
| 证据检索覆盖率 | N/A |
| 证据引用覆盖率 | N/A |
| 禁止证据命中 | 0 |
| 本地 / 外部检索调用 | 0 / 0 |

### 分案例结果

| Case | Model | 通过 | 成功率 | Lower95 | 质量 | 引用覆盖 |
|---|---|---:|---:|---:|---:|---:|
| AIS-C021-T1 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C021-T2 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C021-T3 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C021-T4 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C022-T1 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C022-T2 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C022-T3 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C022-T4 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C023-T1 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C023-T2 | deepseek-v4-pro | 0/1 | 0.0% | 0.0% | 0.938 | N/A |
| AIS-C023-T3 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C023-T4 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C024-T1 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C024-T2 | deepseek-v4-pro | 0/1 | 0.0% | 0.0% | 0.789 | N/A |
| AIS-C024-T3 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C024-T4 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C025-T1 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C025-T2 | deepseek-v4-pro | 0/1 | 0.0% | 0.0% | 0.923 | N/A |
| AIS-C025-T3 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C025-T4 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C026-T1 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C026-T2 | deepseek-v4-pro | 0/1 | 0.0% | 0.0% | 0.882 | N/A |
| AIS-C026-T3 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |
| AIS-C026-T4 | deepseek-v4-pro | 1/1 | 100.0% | 20.7% | 1.000 | N/A |

### 硬门失败

- `AIS-C023-T2` run 1 — `required_fact:科研边界：单病例不能证明因果疗效`：missing
- `AIS-C024-T2` run 1 — `required_fact:C024-B-T=27 deg`：missing
- `AIS-C024-T2` run 1 — `required_fact:C024-B-TL=30 deg`：missing
- `AIS-C024-T2` run 1 — `required_fact:C024-B-ATR-T=11 deg`：missing
- `AIS-C024-T2` run 1 — `required_fact:C024-B-ATR-TL=8 deg`：missing
- `AIS-C025-T2` run 1 — `required_fact:C025-B-T=45 deg`：missing
- `AIS-C026-T2` run 1 — `required_fact:C026-B-T=40 deg`：missing
- `AIS-C026-T2` run 1 — `required_fact:C026-B-L=22 deg`：missing

## 人工语义复核

本节不修改原始 RunLedger，也不覆盖自动分数。它用于区分模型失败与评分器失败。

| Case | 自动结果 | 复核结果 | 说明 |
|---|---|---|---|
| AIS-C023-T2 | Fail | Pass | 已表达“单病例不能证明因果疗效”，仅缺少评分器要求的固定前缀。 |
| AIS-C024-T2 | Fail | Pass | 四项数值均正确，但写成 `观察ID=C024-B-T 27 deg`，未命中脆弱的精确字符串。 |
| AIS-C025-T2 | Fail | **Fail** | 数值内容正确，但自行加入输入中不存在的“40–50°手术阈值”，违反唯一病例来源契约。该陈述不在本报告中判定医学真伪，只判定为未获当前输入支持。 |
| AIS-C026-T2 | Fail | Pass | 两项数值均正确，但等号位置与硬门模板不同。 |

复核后的实质通过率为 **23/24（95.8%）**。自动通过率仍保持
**20/24（83.3%）**，因为原始测评结果必须不可变。

## 首轮启发

1. `T1` 基线抽取、`T3` 纵向总结和 `T4` 科研边界均为 6/6；主要薄弱点集中在 `T2` 时间截断协议。
2. 当前 `required.facts` 使用逐字字符串匹配，会把格式差异误判为事实缺失。观察值应改为结构化 scorer，比较观察 ID、数值、单位与容差。
3. “未调用外部检索工具”不等于“没有使用输入外知识”。病例 25 说明还需要 unsupported-claim/unsupported-number scorer。
4. 所有 24 次运行均产生非空、可预览 Markdown；UTF-8 字节检查未发现替换字符。终端中曾出现的乱码来自显示编码，不是 Artifact 损坏。
5. 本轮每个 case 仅运行一次，只能作为 smoke，不能作为发布可靠性基线。下一阶段应先修正 scorer，再对失败模式和代表性成功任务重复至少 5 次。
