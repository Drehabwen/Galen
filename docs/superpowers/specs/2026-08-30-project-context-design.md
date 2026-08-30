# Galen Project Context 设计规格

> 日期：2026-08-30
>
> 状态：已确认，进入实施
>
> 方案：B — 宿主权威的 Project Context
>
> 适用分支：`galen-research-workbench`

## 1. 背景

真实用户反馈暴露了两个表面相反、根因相同的问题：

1. 用户已经改变研究方向，Galen 仍继续使用旧方向；
2. 用户持续讨论同一课题，Galen 却遗忘已经确认的信息并重复提问。

当前系统已经具有四类持久状态：工作区会话、用户决策账本、`GALEN.md` 项目记忆和 `ResearchTask`。这些状态能够支持会话恢复、具体参数修订和任务交付，但没有一个结构化对象明确表达“当前项目的有效事实”。模型仍需要从聊天和多个存储中推断当前研究问题、有效范围、已排除方向与证据覆盖，因此既可能错误延续旧状态，也可能遗漏应该保留的新状态。

用户还明确要求 Galen 直接完成文献检索，而不是只返回检索式。当前系统能够执行 PubMed 检索，但没有记录数据库覆盖范围，也无法约束模型在只检索 PubMed 后不得把“当前未检出”扩大为“整体没有证据”。

## 2. 目标

第一阶段建立一个由 Rust 宿主维护、版本化、可恢复、可编辑的 `ProjectContext`，作为当前项目状态的权威来源，并打通四个行为：

1. 同一课题持续讨论时，保留当前研究问题、范围与有效决定；
2. 用户明确改变方向时，旧范围退出当前上下文但保留审计历史；
3. 用户要求检索文献时，Galen 实际执行可用数据库工具，而不是只返回检索语句；
4. 所有证据结论显式受数据库覆盖范围约束。

## 3. 非目标

本阶段不实现：

- CNKI、万方、VIP 或 CBM 的网页登录自动化；
- 通用知识图谱或完整 Rehab Context Graph；
- 多人协同和云同步；
- 自动决定模糊表达是否代表全新课题；
- 删除旧会话、旧任务、旧 Evidence 或旧决定；
- 新的常驻科研画布；
- 其他疾病或康复场景扩展。

中文数据库第一阶段只表达覆盖状态，并预留题录导入能力的接口；题录导入实现不进入本次最小切片。

## 4. 设计原则

### 4.1 宿主权威

`ProjectContext` 保存在工作区，由 Rust 宿主读写和校验。模型只能通过受约束的结构化工具提出变更，不能依赖自然语言回复宣称项目状态已经改变。

### 4.2 历史不删除，当前态可替换

“忘记旧方向”不是删除历史，而是将旧方向标记为排除或被替代，使其不再进入当前模型上下文。原始会话、决定和任务继续作为审计记录保存。

### 4.3 执行优先于教学

当用户目标是获得检索结果且存在可用工具时，Agent 必须调用工具。检索式属于高级详情，不作为默认交付物替代检索结果。

### 4.4 声明不得超出覆盖范围

结论必须绑定已完成的数据库覆盖。未检索或不可用的来源必须可见；“当前已检索来源未发现”不能升级为“没有证据”。

### 4.5 渐进兼容

`ProjectContext` 引用现有 `ResearchTask` 和决策记录，不在第一阶段取代它们。现有工作区没有新文件时，应从活动任务生成最小初始上下文，不修改历史文件。

## 5. 权威存储

路径：

```text
<workspace>/.galen/project-context.json
```

写入规则：

- JSON 使用 UTF-8；
- 每次写入先检查调用方提供的 `expectedRevision`；
- 写入临时文件并原子替换；
- 成功写入后 `revision + 1`；
- 失败不得覆盖旧文件；
- 工作区路径继续通过现有 `WorkspacePath` 边界保护；
- 所有时间字段使用宿主生成的毫秒时间戳字符串，与现有任务存储保持兼容。

## 6. 数据模型

