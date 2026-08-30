# Galen AI 开发交接文档

> 快照日期：2026-08-29  
> 当前维护分支：`galen-research-workbench`  
> 当前提交：`78e74c752183b835d54c8e81b8c7d8ec402ccbb3`  
> 最新公开版本：`v0.1.2`  
> 面向对象：下一位接手 Galen 产品、工程或评测工作的 AI

## 1. 接手后先记住这一句话

Galen 不是通用聊天助手，也不是普通医学科研写作工具。它是一个面向康复科研的本地工作台：把真实康复资料转化为可核验的纵向病例、研究可用队列、分析与报告，并把缺失和不可比较问题反馈到下一轮采集。

P0 只聚焦一个切口：**脊柱侧弯保守治疗纵向病例研究闭环**。

不要继续横向增加疾病、Pack、Agent 模式、插件市场或协作功能。下一阶段的目标不是让模型“显得更聪明”，而是让一个真实科研任务能够稳定、快速、可追溯地完成。

## 2. 产品北极星与系统边界

北极星指标：

> 每个研究者小时产出的、经过来源核验且满足研究可比性规则的纵向病例行数量。

P0 闭环：

```text
真实病例资料
  → CaseRecord / ClinicalEvent / Observation
  → 最小人工确认 ReviewDecision
  → 研究可用 CohortRow
  → 分析、图表和研究报告
  → Evidence 反查与 Galen 内部预览
  → 下一轮标准化采集规范
```

系统责任边界：

- 模型负责理解、判断、研究推理和内容生成。
- 宿主负责状态、工具顺序、路径边界、证据约束、恢复、收敛和交付闸门。
- 未核验事实不得进入精确评分硬门或主要科研结论。
- Galen 输出是科研辅助，不是临床诊断或个体治疗建议。
- 不从无标注原始 X 光图像自动计算 Cobb 角；P0 只读取已有、可定位的测量值。

现行产品权威文档是 [`docs/galen-prd.md`](galen-prd.md)。旧的 `galen-prd-v0.2.md` 只用于理解演进历史，不是当前需求基线。

## 3. 当前可交付状态

### 3.1 已发布

- Windows 公开版本：`v0.1.2`
- Release：<https://github.com/Drehabwen/Galen/releases/tag/v0.1.2>
- Windows 安装包：<https://github.com/Drehabwen/Galen/releases/download/v0.1.2/Galen_0.1.2_x64-setup.exe>
- Tauri 自动更新签名已经配置并正常生成 `latest.json`。
- Tauri 更新签名不等于 Windows Authenticode；当前安装包仍可能显示“未知发布者”。

### 3.2 已验证能力

- 新用户向导、模型配置、工作区进入链路已经可以运行。
- Agent 可以创建任务、执行工具、生成 Artifact、登记到研究任务并进入 `deliverable` 状态。
- 长上下文任务可以保留样本量、主要结局和随访时间等关键约束。
- Markdown Artifact 可以在 Galen 内渲染标题、列表、引用和 GFM 表格，而不是显示源码。
- 从全局产物库打开 Artifact 可以自动回到研究任务成果视图。
- 前端已有 9 项测试；架构稳定化时 Rust 测试曾达到 126 项，发布前记录达到 134 项。运行当前分支时以实际测试输出为准。
- `App.tsx` 已从约 793 行降至 286 行，前端状态职责已明显拆分。

### 3.3 尚未完成

- 真实 10 例脊柱侧弯病例的完整桌面端 P0 旅程尚未全部跑通。
- CaseRecord、ClinicalEvent、Observation、ReviewDecision、CohortRow 到分析报告的产品闭环还没有达到发布验收标准。
- 当前 UI 自动化重点验证了 Markdown；患者时间轴、队列、图表、PDF、DOCX、XLSX Viewer 仍需逐类验证。
- 真实关闭/重启恢复与双工作区切换还缺少专用端到端 Runner。
- Evidence 引用真实性与科研方法 Rubric 还需加强。
- Authenticode 代码签名证书尚未接入。

