# Galen 文档系统与合并深化规范 v1.0

更新时间：2026-09-12  
适用范围：Galen Research Workbench / Galen Rehab Intelligence

## 1. 结论

Galen 目前不是“没有文档”，而是文档已经覆盖了产品、架构、数据、评测、计划书和交接，但这些内容没有被一个事实源统一管理。继续分别维护计划书、技术报告和预印本，会产生三个问题：

1. 同一个概念在不同文件中有不同名字；
2. 已实现、演示、试验和规划内容混在一起；
3. 旧版本的数字、截图和叙事仍可能被误当作当前结论。

因此采用以下结构：

> **一份事实主文档（Single Source of Truth） + 四种读者视图 + 一个证据归档层。**

不是把所有文件机械拼成一本大册子。主文档负责回答“我们是什么、怎么工作、现在做到哪一步”；计划书、白皮书、技术佐证和交接手册分别从主文档生成或引用内容。

## 2. 文档分层

| 层级 | 读者 | 唯一职责 | 当前权威文件/目录 |
|---|---|---|---|
| L0 事实主文档 | 核心团队、PI、后续 AI | 统一定义、范围、状态和术语 | `docs/Galen-Research-Workbench-Master-v1.md` |
| L1 项目叙事 | 评委、合作方、非技术读者 | 说明问题、价值、产品闭环和落地路径 | `docs/proposal/`；由 L0 生成 |
| L1 技术白皮书 | 技术评审、研究合作者 | 说明数据模型、PI 编排、连接器、证据和评测 | `docs/vision/galen_product_vision.tex` + L0 技术章节 |
| L1 技术佐证 | 评委技术核验、内部验收 | 给出可运行证据、实验协议、结果、日志和复现路径 | `docs/proposal/galen-rehab-intelligence-technical-evidence-v1.md`、`output/pdf/` |
| L1 操作/交接 | 队友、开发者、接手 AI | 说明如何运行、测试、录制和继续开发 | `docs/DEVELOPER_ONBOARDING.md`、`docs/GALEN_AI_HANDOFF_2026-08-29.md` |
| L2 证据归档 | 复核者 | 保存原始数据、截图、运行日志、PDF、哈希和旧版本 | `evals/`、`output/`、`docs/proposal/tmp_render*` |

## 3. 当前文档盘点

### 3.1 已有且应保留为主干的内容

- `docs/galen-prd.md`：当前产品基线，定义 ResearchProject、任务、工作台和验收要求。
- `docs/vision/galen_product_vision.tex`：最完整的产品愿景、Connect–Govern–Reason–Deliver 闭环、RehabID 和 PI Agent 叙事。
- `docs/rehab-data-model.md`：多模态康复数据的实体、血缘和会话模型。
- `docs/galen-skills-mcp-architecture.md`：Skill、MCP、Session 的能力分层。
- `docs/galen-evaluation-and-negative-optimization.md`、`docs/context-engineering-eval-framework.md`：评测指标、门禁、消融和负优化规则。
- `docs/proposal/galen-rehab-intelligence-technical-evidence-v1.md`：运动疲劳首发场景、工程证据和 Pilot 设计。
- `docs/preprint-galen-v0.1.md`：可量化上下文工程的论文叙事，适合作为研究附录，不作为产品计划书首页叙事。

### 3.2 应降级为专题或附录的内容

- `docs/dsh-study/`：外部框架学习笔记，保留为技术调研附录。
- `docs/superpowers/plans/` 与 `docs/superpowers/specs/`：实施历史和设计决策，不能代替当前架构定义。
- `docs/handoffs/`、`docs/lessons/`：交接和经验记录，按日期归档。
- `docs/galen-ui-system.md`、`docs/galen-ui-implementation-plan.md`：作为界面实现规范，引用主文档的产品对象，不单独定义产品方向。
- `docs/rehab-product-comparison.md`：竞品/产品分析附录。

### 3.3 应停止继续复制的内容

- `docs/proposal/galen-rehab-intelligence-proposal-v2.pdf`、`v3.pdf`、`v4.pdf` 以及同目录重复命名的 PDF：保留为历史快照，今后只从一个源文件生成新版本。
- `output/pdf/Galen-完整文档-合并升级版*.pdf`：早期“全量合并”产物，保留作历史记录，不再作为当前主文档。
- `output/pdf/galen-technical-evidence-v2.pdf` 至 `v8.pdf`：保留版本链；对外只选择一个经过验收的版本。
- `docs/proposal/tmp_render/`、`tmp_render_v4/`：只作为构建缓存，不在叙事中引用。

## 4. 需要统一的四个事实

### 4.1 产品定位

统一表述为：

> **Galen 是面向康复科研的数据治理与执行工作台：把分散的病例、评估、设备和文献证据组织为可追踪的研究状态，并由 PI Agent 驱动分析、验证与成果交付。**

层级关系固定为：

- 当前产品：康复科研工作台；
- 首发验证场景：运动疲劳的多模态纵向研究；
- 核心数据对象：ResearchProject 内的 RehabID；
- 长期方向：开放的康复科研 Context Layer。

“运动疲劳”是切口，不是把 Galen 限制成疲劳监测软件；“RehabID”是研究状态对象，不是临床诊断系统。

### 4.2 对象层级

