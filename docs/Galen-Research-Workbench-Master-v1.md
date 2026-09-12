# Galen Research Workbench

## 面向康复科研的数据治理与执行工作台

版本：v1.0  
日期：2026-09-12  
文档状态：事实主文档（Single Source of Truth）

> 本文档统一产品定义、技术架构、数据模型、评测口径和当前交付状态。计划书、技术白皮书、技术佐证和交接手册均应引用本文档，而不是各自重新定义 Galen。

## 1. 执行摘要

康复科研中的核心问题不是缺少某一个模型，而是研究对象、评估设备、量表、随访记录、文献和分析产物彼此断裂。研究者可以采集很多数据，却难以把这些数据组织成可比较、可追踪、可复现、可以继续更新的研究证据。

**Galen 是面向康复科研的数据治理与执行工作台。**它把分散的数据接入同一个 ResearchProject，在项目内用 RehabID 维护纵向状态，由 PI Agent 负责研究任务编排，再将清洗后的数据、分析结果、证据链接、图表和 PDF 交付物放回同一条研究链路。

当前首发验证场景是**运动疲劳的多模态纵向研究**：基线、负荷后和恢复期的数据通过同一个 RehabID 对齐，系统识别状态变化、数据质量和证据来源，并驱动后续分析与论文级产物生成。运动疲劳是验证系统价值的切口，不是产品的最终边界。

当前状态概览：

| 能力 | 状态 | 证据入口 |
|---|---|---|
| ResearchProject / PI 任务编排 | `Implemented` | `rust/crates/galen/src-tauri/src/research_task.rs`、`runtime_manager.rs` |
| RehabID 与连接器数据回流 | `Demonstrated` | `rust/crates/galen/src/domain/connectors.ts`、`output/real-galen-paper-run-v2/` |
| 研究任务、工具调用和产物预览 | `Implemented` | `rust/crates/galen/src/components/ResearchExecutionThread.tsx`、`ArtifactMarkdown.tsx` |
| 多模态数据模型 | `Implemented / Pilot` | `docs/rehab-data-model.md`、`docs/proposal/galen-rehab-intelligence-technical-evidence-v1.md` |
| 文献检索、覆盖记录和论文级链接 | `Implemented / Iterating` | `docs/handoffs/2026-08-31-search-run-coverage-ledger-handoff.md` |
| 架构对比与上下文评测 | `Demonstrated` | `evals/`、`output/pdf/galen-framework-comparison/` |
| 真实运动疲劳数据验证 | `Pilot` | `docs/protocol/s01_study_protocol.md`、`evals/case-datasets/sports-fatigue-ath001-v1/` |

## 2. 问题定义

### 2.1 数据断裂

康复科研数据同时存在于工作台表单、量表、力台/动作设备、HRV/可穿戴、视频、语音、随访和文献数据库中。它们的格式、时间粒度和命名方式不同，导致一次研究分析必须反复手工搬运和解释。

### 2.2 研究上下文断裂

研究问题、纳入标准、排除方向、已做决定和证据覆盖情况如果只存在于聊天记录中，就会出现“该忘的没有忘、该记的没有记”。Galen 将这些内容写入 Project Context、ResearchTask 和 RunLedger，而不是依赖某一轮对话的偶然记忆。

### 2.3 交付断裂

研究者不只是需要一段回答，还需要数据表、分析脚本、图表、引用、方法说明和可打开的 PDF。Galen 把这些内容作为可追溯 Artifact 交付，产物能够回指输入数据、执行步骤和证据来源。

## 3. 产品闭环：Connect → Govern → Reason → Deliver

### Connect：连接已有工作，而不是替换所有软件

康复师工作台、量表、力台、Dynamo、可穿戴、视频和文献数据库都可以作为连接器。Galen 不要求研究者离开原来的专业软件；连接器负责把经过授权、去标识化的数据纳入 ResearchProject。

### Govern：把数据变成研究资产

Galen 对接入数据执行模式识别、字段映射、单位和时间戳标准化、重复检查、缺失标记、异常标记、冲突记录和来源登记。清洗不会覆盖原始记录，所有派生字段都带有处理版本和来源。

### Reason：由 PI Agent 组织研究执行

PI Agent 不是普通聊天助手。它维护研究问题和项目状态，把目标编译为 ResearchTask，按需调用连接器、检索、代码执行、统计、绘图和文档工具，并把 Session 结果以结构化摘要回流主线程。

### Deliver：给出可验证的科研产物

交付物可以是数据质量报告、队列表、统计结果、图表、研究方案、带论文级链接的证据综述或 LaTeX/PDF 论文。每个关键结论都要能追溯到数据、脚本、文献或人工决定。

## 4. 核心对象和数据关系

```text
ResearchProject
  ├── ResearchQuestion / ResearchDesign
  ├── RehabID
  │     └── AssessmentSession
  │           ├── ScaleRecord
  │           ├── MeasureRecord
  │           ├── Video/Audio Asset
  │           └── QualityFlag / Provenance
  ├── EvidenceItem / SearchRun
  ├── ResearchTask / Session
  ├── AnalysisRun / Figure / Table
  └── Artifact / RunLedger
```

其中：

- **ResearchProject** 是研究和任务的顶层边界；
- **RehabID** 是一个受试者/病例在研究项目内的纵向状态锚点；
- **Connector** 是外部系统到统一数据协议的适配层；
- **EvidenceItem** 是文献、原始数据、分析结果或人工决定的可引用节点；
- **Artifact** 是可交付的文件或预览对象；
- **RunLedger** 保存一次任务的输入、工具调用、输出和错误。

## 5. 首发场景：运动疲劳多模态纵向研究

