# Galen 技术债务审计

日期：2026-08-16

范围：`rust/crates/galen`、与 Galen 直接相连的 runtime 路径、Git 历史与 CI。

## 仓库概况

- 历史：2026-08-12 至 2026-08-14。
- 提交：23。
- 分支：4。
- 贡献者：2；其中 `labops` 完成 22/23 个提交。
- 没有发现 revert、hotfix、emergency 或 rollback 提交。

历史很短，因此热点代表近期集中修改，不足以判断长期趋势。

## 高风险热点

同时进入改动热点与修复提交热点的文件：

1. `rust/crates/galen/src-tauri/src/backend.rs`
2. `rust/crates/galen/src/App.tsx`
3. `rust/crates/galen/src-tauri/src/commands.rs`
4. `rust/crates/galen/src-tauri/src/lib.rs`
5. `rust/crates/runtime/src/session.rs`
6. `rust/crates/galen/src/components/WelcomeWizard.tsx`
7. `rust/crates/galen/src-tauri/src/bin/driver.rs`
8. `rust/crates/galen/src-tauri/src/modes.rs`

## 债务清单

### P0：权威研究状态分叉

状态：**已于本次修复**。

此前 UI 将任务写入 `.galen/tasks/<task-id>/task.json`，模型上下文和恢复协议仍读取根目录 `plan.json`，证据仍写入根目录 `evidence.json`。

修复后：

- 模型进度摘要和恢复协议读取活动 `ResearchTask`。
- 旧 `plan.json` 仅作为一次性迁移源。
- 证据写入活动任务目录的 `evidence.jsonl`。
- 旧 `evidence.json` 仅作为一次性迁移源。
- 旧源文件迁移后保留，便于恢复和核对。
- 旧 `save_plan` / `load_plan` IPC 已取消注册和移除。

### P0：工作区路径边界

状态：**已于本次修复**。

Tauri 目录浏览、文件读取、Artifact 预览和工具层已经共享同一个 `WorkspacePath` 边界，并加入绝对路径、父目录逃逸、同前缀兄弟目录和工作区内新文件测试。

### P0：前端无自动化测试

状态：**测试基线已建立，关键交互覆盖仍待补齐**。

已接入 Vitest，并覆盖计划解析、依赖关系、默认自主节点状态和任务契约提示。仍需继续覆盖首次配置、工作区恢复、任务保存顺序、损坏状态恢复、Artifact 内部打开和情境化画布路由。

### P1：前端主组件职责过多

状态：**已完成第一步拆分**。

任务恢复、创建、CAS 保存队列、冲突回载、Evidence 版本同步和旧节点规范化已经抽入 `useResearchTask`。`App.tsx` 仍承担模型配置、工作区、Chat、节点调度、Artifact 预览和界面路由，后续继续拆为 `useWorkspace`、`useResearchRunner`、`useArtifactPreview` 和 `AdaptiveWorkbenchRouter`。

### P1：后端运行循环过度集中

`backend.rs` 超过 1200 行，`run_chat` 混合 Prompt、上下文、模型路由、MCP、工具、Token、收敛和事件发送。应逐步拆为 `ContextBuilder → ModelRequest → ToolLoop → EvidenceCollector → ConvergencePolicy → RunLedger`。

### P1：MCP 子进程生命周期

状态：**已于本次修复**。

MCP 子进程现在启用 `kill_on_drop`；存在 Tokio Runtime 时异步终止并回收，没有 Runtime 时执行同步终止与非阻塞状态检查，不再创建后立即丢弃 `child.wait()` Future。

### P1：任务并发版本控制

状态：**已于本次修复**。

ResearchTask Schema v2 已加入单调递增 revision、宿主存储锁和 CAS。节点保存必须提交 `expectedRevision`；陈旧写入被拒绝且不会覆盖当前任务。Evidence 登记也推进同一 revision，前端队列会接收新快照，冲突时重新加载宿主权威状态。

### P1：新 PRD 与旧控制流并存

- 科研画布仍常驻右侧。
- 计划解析仍生成 `pending_approval`。
- Welcome Wizard 仍是阻塞式工作面。
- Artifact 尚未形成统一 Viewer 路由。
- Research Design Compilation 尚未成为宿主对象。

### P2：工程卫生

- 全工作区 `cargo fmt --all --check` 当前不能通过。
- Galen 的 Clippy 扫描现有 12 个警告；本次已消除 MCP Future 和旧路径解析器相关警告。
- CI 有 Rust fmt/test/clippy，但没有前端 TypeScript 构建和测试。
- 本地研究产物、探针输出和 Windows 特殊文件需要更明确的忽略与清理策略。

## 推荐顺序

1. 扩充首次配置、工作区切换和 Artifact 的前端交互测试。
2. 继续拆出 `useResearchRunner` 与 `useArtifactPreview`。
3. 实现情境化工作区与 Artifact Viewer。
4. 在统一状态内实现 Research Design Compilation。
