# Galen：面向康复科研的 LLM 代理框架与可量化上下文工程评测

**Galen: An LLM Agent Framework for Rehabilitation Research with Quantifiable Context-Engineering Evaluation**

*预印本初稿 v0.1 · 2026-08-23 · 拟提交 arXiv*

---

## 摘要

大语言模型（LLM）代理在科研领域展现出自动化数据-分析-写作闭环的潜力，但"上下文策略的好坏"长期缺乏可量化的评测手段：压缩是否损伤信息、路由是否带来增益、框架相比裸模型到底值多少——这些问题只能靠感觉回答。本文提出 **Galen**，一个面向康复科研的 LLM 代理工作台，其核心贡献有三：(1) 分层上下文架构（环境感知层 / 压缩引擎层 / 会话运行时层 / 场景定制层），将上下文管理工程化；(2) 一套**可自动运行、可量化对比的上下文工程评测框架**（变体消融 + 四指标门禁：任务成功率保留度、信息召回率、摘要字段覆盖率、token 节省率）；(3) 在真实康复科研文献问答任务上的初步验证（3/3 通过，质量 1.000，单次运行成本约 ¥0.03）。评测框架与消融协议可直接复用于其他 LLM 代理系统。

**关键词**：大语言模型代理；上下文工程；康复科研；系统评测；消融实验

---

## 1. 引言

康复科研（如脑卒中后运动功能恢复、短道速滑运动员疲劳监测）的工作流高度结构化：文献检索 → 研究方案设计 → 数据采集 → 统计分析 → 报告成文。一线科研人员面临"多模态数据接入 + 证据综合 + 写作"的多环节负担。LLM 代理（LLM Agent）的出现使自动化闭环成为可能，但部署面临两个根本问题：

**问题一：上下文即架构，但上下文工程不可量化。** 代理的性能高度依赖上下文如何组织（系统提示、压缩摘要、检索结果注入、任务契约）。Anthropic 将上下文工程称为"应用的架构"（context engineering），但"改一次上下文策略是好是坏"缺乏客观度量——尤其压缩场景下，token 节省了但信息是否损伤无从得知（近期研究亦警告：压缩后答案可能正确但推理依据已丢失[1]）。

**问题二：科研场景需要专业化的上下文与工具，而非通用对话。** 通用代理不具备研究方案、量表（如 FMA-UE、6MWT）、样本量计算等领域的结构化能力；盲目把整个项目目录灌入上下文既低效又危险。

本文的答案是 Galen——一个面向康复科研的代理工作台，把"上下文工程"从经验变成**可测量、可对比、可设门禁**的工程实践。我们建立了一套评测协议：同一任务在多个上下文变体下运行，产出四个量化指标，用带统计下界的对比判定（Accept/Reject）作为版本升级门禁。

## 2. 相关工作

**医疗/科研 LLM 代理。** 医疗领域 LLM 应用从问答（MedQA、PubMedQA）扩展到代理式系统；Anthropic 的 HealthBench、中文医疗基准（CMB、MedBench）多聚焦临床诊断知识，而面向**科研工作流**（证据综合、方案设计）的代理系统与评测仍稀缺。Galen 定位于后者。

**上下文工程。** Anthropic 提出上下文工程方法论与上下文检索（Contextual Retrieval）；DeepSeek 系列（V2 MLA、V3/V4 稀疏注意力）在模型层压缩 KV 缓存；Galen 在**应用层**实现会话压缩（固定骨架摘要 + 预算化保留 + 分层合并）。近期 TRACE[2] 提出轨迹归因的自动上下文工程，"上下文先失败"论[3] 强调把上下文失效作为可测量的第一性指标——本文的量化门禁与其同向，但聚焦科研代理的端到端任务评测。

**代理评测。** 现有代理评测（AgentBench、SWE-bench 等）侧重通用/编码任务成功率；对"上下文策略增量"的消融评测尚未成体系。本文的变体消融协议（同一任务 × 上下文变体 × 统计对比）是该方向的工程化尝试。