## 4. 之前测试真正告诉了我们什么

### 4.1 Agent 基础闭环已经成立

E07 与 E09 使用 Flash、Pro 各重复 20 次，共 80 次：

- 80/80 通过硬门；
- 80/80 Artifact 有效且可预览；
- 工具错误为 0；
- 每次稳定收敛为 2 次模型请求和 1 次 `write_file`；
- 四组 TTFR P50 为 0.7—1.3 秒。

因此不要再以“修复 150 秒稳定启动延迟”为理由重写整个 Agent Loop。当前仍有偶发约 23 秒无首响应长尾，更像模型服务或网络尖峰。产品必须用即时本地回执、阶段提示、取消和 Flash 降级保护用户，而不是假设能完全消灭上游长尾。

证据：[`evals/reports/release-tail-artifact-preview-2026-08-29.md`](../evals/reports/release-tail-artifact-preview-2026-08-29.md)

### 4.2 确定性宿主比增加提示词更有效

E03 的旧评分只检查是否调用过 `read_file` 与 `write_file`，产生了假阳性。严格检查路径、错误状态和顺序后，Flash 仅 1/5、Pro 仅 3/5。改为宿主按用户声明顺序执行只读预检并回放标准 ToolResult 后，两者均达到 5/5，同时 Flash 平均耗时下降 20%，Pro 下降 29%。

E07 收紧到只暴露必要写入工具后，Pro 平均耗时下降 37%，输入 Token 约减半。E09 将工具 JSON 载荷预算与最终文字预算分离后，消除了截断重写。

后续规则：

- 能由代码确定的动作，不让模型猜。
- 每一轮只暴露完成当前节点必需的工具。
- 工具载荷预算与最终回复预算分离。
- 测试工具路径、参数、结果、顺序和状态变化，不只测试“是否调用过”。

证据：[`evals/reports/long-tail-tool-eval-2026-08-29.md`](../evals/reports/long-tail-tool-eval-2026-08-29.md)

### 4.3 上下文压缩有效，但多轮重发仍浪费 Token

E11 消融结果：

- 成功率保持 5/5；
- 首轮上下文约从 31K 降至 13.5K Token，减少约 56%；
- TTFR 下降约 24%；
- GAI 从 78.1 提升到 85.6；
- 累计输入 Token 只下降约 9%。

压缩算法不是当前第一重写对象。下一步应该减少每轮重复发送的稳定上下文、长 ToolResult 和节点执行全文，保持系统提示词与工具 Schema 字节稳定以提高 Prompt Cache 命中。主线程只接收节点的结构化回流，原始日志和证据按需检索。

证据：[`docs/preprint-galen-v0.1.md`](preprint-galen-v0.1.md)

### 4.4 Flash 默认、Pro 按风险路由

- Flash 适合普通问答、固定文件处理和明确交付。
- Pro 适合复杂研究设计、统计判断、证据冲突和高风险结论。
- 高风险结论还必须经过证据门禁和人工签核，不能只依赖 Pro。
- 路由由任务复杂度、风险和失败状态决定，不要求用户频繁手动切模型。

## 5. 当前代码架构

```text
React Workbench
├── App.tsx                         应用壳与顶层组合
├── hooks/                          工作区、模型、研究执行、交付等控制器
├── components/                     对话、画布、产物预览、设置与向导
└── domain/                         前端任务与交付契约
        │ Tauri invoke / events
        ▼
src-tauri/src/commands/             IPC 参数转换与应用服务入口
        │
        ▼
Chat Runtime
├── backend/chat_loop.rs            Agent 回合编排
├── backend/context_engine.rs       动态上下文组装
├── backend/task_contract.rs        任务与交付约束
├── backend/chat_session.rs         会话状态
├── backend/conversation_memory.rs  多轮记忆
└── tools/                          Kernel 工具与 Capability 工具
        │
        ▼
Workspace Authority
├── .galen/tasks/<task-id>/         任务、证据、事件、产物和快照
├── GALEN.md                        项目长期记忆
├── output/                         权威交付文件
└── evals/                          案例、fixture、运行记录和报告
```

