# Galen UI 实施交接计划

用途：交给后续 agent 按阶段执行。  
状态：待实施。  
不得重新发散产品方向。

## 1. 执行目标

根据当前高保真原型、PRD 和 UI 体系文档，重构 Galen 桌面端 UI，使其收敛为：

> 对话驱动科研协作 + 日常科研工作台 + 文档选区修订 + GitHub 式标签 + 证据链 + AI 执行状态。

实现目标不是继续生成新概念图，而是把现有 React/Tauri 前端按既定设计体系落地。

## 2. 必读输入

后续 agent 开始前必须先读：

- `docs/galen-prd.md`
- `docs/galen-ui-system.md`
- `docs/galen-reuse-consistency-audit.md`
- `rust/crates/galen/src/styles/tokens.css`
- `rust/crates/galen/src/styles/layout.css`
- `rust/crates/galen/src/styles/components.css`
- `rust/crates/galen/src/styles/workbench.css`
- `rust/crates/galen/src/App.tsx`
- `rust/crates/galen/src/components/ResearchWorkbench.tsx`
- `rust/crates/galen/src/components/ChatPanel.tsx`
- `rust/crates/galen/src/components/ui/primitives.tsx`
- `rust/crates/galen/src/domain/`

视觉基准为用户提供的两张原型图：

- 图 1：`科研执行线程态`，对话驱动、文档选区修订、证据链、版本记录、协作者。
- 图 2：`日常科研工作台态`，当前目标、阶段进度、AI 执行状态、下一步建议、最近产物、待签核。

## 3. 非目标

- 不重新定义 Galen 产品定位。
- 不把 Kinestra、Fitwork、运动处方或硬件闭环混入当前 Galen 主线。
- 不做深色工程控制台。
- 不默认展开完整代码、日志、任务图和证据。
- 不把右侧 AI 区做成第二个聊天主界面。
- 不引入大型 UI 框架，优先使用现有 React + CSS 结构。

## 4. 实施顺序

### Phase 1: 色彩 token 迁移

目标：让全局视觉先接近原型图。

修改重点：

- `rust/crates/galen/src/styles/tokens.css`

要求：

- 主背景从偏米白改为白色和极浅临床灰。
- 主操作色改为医疗绿 `#006b5a`。
- 当前橙色 accent 降级为 warning/approval。
- 增加或整理以下 token：
  - canvas
  - app background
  - sidebar background
  - panel background
  - border
  - brand
  - success
  - warning
  - danger
  - tag semantic colors

验收：

- 页面不再呈现米黄/橙色主视觉。
- 主按钮、运行中状态、进度条统一使用品牌绿。
- 待签核、风险提醒使用琥珀色。

### Phase 2: 基础组件统一

目标：将按钮、标签、状态、进度条、卡片和浮层统一到设计体系。

修改重点：

- `rust/crates/galen/src/styles/components.css`
- `rust/crates/galen/src/components/ui/primitives.tsx`

新增或强化组件：

- `Tag`
- `StatusDot`
- `ProgressBar`
- `ApprovalCard`
- `BottomDrawerItem`
- `InspectorSection`
- `SelectionActionMenu`

验收：

- GitHub 式标签具备阶段、状态、证据、风险、责任、执行六类视觉。
- 所有状态文案正式化：`待研究者确认`、`批准继续`、`查看依据`、`证据链已关联`。
- 浮层和菜单只使用轻阴影，普通面板只用细边框。

### Phase 3: Shell 布局重构

目标：实现与原型一致的应用骨架。

修改重点：

- `rust/crates/galen/src/App.tsx`
- `rust/crates/galen/src/styles/layout.css`

结构要求：

- 顶部栏：`Galen | 对话驱动科研协作 / 日常科研工作台`，模式，AI 状态，通知，用户。
- 左侧栏：我的课题/当前课题、搜索、课题列表、工作区入口、设置。
- 主区支持两种视图：
  - `科研执行线程态`
  - `日常科研工作台态`
- 右侧区作为检查区：证据链、版本记录、协作者，或 AI 执行状态摘要。
- 底部折叠抽屉：任务图、代码、日志、证据、版本、产物。

验收：

