# Galen Skill 与 MCP 能力架构

版本：v0.1  
状态：实现规划稿  
关联文档：[Galen 现行 PRD](./galen-prd.md)

## 1. 核心判断

Galen 的能力层必须围绕新版产品链路组织：

> 对话确认计划，计划生成画布，画布拆分 Session，Session 执行代码，结果回流主线程。

因此不能把工具简单堆到一个聊天窗口里。正确分层是：

- **Skill**：定义 Galen 会做什么，承担科研方法、计划拆解、统计分析、证据整理和写作能力。
- **MCP / Tool**：定义 Galen 能接什么、能调用什么，承担文件、代码、数据、文献、版本和协作执行能力。
- **Session**：定义一次独立任务如何组合 Skill 与 MCP，承担上下文隔离、执行记录和结构化回流。

一句话：

> Skill 是科研脑子，MCP 是执行手脚，Session 是上下文边界。

## 2. 设计原则

### 2.1 主线程不直接吞全部上下文

主线程只保留：

- 用户目标。
- 计划草案。
- 关键假设。
- 人工确认。
- Session 回流摘要。
- 关键风险和最终产物。

主线程不应默认接收：

- 完整代码。
- 完整日志。
- 完整文献全文。
- 原始数据行。
- 每个 Session 的全部对话。

### 2.2 Session 拥有独立上下文

每个 Session 自带：

- 任务目标。
- 输入。
- 可调用 Skill。
- 可调用 MCP / Tool。
- 运行记录。
- 产物。
- 证据。
- 风险。
- 回流摘要。

### 2.3 能力按科研流程组合

Galen 不应暴露“工具大全”。用户看到的是 Session 节点：

- 文献证据 Session。
- 队列构建 Session。
- 数据清洗 Session。
- 统计分析 Session。
- 图表生成 Session。
- 论文写作 Session。

每个 Session 内部再选择对应 Skill 与 MCP。

## 3. P0 Skill 清单

### 3.1 `research-plan`

作用：把主线程对话转成可确认的研究计划。

输入：

- 用户自然语言目标。
- 课题背景。
- 数据范围。
- 研究约束。

输出：

- 研究问题。
- 关键假设。
- 研究类型。
- Session 节点草案。
- 待确认问题。

典型触发：

- “帮我设计一个回顾性队列研究。”
- “评估某药物对某类患者结局的影响。”
- “把这个课题拆成可执行步骤。”

### 3.2 `session-orchestration`

作用：把已确认计划拆成可执行 Session，并维护依赖关系。

输入：

- 已确认研究计划。
- 用户确认记录。
- 项目文件清单。

输出：

- Session 节点列表。
- 节点依赖边。
- 节点输入输出契约。
- 并行执行建议。
- 回流字段定义。

核心职责：

- 避免上下文爆炸。
- 定义每个 Session 的上下文边界。
- 决定哪些节点必须人工批准。
- 决定哪些节点可以自动执行。

### 3.3 `clinical-research-design`

作用：提供医学科研设计能力。

覆盖：

- 回顾性队列研究。
- 病例对照研究。
- 横断面研究。
- 真实世界研究。
- 纳入排除标准。
- 暴露与对照定义。
- 主要结局和次要结局。
- 偏倚与混杂控制。

输出：

- 研究设计建议。
- 方案草案。
- 风险提示。
- 需要确认的临床边界。

### 3.4 `literature-evidence`

作用：检索、整理和评价医学证据。

覆盖：

- PubMed 检索式生成。
- 指南和共识摘要。
- 关键文献摘要。
- 证据等级提示。
- 引用格式化。

输出：

- 文献证据摘要。
- 支撑声明。
- 引用列表。
- 与 Session 节点关联的证据片段。

### 3.5 `cohort-definition`

作用：定义研究队列。

覆盖：

- 数据字典理解。
- 研究对象筛选。
- 纳入排除标准。
- 暴露组和对照组定义。
- 变量映射。
- 纳排流程图草案。

输出：

- 队列定义表。
- 纳排流程。
- 队列构建代码计划。
- 样本量摘要。
- 风险提示。

### 3.6 `data-cleaning`

作用：数据清洗与质量控制。

覆盖：

- CSV / XLSX 读取。
- 缺失值分析。
- 异常值识别。
- 变量类型推断。
- 单位标准化。
- 数据字典一致性检查。

输出：

- 数据概况。
- 缺失值报告。
- 清洗计划。
- 清洗脚本。
- 清洗后数据集。

### 3.7 `biostatistics`

作用：统计分析设计与执行。

覆盖：

- Table 1。
- 组间比较。
- Logistic 回归。
- Cox 回归。
- Kaplan-Meier 曲线。
- IPTW。
- PSM。
- 敏感性分析。
- 亚组分析。