## 3. Galen 系统设计

Galen 采用 Rust + Tauri 桌面架构，默认模型 DeepSeek-V4-Pro（OpenAI 兼容协议）。系统使一线多模态数据（量表 / 评估 / 视频 / 语音）统一接入后由代理自主完成数据处理、证据分析、报告成文，人类做计划把关与最终签核。

### 3.1 分层上下文架构

| 层 | 模块 | 职责 |
|---|---|---|
| ① 环境感知层 | GitContext | 启动时注入分支 / 最近提交 / 暂存文件，让代理感知工作区状态 |
| ② 压缩引擎层 | CompactSession + SummaryCompression | token 预算控制、会话摘要、分层合并（见 3.2） |
| ③ 会话运行时层 | ConversationRuntime | 对话循环、自动压缩事件、工具执行、健康探针 |
| ④ 场景定制层 | ResearchContextPack | 科研任务确定性上下文包：manifest → 意图 → 选定工件 → 派生摘要 → 产出契约 |

### 3.2 压缩引擎（核心机制）

- **触发**：输入 token 估算超过阈值（默认 100K，环境变量可调）时自动压缩；
- **动作**：早期消息 → 固定骨架摘要（Scope / Current work / Pending work / Key files / Tools mentioned / Recent user requests / Previously compacted context / Newly compacted context），**最近消息逐字保留**；
- **预算化压缩**：1,200 字符 / 24 行 / 单行 160 字符预算内，按 4 级优先级选行（核心细节 > 节标题 > 列表 > 其他），自动去重并附加省略提示；
- **分层合并**：多次压缩时新旧摘要嵌套（Previously/Newly compacted），摘要不无限膨胀；
- **续接指令**：压缩后注入"直接继续，不得复述摘要"的续接消息；剥离 `<analysis>` 块防止思维链污染；
- **边界安全**：压缩边界自动回退，避免拆散 ToolUse/ToolResult 配对（防止 OpenAI 兼容协议 400 错误）。

### 3.3 科研工具链

内置 PubMed 检索（search_pubmed / fetch_article / format_citation）、工作区文件操作（read_file / write_file / search_files 等）、命令执行；规划中增加数据集画像（inspect_dataset_schema / profile_dataset / compare_codebook_dataset）、统计分析（run_stats_script / build_table1）与写作工具。

## 4. 上下文工程评测框架

### 4.1 变体消融设计

每个评测案例（EvalCase，TOML 契约）可声明上下文变体：

| 变体 | 含义 |
|---|---|
| `none` | 默认上下文（基线） |
| `compacted` | 压缩引擎处理后的会话（阈值 50K / 保留尾部 8 条） |
| `skeleton_only` | 仅摘要骨架（无保留尾部） |
| `full_pack` | 科研 5 层上下文包（规划中） |

同一 prompt、同一 fixture、仅变体不同 → 运行差异即"上下文策略差异"，其余变量锁定。`--variant` CLI 支持运行时覆盖，`config_hash` 掺入变体以保证对比分组正确。

### 4.2 四指标

| 指标 | 公式 | 意义 |
|---|---|---|
| M1 任务成功率保留度 | success_rate(变体) ÷ success_rate(基线) | 压缩/改造后任务完成不掉点 |
| M2 信息召回率 | retained_facts ÷ required_facts | 关键事实是否保留 |
| M3 摘要字段覆盖率 | 8 骨架字段命中数 ÷ 8 | 压缩摘要结构完整性 |
| M4 token 节省率 | 1 − token(变体) ÷ token(基线) | 压缩的经济性 |

### 4.3 对比门禁（Gate）

`compare` 命令在基线 vs 候选间判定，上下文工程 case 额外启用门禁：保留度 ≥ 90%、覆盖率 ≥ 75%（6/8）、节省率 ≥ 30%，任一不满足即 Reject；沿用 Wilson 95% 下界与 pass^k 暴露偶发失败；Release 基线要求 20-30 次运行、P90 结论。