### 5.1 研究流程

1. 连接康复师工作台或实验采集系统；
2. 为受试者创建或匹配 RehabID；
3. 导入基线、负荷后、恢复期的 HRV、心率、CMJ/力台、RPE、疼痛和训练记录；
4. 执行数据质量检查与时间对齐；
5. 计算个体基线、变化量和恢复比例；
6. 由 PI Agent 生成研究问题、分析任务和证据链；
7. 输出图表、结果段落、参考文献和 PDF 产物。

### 5.2 当前数据边界

当前演示和评测使用去标识化或构造数据；真实运动疲劳 Pilot 已有采集方案，但不能把 Pilot 方案写成已经完成的临床/人体研究结果。对外文档必须区分 `Demonstrated` 与 `Pilot`。

## 6. 数据治理与清洗

### 6.1 统一数据契约

每条观测至少包含：`rehab_id`、`session_id`、`observed_at`、`measure`、`value`、`unit`、`source`、`quality`、`transform_version` 和 `provenance`。不同来源的字段先映射到统一语义，再进入分析层。

### 6.2 清洗规则

- 原始值只读保存，派生值单独存放；
- 单位转换必须记录原单位、目标单位和转换公式；
- 同一会话的时间戳统一到项目时区并保留原始时区；
- 重复记录、缺失记录、异常值和来源冲突分别标记，不静默删除；
- 任何统计结果都能回到清洗后的行、原始文件和处理版本。

### 6.3 数据质量输出

Galen 不只返回“数据可用/不可用”，还输出字段覆盖率、时间完整性、单位一致性、异常点数量、来源覆盖和需要补采集的字段，并把质量问题反馈给下一轮研究设计。

## 7. PI Agent、上下文和连接器

Galen 的主对话承担研究方向、关键决定和交付验收；执行 Session 承担检索、数据清洗、统计和绘图。Project Context 保存研究问题、范围、排除方向、数据源、已完成任务和开放问题。这样上下文的连续性不依赖模型是否“记得上一轮聊天”。

连接器的职责是“拿到真实工作中的数据”，而不是把所有应用都改造成 Galen 页面。用户继续在专业工作台中工作，需要研究时从 Connector 获取经过授权的数据，Galen 只接收完成研究所需的最小数据集。

## 8. 证据与检索

文献检索必须记录：数据库、检索时间、检索式、返回数量、去重规则、筛选决定和每条引用的稳定链接。证据覆盖不足时，系统应明确显示“已检索哪些来源、哪些来源未覆盖”，而不是把单一数据库结果写成“没有证据”。

科研建议采用以下链路：

```text
主张 → EvidenceItem → 来源/URL/DOI → 检索运行 → 数据库覆盖 → 生成段落/图表
```

## 9. 评测口径

Galen 的优势不能只用“回答更长”证明。评测至少包含：

- 任务成功率与统计区间；
- 事实/字段覆盖率和引用完整性；
- 工具调用次数、失败重试和交付时延；
- 数据清洗和产物的可复现性；
- 失败边界、人工接管点和版本回归。

当前仓库已形成任务卡、重复运行、配对比较、架构消融和 PDF 报告。对外发布前必须冻结唯一的评测数据批次和统计结果，并把其 JSONL、脚本、摘要和 PDF 互相链接。

## 10. 交付证据

当前可引用的证据资产包括：

- `output/real-galen-paper-run-v2/`：从连接器导入到分析和论文产物的端到端记录；
- `output/pdf/galen-technical-evidence-v8.pdf`：技术佐证报告候选版本；
- `output/pdf/galen-framework-comparison/galen_framework_comparison_report_large_scale_final.pdf`：框架比较实验候选版本；
- `docs/proposal/images/`：工作台、RehabID、恢复指标和 Galen 分析中枢截图；
- `docs/media/galen-promo.mp4`、`output/connector-flow-recording/`：产品流程展示资产。

这些文件是证据，不是产品定义。产品定义以本文档和现行 PRD 为准。

## 11. 当前缺口

1. 冻结并唯一化大规模框架比较实验的统计结果；
2. 将技术佐证报告中的每个主张绑定到具体测试、日志、截图或原始数据；
3. 完成真实目标用户对系统结果的人工复核，形成一致率、评分均值和分歧案例；
4. 补齐代码/公式执行链的可复核截图，并将其放入比赛效果验证包；
5. 统一计划书、技术白皮书和演示视频的术语与版本号；
6. 完成真实运动疲劳 Pilot，届时再把 `Pilot` 升级为 `Validated`；
7. 将文献覆盖、数据质量和 Artifact 交付状态纳入同一个项目状态面板。

## 12. 路线图

### 阶段一：打穿闭环

固定一个 ResearchProject，完成 Connector → RehabID → 清洗 → 分析 → 证据 → PDF 的稳定路径。

### 阶段二：形成研究项目操作系统

把任务恢复、证据覆盖、数据质量、RunLedger 和版本回溯做成默认能力。

### 阶段三：扩大连接器和研究场景

接入更多工作台、力台、Dynamo、可穿戴、量表和文献来源，保持统一数据协议，不改变主线。

## 13. 术语基线

| 术语 | 固定含义 |
|---|---|
| Galen | 康复科研的数据治理与执行工作台 |
| PI Agent | 维护研究项目状态并编排执行任务的主智能体 |
| RehabID | 项目内受试者/病例的纵向研究身份 |
| Connector | 外部软件或设备到统一数据协议的适配层 |
| EvidenceItem | 可追踪、可引用的证据节点 |
| Artifact | 可打开、可验证、可继续使用的研究产物 |
| Research Harness | Galen 的内部执行与恢复机制，不是对外产品名称 |