输出：

- 统计分析计划。
- R / Python 分析脚本。
- 模型结果。
- 统计解释。
- 方法学段落草案。

### 3.8 `figure-table`

作用：生成科研表格和图表。

覆盖：

- Table 1。
- 回归结果表。
- 森林图。
- KM 曲线。
- 纳排流程图。
- 敏感性分析图。

输出：

- 表格文件。
- 图表文件。
- 图表标题。
- 图注。
- 论文可用结果摘要。

### 3.9 `manuscript-writing`

作用：将结构化结果转成正式科研文本。

覆盖：

- 研究方案。
- 方法学。
- 结果。
- 讨论。
- 摘要。
- 投稿格式。
- 中文和英文双语草稿。

输出：

- 文档段落。
- 论文草稿。
- 修订建议。
- 引用占位或真实引用。

### 3.10 `evidence-linking`

作用：建立证据链。

覆盖：

- 文档段落与证据关联。
- 统计结果与代码输出关联。
- 图表与 Session 关联。
- 版本与批准记录关联。

输出：

- 证据链记录。
- 引用映射。
- 可追溯关系。
- 缺失证据提示。

## 4. P0 MCP / Tool 清单

### 4.1 Filesystem MCP

用途：

- 读取工作区文件。
- 写入 Session 产物。
- 管理项目目录。
- 保存计划、日志、代码和文档。

P0 必需。

### 4.2 Code Runner MCP

用途：

- 执行 Python / R / Shell。
- 保存脚本。
- 捕获 stdout / stderr。
- 记录退出码和耗时。
- 产出可复现运行记录。

P0 必需。

### 4.3 Spreadsheet MCP

用途：

- 读取 Excel / CSV。
- 解析数据字典。
- 生成 Table 1。
- 输出 XLSX / CSV 结果。

P0 必需。

### 4.4 Document MCP

用途：

- 读取 Markdown / DOCX。
- 写入方案和论文草稿。
- 保存修订版本。
- 导出报告。

P0 必需。

### 4.5 PDF MCP

用途：

- 解析指南 PDF。
- 解析论文 PDF。
- 提取段落和引用信息。

P0 建议必需。

### 4.6 Literature MCP

用途：

- PubMed 检索。
- Crossref 查询。
- Semantic Scholar / Europe PMC 扩展。
- 获取 DOI、标题、作者、摘要和引用格式。

P0 建议必需。

### 4.7 Evidence Store MCP

用途：

- 存储证据片段。
- 存储文档段落。
- 存储 Session 回流摘要。
- 支持引用回溯。

P0 可先用本地 SQLite / JSONL，P1 再扩展向量检索。

### 4.8 Git / Version MCP

用途：

- 记录代码版本。
- 记录文档版本。
- 记录产物版本。
- 支持 diff 和回滚。

P0 可先做轻量版本记录，P1 接入完整 Git 能力。

### 4.9 Database MCP

用途：

- SQLite / DuckDB 本地分析。
- 后续接入 PostgreSQL、OMOP、FHIR 或院内数据库。

P0 可选，P1 建议接入。

### 4.10 Collaboration MCP

用途：

- 分派 Session。
- 通知协作者。
- 评论与签核。
- 后续接入飞书、企业微信、邮件或内部账号系统。

P0 可先做 UI 壳和本地 assignment 记录。

## 5. Session 与能力映射

| Session | 主要 Skill | 主要 MCP / Tool | 回流产物 |
| --- | --- | --- | --- |
| 课题定义 | `research-plan`, `clinical-research-design` | Filesystem, Document | 研究问题、关键假设、计划草案 |
| 文献证据 | `literature-evidence`, `evidence-linking` | Literature, PDF, Evidence Store | 文献摘要、证据片段、引用列表 |
| 队列构建 | `cohort-definition`, `data-cleaning` | Spreadsheet, Filesystem, Code Runner | 队列定义表、纳排流程、cohort.py |
| 数据清洗 | `data-cleaning` | Spreadsheet, Code Runner, Filesystem | 缺失值报告、清洗脚本、清洗后数据 |
| 统计分析 | `biostatistics` | Code Runner, Spreadsheet, Evidence Store | Table 1、模型结果、统计解释 |
| 图表与论文 | `figure-table`, `manuscript-writing`, `evidence-linking` | Code Runner, Document, Evidence Store | 图表、方法学段落、结果段落、论文草稿 |

## 6. 最小可上线组合

如果只做第一版商业 MVP，建议收敛为：

### 6.1 必备 Skill