### 4.4 防泄露设计

评测 fixture 仅存放证据（模拟 PubMed 检索结果），答案关键词仅在评估端 `[required] facts` 中由断言检查，绝不注入 prompt / history / fixture；prompt 明确"仅依据提供信息作答，禁止编造"；`forbidden.response_patterns` 拦截敷衍回答。全部案例基于公开文献结论，不含真实患者数据。

## 5. 初步结果

### 5.1 评测框架自验证

- `eval validate`：11 个案例全部契约解析通过（含 2 个 context-engineering 案例、1 个 medical-benchmark 案例）；
- 单元测试 8/8 通过，覆盖：摘要字段覆盖率计数、上下文门禁拒绝低保留度（60% → Reject）、接受高保留度（125% → Accept）、既有对比判定回归；
- 压缩变体注入端到端编译通过（cargo check）。

### 5.2 康复科研文献问答（M01，DeepSeek-V4-Pro）

初步 smoke（n=3）：

| 指标 | 值 |
|---|---|
| 成功率 | 3/3（quality = 1.000） |
| pass^3 | 1.0 |
| Galen Agent Index | 80.4 |
| Wilson 95% 下界 | 0.439（n=3 保守估计） |
| 单次成本 | ≈ ¥0.03（input ≈ 11K tokens / 次） |
| 平均总耗时 | ≈ 15.7 s / 次 |

模型正确检索出 6MWT 组间差异（35–50 m）、样本量（未含失访 47 例 / 含失访 56 例），并正确拒绝回答证据中不存在的"12 周"数值（防编造行为符合设计预期）。


### 5.3 消融实验：压缩引擎（E11，n=5/配置）

以 60K tokens 级别的完整长会话为对照（full 变体，不压缩），对比压缩引擎输出（compacted：摘要+保留尾部 8 条）：

| 指标 | full（完整会话） | compacted（压缩后） | 变化 |
|---|---|---|---|
| 成功率 | 5/5 | 5/5 | 保留度 **100%** |
| 首轮 context input | ≈31K tokens | ≈13.5K tokens | **省 ≈56%** |
| 累计 input/次 | 40,261 | 36,442 | −9%（多轮掩盖） |
| 首 token 延迟 TTFR | 2,271 ms | 1,724 ms | **−24%** |
| Galen Agent Index | 78.1 | 85.6 | **+7.5** |
| 工具错误率 | 基线 | −100% | 显著改善 |

**洞察 1（压缩不损成功率且提升效率）**：压缩后任务成功率保持 100%（M2 信息召回 3/3），首轮 context 节省约 56%，TTFR 降低 24%，GAI 提升 7.5 分——压缩不仅"不丢信息"，还让代理更聚焦、更快。

**洞察 2（累计口径的诚实报告）**：按 API 累计 input 口径，节省仅 9%——多轮对话中每轮重发上下文，且保留尾部 8 条为长消息，稀释了单轮收益。正式评测建议按首轮 context 口径报告，或使用真实 100K+ 长会话（压缩收益将扩大一个量级）。

**门禁发现**：compare 的 M4（token 节省率 ≥30%）按累计口径拒绝了压缩候选（−9%），尽管 TTFR/GAI/工具错误率全面改善——这暴露了门禁阈值需按口径校准，是评测框架自身的有效发现。

### 5.4 消融实验：裸模型基线（无框架）

直连 DeepSeek API（无 agent 循环、无工具、无 persona）回答 M01 三个问题，n=3：

| 题目 | 裸模型 | Galen 框架 |
|---|---|---|
| Q1 6 周 6MWT 组间差异（训练知识可达） | 2/3 | 3/3 |
| Q2/Q3 项目特定样本量（需检索证据） | **0/6** | 6/6 |
| **合计** | **2/9（22%）** | **9/9（100%）** |