- 布局不再是传统左文件树 + 右聊天的割裂形态。
- AI 执行状态和科研线程能并存，但不会形成两个聊天主界面。
- 底部深层能力默认折叠。

### Phase 4: 日常科研工作台态

目标：先实现图 2 的默认常驻形态。

修改重点：

- `rust/crates/galen/src/components/ResearchWorkbench.tsx`
- `rust/crates/galen/src/styles/workbench.css`
- `rust/crates/galen/src/domain/`

界面模块：

- 当前目标。
- 五阶段进度：课题设计、数据处理、统计分析、图表生成、论文写作。
- 正在执行。
- 最近产物表。
- 右侧 AI 执行状态。
- 下一步建议。
- 等待签核。

验收：

- 默认态足够留白。
- 只展示当前目标、当前执行、下一步和待签核。
- 最近产物使用列表/表格，不堆卡片。

### Phase 5: 科研执行线程态

目标：实现图 1 的对话驱动协作形态。

修改重点：

- `rust/crates/galen/src/components/ChatPanel.tsx`
- 可新增 `ResearchExecutionThread.tsx`
- 可新增 `ResearchDocumentCanvas.tsx`
- 可新增 `ResearchInspector.tsx`

界面模块：

- 科研执行线程。
- 用户正式请求。
- AI 计划块。
- 修订建议块。
- 关联证据块。
- 待签核卡片。
- 文档画布。
- 证据链/版本记录/协作者。

验收：

- 对话、计划、执行、修订、签核在同一线程中呈现。
- 右侧只做检查，不做主对话。
- 文档画布像正式论文/方案编辑区，而不是 dashboard 卡片。

### Phase 6: 选区修订交互壳

目标：先实现 UI 壳，不要求完整 AI 端到端能力。

修改重点：

- 文档画布组件。
- `SelectionActionMenu`。
- 线程中的修订建议块。

交互要求：

- 用户选中文档文本。
- 出现 `AI 修订` 浮层菜单。
- 操作项：
  - `润色表达`
  - `补充依据`
  - `改写为统计语言`
  - `生成替代版本`
  - `解释此段落`
- 选择动作后，在科研执行线程中生成对应的 UI 事件或 mock 修订建议。

验收：

- 选区高亮清晰。
- 浮层位置和尺寸克制。
- 修改建议必须进入线程，不直接覆盖文档。

## 5. 文案规则

必须使用正式医学科研语言。

推荐：

- `科研执行线程`
- `当前目标`
- `正在执行`
- `下一步建议`
- `待研究者确认`
- `批准继续`
- `查看依据`
- `接受修订`
- `证据链已关联`
- `版本记录`

避免：

- `搞定`
- `主驾驶` 作为所有场景的一级标题
- `帮你`
- `魔法`
- 口语化夸张表达

说明：

- `AI 主驾驶` 可以在图 2 的日常态右侧作为概念标题使用。
- 在图 1 的正式协作态中，优先用 `科研执行线程` 和 `AI 执行状态`。

## 6. 验证要求

每个实现阶段至少运行：

```powershell
cd rust/crates/galen
npm run build
```

如果改动 Rust/Tauri：

```powershell
cd rust
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

如果启动本地预览：

```powershell
cd rust/crates/galen
npm run dev -- --host 0.0.0.0 --port 5173
```

视觉验收：

- 桌面宽屏下无文本重叠。
- 左栏、主区、右栏、底部抽屉比例接近原型。
- 默认态不拥挤。
- 标签颜色符合语义。
- 待签核和风险状态明显但不刺眼。

## 7. 推荐提交拆分

建议拆成以下提交或 PR：

1. `ui tokens`: 色彩 token 和基础语义色。
2. `ui primitives`: 标签、状态点、进度条、签核卡片、抽屉项。
3. `daily workbench`: 日常科研工作台态。
4. `execution thread`: 科研执行线程态。
5. `selection revision`: 文档选区修订交互壳。

## 8. 完成定义

第一轮 UI 实施完成需要满足：

- `docs/galen-ui-system.md` 的核心视觉方向已反映在代码中。
- 应用默认界面接近图 2。
- 用户进入执行协作时接近图 1。
- 主色、标签、状态、签核、底部抽屉风格一致。
- `npm run build` 通过。
- 未引入与 Galen 定位无关的新产品概念。