```text
ResearchProject
  ├── ResearchQuestion / ResearchDesign
  ├── RehabID（受试者/病例的纵向研究身份）
  │     └── AssessmentSession / Observation / Artifact
  ├── EvidenceItem / SearchRun
  └── ResearchTask / RunLedger
```

ResearchProject 是顶层；RehabID 是项目内的纵向状态锚点；设备、量表、工作台和 Recovery Companion 都是数据连接器或数据来源，不是产品中心。

### 4.3 证据状态

所有文档必须给每个关键陈述加状态标签：

| 标签 | 含义 |
|---|---|
| `Implemented` | 当前代码可运行并有测试或截图 |
| `Demonstrated` | 已用去标识化/构造数据完成端到端演示 |
| `Pilot` | 已有采集协议和评价方案，仍需真实数据验证 |
| `Planned` | 已进入路线图，但尚未交付 |
| `Archived` | 历史版本，不代表当前结论 |

这一步是合并的关键：同一段话不能同时被写成“已经实现”和“计划实现”。

### 4.4 评测结果

当前评测资料中存在 `large-comparison-20260912-final` 和 `large-comparison-20260912-corrected` 两个结果文件。它们必须在对外发布前通过一张“结果冻结表”选出唯一版本，并记录：数据文件、任务卡版本、过滤规则、运行时间、统计脚本和最终 PDF 的哈希。

在冻结完成前，文档只能写：

> “已完成大规模架构先导实验，结果正在进行最终一致性核对。”

不能在不同文档中交替引用 98.7% 和 94.7% 两套数字。

## 5. 合并后的主文档结构

`docs/Galen-Research-Workbench-Master-v1.md` 采用以下顺序：

1. **执行摘要**：一句话定位、当前切口、当前交付状态。
2. **问题定义**：康复科研的数据碎片、上下文断裂和交付断裂。
3. **产品闭环**：Connect → Govern → Reason → Deliver。
4. **用户与首发场景**：研究 PI/康复师/研究助理；运动疲劳三时间点闭环。
5. **系统架构**：PI Agent、Project Context、RehabID、Connector、Evidence、Artifact。
6. **数据治理**：统一数据协议、清洗、质量标记、时间对齐、来源和版本。
7. **证据与检索**：SearchRun、数据库覆盖、论文级链接、Evidence Graph。
8. **执行与交付**：任务契约、工具调用、代码/统计、图表、LaTeX/PDF 产物。
9. **评测方法与结果**：任务卡、基线、消融、成功率、工具经济性、时延和失败边界。
10. **真实产品证据**：工作台、连接器、RehabID、Galen 产物和视频索引。
11. **边界与当前缺口**：哪些已实现，哪些仍需 Pilot 或外部连接器验证。
12. **路线图与验收门槛**：下一版只围绕数据治理、PI 执行和可核验交付推进。
13. **证据索引与术语表**：每个关键主张对应代码、测试、日志、截图或 PDF。

## 6. 四个对外输出如何从主文档派生

### 6.1 计划书

只保留评委需要的内容：问题、洞察、产品闭环、首发场景、创新点、验证、落地和团队。技术细节用图和证据编号引用，不把内部 API、工具名和长段 prompt 放进正文。

### 6.2 技术白皮书

展开数据模型、Context、PI 编排、连接器、证据图和评测协议。它解释“为什么这个架构成立”，不承担比赛叙事的全部任务。

### 6.3 技术佐证报告

每一项能力都采用固定格式：

```text
主张 → 操作步骤 → 输入 → 产物 → 可复核证据 → 当前状态
```

截图、运行日志、测试编号、原始 JSONL 和最终 PDF 必须有可定位路径。

### 6.4 操作/交接手册

只写如何运行、如何加载数据、如何启动 MCP、如何执行评测、如何生成产物和如何排错；不再重复产品愿景。

## 7. 深化重点

合并后真正需要补深的不是口号，而是五个可验证层：

1. **数据层**：定义清洗规则、单位、时间戳、缺失/异常/冲突标记和来源血缘。
2. **上下文层**：明确 PI 记忆什么、何时更新项目状态、Session 如何回流主线程。
3. **证据层**：每个科研建议连接到检索结果、原始数据、分析脚本或人工决定。
4. **评测层**：把任务成功率、引用完整性、工具调用经济性和交付质量纳入同一协议。
5. **交付层**：把 Markdown/LaTeX、图表、PDF、预览链接和运行记录视为同一个 Artifact，而不是分散文件。

## 8. 版本治理

- 主文档版本采用 `vMAJOR.MINOR`，事实变化才升版本；措辞修订不另造一套产品名称。
- 每个对外 PDF 的首页写明：主文档版本、代码提交、数据批次、生成日期。
- 旧版不删除，统一移动或标记为 `Archived`；不在 README 或计划书中并列推荐多个“最新版”。
- 任何新截图、新数字、新功能，先进入主文档的证据索引，再进入计划书或白皮书。

## 9. 当前判断

这次合并是必要的，而且现在正是合并的时间点：Galen 的产品主线已经从“聊天助手”收敛为“康复科研的数据治理与执行系统”，PI、Connector、RehabID、证据链和实验评测也已经有了对应实现或证据。下一步不应继续增加同义文档，而应完成主文档、冻结评测结果，并从主文档生成一版新的计划书和技术佐证报告。