优先阅读：

1. [`AGENTS.md`](../AGENTS.md)
2. [`docs/galen-prd.md`](galen-prd.md)
3. [`docs/codebase-recon-report.md`](codebase-recon-report.md)
4. [`docs/galen-evaluation-and-negative-optimization.md`](galen-evaluation-and-negative-optimization.md)
5. [`evals/README.md`](../evals/README.md)
6. `rust/crates/galen/src-tauri/src/backend/chat_loop.rs`
7. `rust/crates/galen/src/App.tsx`

高风险改动区域：`chat_loop.rs`、`commands/`、会话恢复、任务持久化、Artifact 登记、上下文压缩边界。修改这些区域必须添加或更新回归测试。

## 6. 当前工作区状态与所有权

交接时工作区不是干净状态。不要使用 `git reset --hard`、`git checkout --` 或批量覆盖。

本轮构建优化涉及：

- `.github/workflows/galen-windows-release.yml`
  - 修正 `rust -> rust/target` 为 `rust -> target`；
  - 在维护分支预热 Windows release Cargo 缓存；
  - 标签发布复用同一 `shared-key`；
  - 移除发布流程中重复的前端生产构建。
- `.github/workflows/galen-macos.yml`
  - 修正相同的 Cargo 缓存路径错误。
- `rust/crates/galen/src-tauri/tauri.conf.json`
  - NSIS 压缩切换为 `zlib`，用稍大的安装包换取更快打包。

这些改动已经通过 JSON/YAML 解析、Cargo metadata、9 项前端测试和前端生产构建，但在本交接快照中尚未提交或推送。推送后第一次 Windows 分支预热仍会是冷编译；后续标签发布才会明显受益。

以下文件在本轮构建优化之前已经存在用户修改或新增内容，不得混入构建优化提交，除非先确认其目的：

- `docs/GALEN_ALPHA_EXPLORATION_GUIDE.md`
- `scripts/build_alpha_exploration_guide_pdf.py`
- `docs/assets/`
- `scripts/build_merged_doc.py`
- `scripts/build_merged_doc_charts.py`
- `scripts/merged_doc_content.py`

提交前必须再次运行 `git status --short`，只暂存当前任务明确拥有的文件。

## 7. 下一阶段唯一主线

不要同时开多条产品线。按以下顺序推进：

### P0-A：长尾体验保护

目标：即使模型或网络停顿，用户也不会面对静默转圈。

验收：

- 任务提交后 300 ms 内显示本地回执；
- 连续 2 秒没有可见文本时显示当前阶段；
- 超过 8 秒显示“模型仍在响应”及已完成的本地准备；
- 超过合理阈值允许取消；
- 普通任务可切换或自动降级 Flash；
- 探针单列 `TTFR > 8s` 事件率和无反馈时长；
- 不以伪造进度或循环动画冒充真实执行进展。

### P0-B：10 例 AIS 黄金旅程

目标：用现有 10 例已核验病例跑通真实产品路径，而不是只运行独立问答或文件交付案例。

验收：

- 病例不串线；
- 不泄漏未来信息；
- 正确区分支具内、脱支具和未知状态；
- Observation 保留页码、图片区域或结构化字段来源；
- 歧义只触发最小人工确认；
- 每例生成可解释的 CohortRow；
- 报告声明能够反查 Observation 和原始来源；
- 单病例不产生因果疗效结论。

### P0-C：Galen 内部完整验收

目标：用户不打开文件管理器、浏览器、Word、Excel 或独立 PDF 阅读器，也能完成主要交付验收。

按顺序补齐并测试：患者时间轴、病例队列、数据质量、统计图表、PDF、DOCX、XLSX、证据反查、批注、版本与签核。

### P0-D：可靠性规模化

