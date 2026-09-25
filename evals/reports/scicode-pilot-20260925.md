# Galen-SciCode Pilot v1：首轮代码能力对照

## 结论

本轮是诊断性单次 smoke，不是稳定排名，也不是官方 ScienceAgentBench 分数。

| Agent 产品栈 | 可执行评分 | 隐藏泛化 | 静态安全 | 总耗时 | 原生契约 |
|---|---:|---:|---:|---:|---:|
| Galen + deepseek | 77.5/100 | 1/2 | 通过 | 65.997 s | 未通过 |
| Codex CLI 0.146.0（本机配置模型） | 100/100 | 2/2 | 通过 | 262.543 s | 通过 |

Codex 正确完成公开数据、未见 ID/乱序数据以及重复、缺失、非 verified、零分母等隐藏边界。Galen 的程序将 `verified_rows` 统计放在过滤/去重之后，得到 6，而原始输入中状态为 verified 的行数应为 8。

## Galen 暴露的问题

1. `execute_command` 没有在评测 workspace 中执行，而落在仓库根目录；Agent 被迫搜索临时目录并使用绝对路径切换，制造无效调用和路径泄漏风险。
2. 任务提示中的反引号路径与占位路径被提前当作真实文件读取，首三次工具调用直接失败。
3. 实际使用 17 次模型请求、19 次工具调用，超过任务卡的 12/14 上限；这不是单纯模型答错，而是执行编排失控。
4. Agent 只在公开数据上做断言，并在最终回答中明确承认未测试缺失、非 verified、重复与零分母分支。当前 Galen 缺少“根据需求主动构造最小边界样例”的科研编码策略。
5. `.py` 被原生 evaluator 判为不可预览，导致代码交付即使存在也触发 preview hard gate；代码预览能力与 evaluator 的格式白名单不一致。

## 公平性边界

- 两边执行相同任务卡、公开 fixture 和 evaluator-only 隐藏测试，但底层模型、系统提示、工具协议和网络状态不同，因此这是产品级对照，不是同模型消融。
- 每个产品只有一次有效运行，不能估计方差、pass@k 或置信区间；进入基线前应至少重复 5 次。
- 官方 ScienceAgentBench 含 102 个来自 44 篇论文的任务；本 pilot 只有一个康复数据任务，只验证其“生成自包含 Python 程序并按执行结果评分”的核心范式。
- 官方完整评测包当前需要受控下载，且本机缺少其推荐的 Docker/Python 3.10 环境，因此本报告不声称与官方榜单可横向换算。

## 可审计记录

- Galen 运行：`evals/runs/scicode-galen-20260925-154546.jsonl`
- Galen 隐藏评分：`evals/runs/scicode-galen-20260925-154546-score.json`
- Codex 有效运行：`evals/runs/scicode-codex-20260925-181557.jsonl`
- Codex 隐藏评分：`evals/runs/scicode-codex-20260925-181557-score.json`
- Codex workspace：`evals/runs/scicode-workspaces/codex-20260925-181557/SCICODE01-run-1`

前四次 Codex 适配器尝试因 Windows shim 权限/转发问题以及多行 TOML prompt 未解析而无效；这些记录只用于评测基础设施审计，不计入 Agent 分数。

## 下一轮门禁

先修复 Galen workspace cwd、提示词伪路径预读取、预算停止条件和 Python 预览判定；随后对同一 SCICODE01 各跑 5 次。再扩展到统计分析、文献数据抽取和信号处理三类任务，形成最小可解释的康复科研编码任务簇。
