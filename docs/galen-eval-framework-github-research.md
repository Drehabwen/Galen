# Galen 评测框架 GitHub 调研与技术选型

调研日期：2026-08-22

## 结论

Galen 不直接嵌入一个完整的第三方评测平台，而是在现有 Rust `driver`、`ToolTrace`、`ChatRunSummary` 和追加式 Session 之上实现轻量评测内核。这样能直接观察桌面端真实链路，并避免 Python sidecar、Docker 服务或云端控制面的额外故障点。

采用的设计组合：

- 借鉴 Inspect AI 的 `Task → Solver → Scorer` 分层。
- 借鉴 METR Task Standard 的隔离任务环境、隐藏评分数据和环境结束评分。
- 借鉴 AgentEvals 的工具轨迹匹配，但优先采用确定性轨迹断言。
- 借鉴 Promptfoo 的声明式案例、CLI 与 CI 阈值体验。
- 借鉴 CORE-Bench 的“以真实产物和可复现结果评分科研 Agent”原则。
- 为未来接入 OpenTelemetry 保留 RunLedger 字段，但第一阶段不部署 Phoenix、Langfuse 或 Opik。

## 候选项目比较

| 项目 | 最值得借鉴 | 不直接采用的原因 | Galen 决策 |
| --- | --- | --- | --- |
| [promptfoo/promptfoo](https://github.com/promptfoo/promptfoo) | 声明式测试、Provider 适配、断言、缓存、CLI/CI 报告 | 更擅长 Prompt/响应评测；Galen 还要检查本地工作区、Session、Evidence、节点恢复和 Artifact | 借鉴配置与报告，不作为主 Runner |
| [UKGovernmentBEIS/inspect_ai](https://github.com/UKGovernmentBEIS/inspect_ai) | Task/Solver/Scorer、epochs、日志、沙箱、多轮工具 Agent | Python 运行时和自身 Agent 抽象会与 Galen Rust 循环重叠 | 借鉴核心抽象与多次运行模型 |
| [METR/task-standard](https://github.com/METR/task-standard) | 环境构建、权限、资产、隐藏任务数据、结束/中间评分 | 容器/VM 标准对桌面端本地工作区过重 | 采用 fixture 隔离、原始样本只读、工作副本评分 |
| [langchain-ai/agentevals](https://github.com/langchain-ai/agentevals) | EXACT、IN_ORDER、ANY_ORDER 轨迹比较与 LLM 轨迹 Judge | LangChain 依赖与 Judge 成本不是硬质量门的好基础 | 采用轨迹断言概念，Rust 原生实现 |
| [agentevals-dev/agentevals](https://github.com/agentevals-dev/agentevals) | 直接从 OpenTelemetry Trace 评分，执行与评分解耦 | 项目较新；Galen 当前还没有统一 OTEL 轨迹 | RunLedger 稳定后再评估 OTEL 导出 |
| [langwatch/scenario](https://github.com/langwatch/scenario) | 多轮用户模拟、脚本化中途断言、缓存 | 模拟用户与 Judge 会引入第二个随机系统 | 第二阶段只用于多轮交互压力测试 |
| [siegelz/core-bench](https://github.com/siegelz/core-bench) | 科研复现任务、隔离环境、对最终计算结果评分 | 数据集主要评测论文代码复现，不覆盖 Galen 产品状态 | 借鉴 Artifact 与可复现性评分；未来选择医学子集 |
| [Arize-ai/phoenix](https://github.com/Arize-ai/phoenix) | OpenInference/OTEL 追踪、数据集、实验和可视化 | 完整平台较重，且许可证不是 MIT/Apache 类宽松许可 | 不嵌入；仅借鉴 Trace/Span 字段 |
| [langfuse/langfuse](https://github.com/langfuse/langfuse) | Session Trace、数据集、实验、人工标注 | 需要独立服务与控制面 | 团队规模扩大后作为可选观测后端 |
| [comet-ml/opik](https://github.com/comet-ml/opik) | Trace、离线评测、Agent 工作流观测 | 对当前单机 Galen 属于过度部署 | 暂不采用 |
| [confident-ai/deepeval](https://github.com/confident-ai/deepeval) | Pytest 风格与大量 LLM/RAG 指标 | 偏 Python 与 LLM-as-Judge，难以验证本地状态恢复 | Rubric 阶段参考，不承担硬门 |
| [openai/evals](https://github.com/openai/evals) | Eval registry、私有数据集和可扩展 scorer | 更偏模型输出评测，且引入独立 Python 栈 | 不作为 Galen 主评测入口 |

## Galen 的复用边界

### 原生实现

- TOML CaseSpec 与 schema 版本。
- fixture 到临时工作区的安全复制。
- 调用真实 `run_chat`，保留 Galen 自己的模型、工具、Session 和 Artifact 路径。
- TTFT、TTFR、总耗时、请求数、Token、压缩次数和工具轨迹。
- 硬断言、重复调用检测、Artifact 存在/非空/可预览检查。
- JSONL RunLedger 和 baseline/candidate 比较。

### 延后接入

- OpenTelemetry/OpenInference 导出。
- LLM-as-Judge 研究 Rubric。
- 多轮模拟用户。
- Web 仪表盘和团队标注队列。

### 明确不做

- 不让 Judge 模型覆盖事实、文件、路径、引用和恢复等确定性失败。
- 不将“输出更像参考答案”作为 Agent 成功的唯一指标。
- 不把工作区原件直接交给评测 Runner 修改。
- 不在同一次变更中调整产品实现和放宽验收阈值。

## 第一阶段验收

第一阶段完成的标志不是生成一个漂亮 Dashboard，而是以下命令能够稳定工作：

```powershell
cargo run -p galen --bin eval -- validate
cargo run -p galen --bin eval -- run --case E01 --repeat 5
cargo run -p galen --bin eval -- compare --baseline <baseline.jsonl> --candidate <candidate.jsonl>
```

每次失败都必须能够定位到案例、提交、模型配置、工作区副本、断言和原始运行记录。
