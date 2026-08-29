# Galen Codebase Recon Report

> 日期：2026-08-28
> 范围：`galen-research-workbench` 全部 Git 历史
> 目的：识别架构复杂度来源，恢复项目控制力

## Repo Vitals

- 历史：2026-08-12 至 2026-08-28
- 提交：31
- 分支：4
- 分析窗口：全部历史
- 贡献者：labops 27 次提交，Drehabwen 4 次提交
- 近三个月活跃贡献者：2/2

仓库历史很短，但变化密度极高。当前复杂度不是多年遗留造成的，而是产品、运行时、评测和领域能力在短时间内同时扩张造成的。

## 1. Code Hotspots

| 排名 | 修改次数 | 文件 |
| ---: | ---: | --- |
| 1 | 14 | `rust/crates/galen/src-tauri/src/backend.rs` |
| 2 | 11 | `rust/crates/galen/src/App.tsx` |
| 3 | 10 | `rust/crates/galen/src-tauri/src/commands.rs` |
| 4 | 9 | `rust/crates/galen/src-tauri/src/lib.rs` |
| 5 | 8 | `rust/crates/galen/src-tauri/src/tools/mod.rs` |
| 6 | 6 | `README.md` |
| 7 | 6 | `rust/crates/galen/src/hooks/useChat.ts` |
| 8 | 6 | `rust/crates/galen/src-tauri/src/bin/driver.rs` |
| 9 | 6 | `rust/crates/runtime/src/session.rs` |
| 10 | 6 | `rust/crates/galen/src/styles/layout.css` |

## 2. Bug Magnets

| 排名 | 修复提交关联次数 | 文件 |
| ---: | ---: | --- |
| 1 | 3 | `rust/crates/galen/src-tauri/src/backend.rs` |
| 2 | 3 | `rust/crates/galen/src/App.tsx` |
| 3 | 2 | `rust/crates/galen/src-tauri/src/bin/driver.rs` |
| 4 | 2 | `rust/crates/galen/src-tauri/src/commands.rs` |
| 5 | 2 | `rust/crates/galen/src-tauri/src/lib.rs` |
| 6 | 2 | `rust/crates/galen/src/components/WelcomeWizard.tsx` |
| 7 | 2 | `rust/crates/galen/src/styles/components.css` |
| 8 | 2 | `rust/crates/runtime/src/session.rs` |

## 3. High-Risk Files

同时出现在热点和 Bug 磁铁中的最高风险文件：

| 文件 | 热点排名 | Bug 排名 | 主要所有者 |
| --- | ---: | ---: | --- |
| `backend.rs` | 1 | 1 | labops（12/14） |
| `App.tsx` | 2 | 2 | labops（10/11） |
| `commands.rs` | 3 | 4 | labops（9/10） |
| `lib.rs` | 4 | 5 | labops |
| `bin/driver.rs` | 8 | 3 | labops |
| `runtime/src/session.rs` | 9 | 8 | labops |

## 4. Bus Factor

- labops：27 次提交
- Drehabwen：4 次提交
- 活跃贡献者：2/2

活跃比例没有异常，但实现知识高度集中在一个开发身份和少数总控文件。真正的风险不是无人维护，而是只有一个人能同时理解 UI、Agent loop、Tauri IPC、上下文和评测。

## 5. Team Momentum

全部 31 次提交都发生在 2026-08。由于历史不足两个月，无法判断上升、稳定或下降趋势。可以确认的是开发速度很高，架构消化速度低于功能进入速度。

## 6. Firefighting Frequency

没有发现包含 revert、hotfix、emergency 或 rollback 的提交。当前问题不是频繁救火，而是复杂度尚未经过长期运行验证。

## 7. Structural Findings

### 前端

- `App.tsx`：746 行，同时协调对话、研究任务、Artifact、模型、工作区、记忆、能力包和多个面板；
- 前端共有 43 个 TypeScript/TSX 文件、20 个组件、13 个 domain 文件和 6 个 hooks；
- 组件已经拆分，但状态所有权仍集中在 `App.tsx`。

### 后端

- `chat_loop.rs`：892 行，承担回合准备、上下文、工具、MCP、事件、停止条件与交付闭环；
- `commands.rs`：787 行，混合 IPC、状态锁、配置、文件和应用服务；
- `tools/mod.rs`：同时承载 Kernel 工具、MCP 和 Pack 组合入口；
- 新 Capability 架构已建立边界，但领域实现仍处于同一 crate，是过渡态而非完成态。

### 评测

- `eval.rs`：2091 行；
- `rag_eval.rs`：648 行；
- `bin/eval.rs`：695 行。

评测规模很大，但相对隔离，不是当前运行时复杂度的第一处理对象。

## 8. Current Architecture

```text
React Workbench
├── App.tsx
├── useChat
├── useResearchTask
└── UI Components
        │ Tauri invoke / events
        ▼
commands.rs
        │
        ▼
Chat Runtime
├── chat_loop.rs
├── context_engine.rs
├── task_contract.rs
├── chat_session.rs
├── conversation_memory.rs
└── ToolRegistry
    ├── Kernel Tools
    └── Capability Packs
        ├── Research
        ├── Rehabilitation
        └── PDF Report
```

## 9. Stabilization Decisions

暂停增加 Pack 市场、动态插件、新 Agent 模式和更多 UI 插槽，进入复杂度回收阶段。

### 第一刀：拆 `App.tsx`

把状态协调移入：

- `useWorkspaceController`
- `useResearchSessionController`
- `useApplicationStatus`

目标：`App.tsx` 降至约 300–350 行。

完成状态（2026-08-28）：`App.tsx` 已从 793 行降至 286 行，并拆出模型配置、研究执行、成果交付、对话上下文、快捷键、工作区选择和顶部应用栏边界。前端构建、9 项前端测试与 126 项 Rust 测试通过。

### 第二刀：拆 `commands.rs`

```text
commands/
├── chat.rs
├── workspace.rs
├── research.rs
├── evidence.rs
├── settings.rs
└── capability.rs
```

命令层只负责参数转换和调用应用服务。

### 第三刀：冻结并分解 `chat_loop.rs`

```text
TurnPreparation
ToolExecution
TurnCompletion
```

先建立阶段边界和独立测试，再考虑继续优化行为。

### 第四刀：Capability 止损线

- Kernel 不依赖领域模块；
- Pack 可以独立测试；
- 关闭 Pack 后工具与上下文同时消失；
- 新增 Pack 不修改 `chat_loop.rs`；
- 达不到这些条件时，不继续动态插件化。

## 10. Stabilization Gate

内部里程碑：`v0.1.2 Architecture Stabilization`

- `App.tsx < 350` 行；
- `commands.rs` 完成模块化；
- `chat_loop.rs` 形成三个阶段；
- 原有产品流程不变；
- 当前测试全部继续通过；
- 暂停引入新的架构名词和扩展机制。

## Recommendation

- Start reading：`backend.rs`、`App.tsx`、`commands.rs`；
- Primary ownership：labops；
- Watch out：高提交密度、总控文件、过渡期双重架构；
- Immediate action：先减少状态和职责集中度，再继续产品扩张。