- `research-plan`
- `session-orchestration`
- `clinical-research-design`
- `cohort-definition`
- `data-cleaning`
- `biostatistics`
- `manuscript-writing`

### 6.2 必备 MCP / Tool

- Filesystem。
- Code Runner。
- Spreadsheet。
- Document。
- Literature。
- Evidence Store。

### 6.3 可后置

- 完整 Git。
- 多人协作。
- 院内数据库。
- OMOP / FHIR。
- 投稿包自动导出。
- 实时云同步。

## 7. 数据契约建议

### 7.1 Skill 输入契约

每个 Skill 不应直接读取全部项目上下文，而应接收明确输入：

```json
{
  "project_id": "research-project-id",
  "session_id": "session-id",
  "goal": "当前任务目标",
  "confirmed_assumptions": [],
  "selected_artifacts": [],
  "available_tools": [],
  "output_contract": {}
}
```

### 7.2 Session 回流契约

每个 Session 回流主线程时，至少包含：

```json
{
  "session_id": "session-id",
  "title": "队列构建",
  "status": "completed",
  "summary": "已完成 HFrEF 队列定义与样本量统计。",
  "key_results": [],
  "artifacts": [],
  "evidence_links": [],
  "code_refs": [],
  "risks": [],
  "requires_approval": false,
  "next_suggestion": "进入数据清洗 Session。"
}
```

### 7.3 Evidence 记录契约

证据记录建议包含：

```json
{
  "evidence_id": "evidence-id",
  "source_type": "pubmed | pdf | dataset | code_output | human_approval",
  "source_ref": "来源路径或外部 ID",
  "claim": "该证据支撑的声明",
  "quote_or_summary": "证据摘要",
  "linked_session_id": "session-id",
  "linked_artifact_id": "artifact-id",
  "confidence": "high | medium | low"
}
```

## 8. 实现路线

### M1：本地能力壳

- 建立 Skill 注册表。
- 建立 Session 类型定义。
- 建立 MCP / Tool 能力清单。
- 使用模拟数据展示 Session 能力映射。
- 在 UI 中展示每个节点的可用 Skill 和可用工具。

### M2：真实文件与数据能力

- 接入 Filesystem。
- 接入 Spreadsheet。
- 接入 Document / PDF。
- 建立本地 Evidence Store。
- 支持 Session 写入产物。

### M3：代码执行能力

- 接入 Code Runner。
- 保存脚本、日志、退出码和产物。
- 支持统计分析 Session 真实执行。
- 支持结果回流主线程。

### M4：证据链与版本

- 建立 evidence linking。
- 建立版本记录。
- 支持文档段落、代码输出、证据片段互相关联。

### M5：协作与分派

- 支持 assignment。
- 支持协作者视图。
- 支持评论和签核。
- 后续接入外部协作系统。

## 9. 工程落点

建议新增或调整：

- `rust/crates/galen/src/domain/session.ts`
- `rust/crates/galen/src/domain/skills.ts`
- `rust/crates/galen/src/domain/tools.ts`
- `rust/crates/galen/src/components/ResearchPlanCanvas.tsx`
- `rust/crates/galen/src/components/SessionInspectorDrawer.tsx`
- `rust/crates/galen/src-tauri/src/research_context.rs`
- `rust/crates/galen/src-tauri/src/session_orchestration.rs`
- `rust/crates/galen/src-tauri/src/tools/research.rs`
- `rust/crates/medical-core/src/research.rs`

当前已有 `docs/galen-llm-context-tools.md`，后续实现时应把其中的 context pack 思路升级为 Session Context Pack。

## 10. 验收标准

### 产品验收

- 用户能理解 Skill 与 MCP 不直接暴露为工具列表，而是被封装进 Session。
- 每个 Session 都能说明它会使用哪些能力、读取哪些输入、产生哪些输出。
- 主线程只接收结构化回流，不接收全部原始上下文。
- 计划画布能表现出并行执行、分派和回流关系。

### 工程验收

- Skill、MCP / Tool、Session 三层对象边界清晰。
- P0 不依赖云端服务也能跑通本地模拟闭环。
- 代码执行、文件读写和证据记录都有日志。
- 所有产物能追溯到 Session、代码、输入和证据。

## 11. 商业化优先级

第一阶段不要卖“Galen 全能科研平台”，而要卖一个可交付闭环：

> 医生上传数据字典和 Excel，Galen 生成研究计划、队列定义、Table 1、统计结果、图表和论文方法 / 结果段落。

因此 P0 最优先打穿：

1. 主线程确认计划。
2. 计划画布生成 Session。
3. 队列构建 Session。
4. 统计分析 Session。
5. 论文结果回流。

这条链路最容易形成可感知价值，也最容易转化为课题组或科室付费。