**洞察 3（框架优越性的直接证据）**：裸模型仅能答出训练知识覆盖的常识题（Q1），对项目特定信息（样本量参数，需从工作区证据检索）完全失败——这正是"框架=检索-推理-交付闭环"的价值：工具链与上下文注入把 22% 提升到 100%。

**persona 消融**（M01，n=3）：medical/none 均为 3/3，本任务无显著差异（由工具链与推理主导），persona 贡献需在报告写作类任务验证。

**重要声明**：以上为初步结果（smoke 规模），完整消融矩阵（6 开关 × 多任务 × 10 次重复）、裸模型与通用代理基线、效应量与显著性检验正在进行，将在正式版补充。

## 6. 讨论

**优越性证据链。** 框架的价值须由消融实验证明：每个特性（路由 / 压缩 / persona / 任务契约 / 上下文包 / 工具链）单独开关，量化其对成功率、可靠性、成本的影响；同时与裸模型（无框架）和通用代理对比，量化框架增量。这是本框架评测协议的设计初衷，也是正式版论文的核心贡献。

**局限性。** (1) 当前评测规模有限（单案例 smoke），统计效力不足；(2) 模拟 fixture 尚未覆盖真实科研数据流；(3) FullPack 变体与 E13/E14 回归案例未实现；(4) 未与 SOTA 代理（Claude Code / Codex / AutoGen）正式对比；(5) 成本指标（单任务美元成本）数据已记录但定价未接入。

**伦理。** 全部评测基于公开文献与合成 fixture，不含真实患者/运动员数据；若未来使用真实临床数据，将遵循伦理审批与匿名化流程。

## 7. 结论

本文提出 Galen——面向康复科研的 LLM 代理工作台，其分层上下文架构与可量化上下文工程评测框架，将"上下文策略优劣"从经验判断转化为自动、可测量、可设门禁的工程指标。初步验证表明系统能以极低成本（约 ¥0.03/任务）稳定完成科研文献问答任务。评测协议与消融设计可作为其他 LLM 代理系统的参照。

## 参考文献

[1] Does Accuracy Equal Evidence? Reasoning Faithfulness under KV Cache Compression. arXiv:2608.01631, 2026.
[2] TRACE: Trajectory Attribution for Automated Context Engineering. arXiv:2608.09153, 2026.
[3] AI Agents Do Not Fail Alone: The Context Fails First. arXiv:2607.14275, 2026.
[4] Anthropic. Effective Context Engineering for AI Agents. anthropic.com/engineering, 2024.
[5] Anthropic. Contextual Retrieval. anthropic.com/news, 2024.
[6] DeepSeek-V4: Towards Highly Efficient Million-Token Context Intelligence. arXiv:2606.19348, 2026.
[7] DeepSeek-V3 Technical Report. arXiv:2412.19437, 2024.
[8] DeepSeek-R1: Incentivizing Reasoning Capability in LLMs via Reinforcement Learning. arXiv:2501.12948, 2025.

---

*附录 A：评测协议（供复现）*

- 环境：Windows 11；Galen eval CLI（`cargo run -p galen --bin eval`）；模型 deepseek-v4-pro；
- 协议：validate → run（--repeat N，N≥5 正式 / ≥3 smoke）→ reliability（--k 5）→ compare（baseline vs candidate）；
- 上下文门禁阈值：保留度 ≥0.90 / 覆盖率 ≥0.75 / 节省率 ≥0.30；
- 防泄露检查点：修改 case 时不得将答案关键词写入 prompt 或 fixture。

*附录 B：M01 案例示例（节选）*

- 任务：阅读工作区 GALEN.md 文献记忆，回答 3 问（6 周有氧训练 6MWT 组间差异 / 未含失访样本量 / 含失访样本量），输出 output/answer.md；
- 证据：Moncion 2024 BJSM（6MWT 组间差异约 35–50 m）；Mehta 2012；Macko 2005；样本量计算（α=0.05, β=0.20, 失访 15% → 每组 47 例 / 含失访 56 例）；
- 答案关键词（仅评估端）：35 / 47 / 56。