Rust 权威类型：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectContext {
    pub schema_version: u32,
    pub revision: u64,
    pub project_id: String,
    pub research_question: String,
    pub active_scope: Vec<String>,
    pub excluded_directions: Vec<ExcludedDirection>,
    pub evidence_sources: BTreeMap<String, EvidenceSourceCoverage>,
    pub active_task_id: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExcludedDirection {
    pub direction: String,
    pub reason: String,
    pub excluded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    NotSearched,
    Searching,
    Searched,
    Unavailable,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceSourceCoverage {
    pub status: CoverageStatus,
    pub searched_at: Option<String>,
    pub query_summary: Option<String>,
    pub result_count: Option<u64>,
    pub reason: Option<String>,
}
```

固定来源键：

```text
pubmed
cnki
wanfang
vip
cbm
guidelines
workspace
```

未知来源允许使用小写 ASCII slug 扩展，但必须通过长度和字符校验。来源状态不能由最终文字回复隐式修改。

`projectId` 在第一次落盘时由宿主生成 `project-<毫秒时间戳>`，之后保持不变。首次创建时初始化全部固定来源：`pubmed`、`guidelines` 和 `workspace` 为 `not_searched`；没有连接器的中文数据库为 `unavailable` 并保存“尚未配置连接器”的原因。

## 7. 状态生命周期

### 7.1 首次加载

工作区没有 `project-context.json` 时：

1. 如果存在活动 `ResearchTask`，使用其 `taskId`、`goal` 和 `title` 创建内存中的最小上下文；
2. 如果没有活动任务，返回空上下文；
3. 只读加载不得立即落盘；
4. 第一次用户确认研究问题或第一次结构化修改时才创建文件。

文献检索属于结构化状态修改：如果检索发生时文件仍不存在，宿主以 Revision `0` 和当前活动任务生成最小上下文，并在写入 `searching` 覆盖状态时完成第一次落盘。

### 7.2 同一课题继续

普通对话不会修改 `researchQuestion`。每轮动态上下文注入当前项目状态：

- 当前研究问题；
- 有效范围；
- 已排除方向；
- 当前任务；
- 最近有效决定；
- 数据库覆盖状态。

已确认字段不得由模型重复询问，除非信息之间存在明确冲突或用户要求重新讨论。

### 7.3 明确替换方向

只有用户明确表达“改为”“彻底去掉”“不再采用”“重新开始一个新方向”等替换意图时，Agent 才调用结构化修改工具。

成功替换执行：

1. 旧 `researchQuestion` 如非空，加入 `excludedDirections`；
2. 新问题写入 `researchQuestion`；
3. `activeScope` 使用调用中提供的新范围完整替换，不与旧范围合并；
4. 明确取消的其他方向写入 `excludedDirections`；
5. 当前 `activeTaskId` 置空，同时清除 `.galen/active-task.json` 活动指针；旧任务目录与任务文件完整保留；
6. 与旧问题绑定的数据库覆盖重置为 `not_searched`；
7. `revision` 增加；
8. 后续上下文不得将被排除方向作为当前建议重新引入，除非用户明确恢复。

### 7.4 修订而非替换

增加或删除一个范围项时只更新 `activeScope`，不清空当前任务和覆盖记录。调用方必须明确选择 `replace_direction` 或 `patch_scope`，宿主不从自由文本自行猜测操作类型。

## 8. 宿主 API

新增应用服务模块：

```text
rust/crates/galen/src-tauri/src/project_context.rs
```

公开函数：

```rust
pub fn load_project_context(workspace: &Path) -> Result<Option<ProjectContext>, String>;

pub fn ensure_project_context(workspace: &Path) -> Result<ProjectContext, String>;

pub fn replace_project_direction(
    workspace: &Path,
    expected_revision: u64,
    research_question: String,
    active_scope: Vec<String>,
    excluded_directions: Vec<String>,
    reason: String,
) -> Result<ProjectContext, String>;

pub fn patch_project_scope(
    workspace: &Path,
    expected_revision: u64,
    add: Vec<String>,
    remove: Vec<String>,
) -> Result<ProjectContext, String>;

pub fn update_evidence_coverage(
    workspace: &Path,
    expected_revision: u64,
    source: String,
    coverage: EvidenceSourceCoverage,
) -> Result<ProjectContext, String>;

pub fn render_project_context(context: &ProjectContext) -> String;
```

`ensure_project_context` 只供第一次结构化写入和工具覆盖更新使用。方向替换通过现有 ResearchTask 应用服务新增的 `deactivate_active_task` 清除活动指针；该操作不删除任务目录。

Tauri 命令：

```text
get_project_context
replace_project_direction
patch_project_scope
```

前端编辑使用 Tauri 命令。Agent 使用 Tool Registry 中的受约束工具，不直接调用 IPC。

## 9. Agent 工具

新增工具 `update_project_context`：

```json
{
  "operation": "replace_direction | patch_scope",
  "expected_revision": 3,
  "research_question": "...",
  "active_scope": ["..."],
  "add_scope": ["..."],
  "remove_scope": ["..."],
  "excluded_directions": ["..."],
  "reason": "用户明确改变研究方向"
}
```

约束：

- `replace_direction` 必须提供非空研究问题和非空理由；
- `patch_scope` 至少有一个增加或删除项；
- 变更必须携带当前 Revision；
- Revision 冲突返回结构化错误，模型必须重新读取状态，不得重试旧参数；
- 工具结果返回更新后的完整 `ProjectContext`；
- Discuss 模式不允许写入，Auto 和 Plan 模式遵循现有写工具权限边界。

## 10. 上下文组装

`context_engine::build_turn_context` 新增 `ProjectContext` 摘要，放在 `ResearchTask` 进度之前。摘要只包含当前有效状态和覆盖状态，不注入全部历史。

示例：

```text
## 当前项目状态（宿主权威）
研究问题：脑卒中居家上肢训练依从性
有效范围：居家训练；上肢；依从性；叙述性综述
已排除方向：中西医结合（用户明确取消）
证据覆盖：PubMed=已检索；CNKI=不可用；万方=未检索
约束：不得将已排除方向当作当前范围；证据声明不得超出已检索来源。
```

`ProjectContext` 与决策账本冲突时：

1. `ProjectContext` 决定当前研究问题、范围、排除方向与覆盖；
2. 决策账本继续提供样本量、结局、随访等细粒度有效决定；
3. 后续实现可以将稳定决定迁移为 Project Context 子对象，本阶段不做双写迁移。

## 11. PubMed 自动执行与覆盖写入

现有 `search_pubmed` 和 `search_rehab_literature` 保持检索职责。成功执行后，由工具实现调用 `update_evidence_coverage`：

- 开始调用前写入 `searching`；
- 成功后写入 `searched`、宿主时间、查询摘要和结果数量；
- API、解析或网络失败写入 `failed` 和非敏感错误摘要；
- 用户 Key、完整请求头和响应正文不得写入 Project Context；
- 覆盖写入失败时，工具调用整体返回错误，不能出现“检索成功但状态仍未知”的分叉。

工具每次更新前重新读取当前 Revision：先持久化 `searching`，完成请求后基于新的 Revision 写入 `searched` 或 `failed`。应用异常退出后遗留的 `searching` 在下次加载时渲染为“上次检索未完成”，不得视为已覆盖。

文献请求的 Agent 契约增加硬规则：存在可用检索工具时必须实际调用；最终回复默认展示结果和覆盖摘要，检索语句放在可选详情中。

## 12. Evidence Coverage 结论边界

上下文中加入确定性覆盖规则：

- 只有 `searched` 来源属于已覆盖；
- `failed`、`unavailable` 和 `not_searched` 必须在结论中视为未覆盖；
- 存在任一计划内来源未覆盖时，不得使用“没有证据”“不存在研究”“该方向无文献”等全局否定；
- 允许使用“在本次已完成的 PubMed 检索中未发现……”；
- 搜索结果为零不等于来源不可用，仍记录为 `searched` 且 `resultCount=0`；
- Coverage 约束属于交付硬门，不能由回答流畅度或速度抵消。

## 13. 前端

新增轻量组件，不增加常驻画布：

```text
ProjectContextStrip
├── 当前研究问题
├── 有效范围摘要
└── 编辑入口

EvidenceCoverageCard
├── 来源状态
├── 最近检索时间
├── 结果数量
└── 未覆盖说明
```

放置策略：

- `ProjectContextStrip` 位于日常工作台顶部、对话区域之上；
- `EvidenceCoverageCard` 位于现有 Context/Inspector 区域；
- 默认只显示摘要，展开后才显示检索策略和错误原因；
- 被排除方向不在主视图反复展示，只在编辑或审计视图可见；
- Revision 冲突时重新加载并显示“项目状态已更新，请基于最新状态重试”。

## 14. 错误处理

- 文件损坏：返回明确错误并保留原文件，不自动覆盖；
- Revision 冲突：拒绝写入并返回当前 Revision；
- 非法来源键：拒绝更新；
- 空研究问题：拒绝方向替换；
- 重复范围项：标准化、去重并保持首次出现顺序；
- 同一排除方向重复写入：保留最早审计记录，不重复追加；
- PubMed 失败：Coverage 写为 `failed`，最终回复说明失败，不生成伪结果；
- 无工作区：所有 Project Context 写操作拒绝执行。

## 15. 测试设计

### 15.1 Rust 单元测试

- 空工作区返回 `None`；
- 从活动 ResearchTask 生成只读最小上下文；
- 首次方向确认创建文件；
- 明确方向替换清空 `activeTaskId` 和旧覆盖；
- 旧方向进入排除记录；
- Scope Patch 不清空任务和覆盖；
- Revision 冲突不覆盖文件；
- 原子写入失败保留旧文件；
- 来源键校验；
- Coverage 状态序列化和恢复；
- 摘要只渲染当前状态；
- ProjectContext 优先于冲突的通用决策记录。

### 15.2 工具测试

- `update_project_context` 在 Auto 模式成功；
- Discuss 模式拒绝写入；
- 缺少 Revision 或理由时拒绝；
- PubMed 成功后 Coverage 为 `searched`；
- PubMed 失败后 Coverage 为 `failed`；
- Coverage 写入失败时检索不得报告完成。

### 15.3 前端测试

- 状态条显示研究问题和范围；
- 覆盖卡区分已检索、未检索、不可用和失败；
- Revision 冲突后重新加载；
- 空 Project Context 不显示伪状态；
- 编辑方向时明确区分替换与范围修订。

### 15.4 Agent 评测

新增固定案例：

1. `E13 same-topic continuity`：连续多轮不得重复询问已确认课题；
2. `E14 direction replacement`：旧方向退出后续回答但保留审计记录；
3. `E15 search execution`：要求找文献时必须实际调用 PubMed；
4. `E16 coverage boundary`：只覆盖 PubMed 时不得作全局无证据声明；
5. `E17 restart recovery`：重启后 Project Context 完整恢复；
6. `E18 workspace isolation`：两个工作区状态不得串扰。

每个案例开发阶段至少重复 5 次；进入 Release Gate 后至少重复 20 次。方向串扰、虚假检索和超范围结论均为硬门失败。

## 16. 实施切片

实现按以下顺序推进，每个切片可独立测试和回退：

1. `ProjectContext` 类型、存储、Revision 和迁移读取；
2. Tauri 查询与编辑命令；
3. `update_project_context` Agent 工具；
4. 动态上下文注入与冲突优先级；
5. PubMed Coverage 状态写入；
6. Project Context 状态条与 Evidence Coverage 卡；
7. E13—E18 确定性契约和真实模型评测。

## 17. 完成定义

本阶段只有同时满足以下条件才算完成：

- 用户不需要在同一项目中重复说明当前研究问题和已确认范围；
- 明确替换方向后，旧方向不再进入当前模型上下文；
- 旧状态仍可审计，不发生数据删除；
- 文献请求实际执行 PubMed；
- Galen 显示已检索与未覆盖的数据库；
- 只检索 PubMed 时不会产生全局“无证据”结论；
- 重启和工作区切换保持正确隔离；
- 新增单元、组件和 Agent 评测通过；
- 当前 34 项前端测试和 136 项 Galen Rust 测试不回退。