- 开发 Gate：每个关键案例至少 K=5；
- Release Gate：至少 K=20—30；
- 关键用户旅程持续累计到 K=100；
- 单列长尾失败，不用平均值掩盖；
- 10 例核验病例端到端回归后，再扩展 90 例 OCR 候选与 400 项衍生压力任务；
- OCR 候选不得冒充核验金标准。

## 8. 负优化门禁

任何候选改动必须先通过硬门，再讨论速度或 Token。

无条件拒绝：

- 研究结论与输入或 Evidence 冲突；
- 虚假引用、虚假交付或无法反查来源；
- 上下文压缩丢失任务约束和已接受决定；
- 重启后重复执行完成节点；
- 工作区、患者或任务状态串扰；
- Artifact 缺失、为空或内部预览失败；
- 未核验候选事实进入硬门或主要结论；
- 来源外医学数值、未来信息泄漏或支具状态混淆。

没有硬门失败时，候选还必须满足：

- 综合质量下降不超过 3 个百分点；
- P90 延迟恶化不超过 10%；
- 工具错误率、循环率和恢复失败率不增加；
- TTFR、总耗时或 Token 至少改善 15%，或成功率改善 5 个百分点，或人工干预下降 20%。

连续两轮无法证明收益的复杂改动，应撤销、缩小或降级为实验分支。

## 9. 验证命令

在仓库根目录开始：

```powershell
git status --short

cd rust
cargo check --workspace
cargo test --workspace

cd crates/galen
npm test
npm run build
```

当前 Windows 环境中，完整 workspace 测试历史上出现过与 POSIX shell / Python fixture 假设有关的失败。不得简单忽略：先确认是否与当前改动相关，并在报告中区分新回归与已有跨平台测试问题。

无 UI 闭环探针：

```powershell
cd rust/crates/galen
npm run test:ui-contract
npm run probe:closed-loop -- --model deepseek-v4-flash --timeout 240
```

评测 CLI：

```powershell
cd rust
cargo run -p galen --bin eval -- validate
cargo run -p galen --bin eval -- run --case E07 --model deepseek-v4-flash --repeat 5 --output ../evals/runs/e07-candidate.jsonl
cargo run -p galen --bin eval -- reliability --input ../evals/runs/e07-candidate.jsonl --k 5
```

UI 与预览自动化：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\evals\run_galen_ui_e2e.ps1
```

真实模型运行记录位于 `evals/runs/`，默认不提交 Git。不得在报告、截图、日志或提交中写入 API Key、患者身份信息或未脱敏原始资料。

## 10. 不要重复做的事情

- 不要因为单次慢响应就重写模型路由或 Agent Loop。
- 不要用单次成功或平均延迟宣布优化成立。
- 不要继续降低上下文窗口来追求速度，除非信息保留和长会话门禁同时通过。
- 不要用更长提示词替代确定性状态机、工具契约和宿主校验。
- 不要让科研画布永久占据界面；它只在并行、阻塞、方向调整、追溯和交付缺口时情境化出现。
- 不要把“文件写出来”当成交付完成；必须在 Galen 内可读、可反查、可验收。
- 不要把评测框架的复杂度当成产品价值；评测只服务真实用户闭环和发布决策。
- 不要扩展到其他疾病，直到 AIS 的 10 例真实黄金旅程通过。

## 11. 接手完成的定义

下一位 AI 不需要一次完成整个 P0。一次高质量接手应做到：

1. 阅读本交接文档、现行 PRD、评测规范和最近两份报告；
2. 检查当前 Git 状态并明确本次改动所有权；
3. 只选择 `P0-A` 或 `P0-B` 中一个可验证的最小切片；
4. 修改实现并补充对应的确定性测试或评测断言；
5. 运行与风险相称的验证；
6. 用基线与候选数据说明它是正优化、无显著变化还是负优化；
7. 更新本交接文档中的快照、已完成项和下一阻塞点。

如果只能记住最后一条原则：**证据证明完成，宿主保证可靠，模型负责推理，用户只在真正改变科研结论的地方介入。**
