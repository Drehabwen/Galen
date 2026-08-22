# DSH 核心会话与代理循环研究报告

仓库:`D:\DEV\姿态捕捉\deepseek-harness`(以下路径相对仓库根)。

## 1. 整体骨架

六个 core 包构成一条循环主干(turn flow 见 `docs/architecture.md`):

| 包 | 职责 | ctx 键 |
|---|---|---|
| `packages/core/session` | 追加式 SessionEvent 日志 + 内存 store | `ctx.sessions` |
| `packages/core/system-prompt` | 提示词分区与工具 schema 组装 | `ctx.systemPrompt` |
| `packages/core/tools` | 作用域化工具注册表与守卫管线 | `ctx.tools` |
| `packages/core/agent` | Agent 接口、live registry、`agent/*` 事件 | `ctx.agents` |
| `packages/core/agent-loop` | Agent 接口的唯一默认实现(驱动循环) | `ctx.agentLoop` |
| `packages/core/scope` | 每代理作用域注册原语(纯库,无 ctx 键) | — |

语义:**turn = 零或多个 step;step = 一次模型调用 + 它请求的工具执行**。每次请求的历史 100% 从会话日志 derive,模型可见的一切先落日志。

## 2. SessionEvent 数据模型(`packages/core/session/src/types.ts`)

`SessionEventMap` 是 merge-extensible 接口(插件用 `declare module` 合并扩展),核心 13 种事件:`turn/start`、`turn/end`、`step/start`、`step/end`、`user/message`、`assistant/chunk`、`assistant/message`、`tool/call`、`tool/result`、`todo/write`、`request/header`、`request/context`、`session/end-seed`;插件再叠加 `agent/inbox/spliced`、`compaction/*`、`hook/*`、`llm/retry` 等(`session/src/known-event-types.ts` 列出全仓库 43 种)。

`SessionEvent` 是**对 type 的判别联合**(switch 自动收窄),`seq = log.length` 单调连续,`time` 为 epoch ms。`surfaceOp`/`sourceEventSeqs` 仅在 `SurfaceEventType`(`user/message`/`assistant/message`/`tool/result`)上条件存在——编译期强制。

**surface 投影**:三种消息事件必须声明 `surfaceOp`(`'append'` 或 `{op:'replace',start,end}`),构成模型可见的有序表面;compaction 用 replace 影藏旧节点,`SessionSurface.replaceGeneration` 标记重写代次。`deriveMessages()` 按表面节点折叠:每节点首次见到时投影一次并缓存,表面重写时重建;返回新数组但消息对象共享且深冻结。

**"模型可见即已记录"**:agent-loop 的 `buildRequest` 中 system/tools 来自 `ctx.systemPrompt.assemble`,messages 来自 `session.deriveMessages()`;`assistant/message` 携带 `sourceEventSeqs` 指向组装它的 chunk;`request/header` 事件记录完整 `EpochHeader`(config+system+tools),使**每个请求都是日志的纯函数**——因此任何新的模型可见输入都必须新增事件类型并从日志渲染,这是架构.md 明示的不变量。`Session.append()` 在提交点做 lossless-JSON 校验与深冻结,坏数据源头失败;提交后 fire-and-forget 广播 `session/event`,热路径不阻塞 I/O(持久化插件异步缓冲,`session/flush` 为显式耐久屏障)。`session/src/invariant.ts` 校验 turn/step 编号、执行事件必须封在 open turn 内、`tool/call`→`tool/result` 同 step 配对。

## 3. Agent 接口与 agent-loop

`Agent`(公开面,`packages/core/agent/src/types.ts`):`id: SessionId`、`options`、`session`、`inbox`、`status: 'idle'|'running'`、`ctx`;方法 `send(message, target: InboxTarget, wakeup)`、`followup`/`steer`/`inject`(按 `next-turn`/`next-step` 入队并可选择唤醒)、`cancel(cause, {keepInbox})`、`whenIdle()`、`runMaintenance()`。

`ReactLoopAgent`(`packages/core/agent-loop/src/agent.ts`)是唯一实现:阶段机 idle/maintenance/running,`kick()` 循环调 `turn()`。turn 流程:`turn/start` → claim inbox(`next-step` 批 + 一个 `next-turn`)→ `systemPrompt.assemble` + 动态上下文投影 → `agent/pre-step` 瀑布(reject 或改写 messages)→ `step/start` + 逐条 append `user/message` → `step()`:`agent/request` 瀑布(冻结 call config)→ `llm.prepareCall` → append `request/header`(reason: initial/resume/change,变了才记)+ `request/context` → `llm/stream` 逐 chunk append `assistant/chunk` → 组装 `assistant/message`(带 usage 与 chunk 的 sourceEventSeqs)→ 按 `executionMode` 分类并行/独占执行工具 → 循环直到无 tool-call 或 max-tokens → finally append `turn/end`。`TurnEndReason` 六元:`completed/aborted/blocked/error/max-tokens/interrupted`(interrupted 仅崩溃恢复合成;max-tokens 粘性优先于 completed)。失败统一结构化:不再重试即抛,`agent/error` 现场报告。

`ctx.agents` 注册表:`setFactory`(agent-loop 构造时注册,消费方不依赖具体循环包)、`create/resume`、`register/enter/announce`(prepare→enter→announce 三段式带回滚的发布)、`get/list/roots/isOwnedBy`;initiator 用 `AsyncLocalStorage` 提供进程内因果归属(`currentInitiator`/`requireInitiator`/`withInitiator`/`withoutInitiator`)。

## 4. 工具注册与执行管线(`packages/core/tools/`)

`ToolDefinition` = `ToolSchema`(模型可见字段)+ `output`(schema+render+presentationMeta)+ `execute(args, exec: ToolRunContext)` + 可选 `finalizeContent`/`timeoutMs`/`isConcurrencySafe`/`presentCall`/`presentResult`;`schemas()` 用白名单只暴露 name/description/parameters,回调绝不漏到线上。

`ctx.tools.execute()` 管线(参数只物化一次、深冻结):

```
tools/pre-execute 瀑布(allow/deny/ask;ask 走 ctx.approval 一次性批准)
  → 单调守卫 guard(返回 string 即拒绝;无 allow 结果,顺序无法翻案)
  → tools/execute 环绕瀑布(超时/重试/指标;可替换 signal 不可删除)
  → 工具体 execute()
  → tools/post-execute 瀑布(accept / replace value|content / block→isError 反馈)
  → ToolDefinition.finalizeContent(同步内容末道不变式)
  → 物化冻结 → tools/result(终态广播,观察者不可变更)
```

全程异常收敛为 isError 结果:未知工具、拒绝、抛错都不断 turn。守卫与 pre-execute 都注册在 `agent.ctx` 上即只对该 agent 生效(见下节)。

## 5. scope 原语(`packages/core/scope/`)

`ScopeKey` 是不透明对象身份——循环直接拿 **Agent 实例本身当 key**。`createScope(ctx, key)` 铸造一个 Cordis fiber,经它注册的 effect 随 fiber 撤销;`scopeOf(ctx)` 读最近作用域标签;`scopeTarget(base, key)` 造只路由的 `Scoped<T>` 载体:未打标签监听者全局可见,打标签的按 key 或其祖先链(`bindScopeParent` 建立,含环检测)放行——**事件沿父链向上流动**。`ScopedLayers`(store.ts)持全局层 + 惰性逐 scope 层:注册视图沿父链**向下继承**(子 agent 见祖先的工具),`merge()` 全局在前、最近 scope 最后覆盖;层空即回收。一个 ctx 同时决定可见性与生命周期所有权,这就是"每个 agent 有独立能力集"的机制。

## 6. system-prompt 组装(`packages/core/system-prompt/`)

`PromptSection{name, order, text(静态或按 AssembleContext 求值), complete?}`:order 升序拼接(约定 -100 框架身份、0 persona、100-199 工具指引);`complete` 节可独占全文。`PromptContext` 为动态上下文(以 user-role 快照入日志)。`assemble(context{scope, signal})` = 收集全局+链上作用域 provider → 规范排序 → `system-prompt/assemble` 瀑布 → 强制 complete 节;`renderPrompt` 插值 `{{variables}}`。工具 schema 经 `tools(provider)` 汇入同一次组装。

## 7. 事件域划分(architecture.md Events 节)

- **session 事件** = 耐久事实,追加进日志并经 `session/event` 广播,可重放重建;
- **agent 事件** = 在途控制(inbox 增删/claimed、`agent/status`、`agent/error`,以及 pre-step/request/request-error 瀑布);
- **capability 事件** = 接缝策略(`fs/*`、`tools/*`、`telemetry/*`),不依赖循环即可挂载。

关键取舍:turn/step 边界是 **session 事件**(耐久)而非 agent 事件——边界可重放,控制易失。

## 8. 设计亮点

1. **Map→派生联合 + declaration merging**:插件零改动扩事件/触发原因类型(`core.md` 统一文档化);
2. **模型可见即已记录**:日志可重建一切请求,deriveMessages 缓存 + 深冻结;
3. **瀑布 `next()` 惯例 + 单调守卫**:拦截可叠加、守卫只减权不增权;
4. **注册上下文即作用域即所有权**:一个 ctx 决定可见性 + 生命周期,`enter/announce` 带回滚的发布;
5. **append 时校验 + 深冻结 + seq 连续**:坏数据源头失败,持久化可原样存日志(连 chunk 都不丢);
6. **`ignorable` 标记**:未知事件要么显式可跳过、要么拒绝重建,防静默丢数据(向前兼容的读端护栏);
7. 品牌化 ID、AsyncLocalStorage initiator、prepare/enter/announce 三段事务。

## 9. 如何扩展

- **加一个工具**:新插件 `defineTool` + `ctx.tools.register()`;schema 自动进 assemble。只给某 agent 用→在 `agent.ctx` 里注册。
- **加一个会话事件**:包内 `declare module '@deepseek-ai/dsh-session/types' { interface SessionEventMap {...} }`;若需模型可见→作为 SurfaceEventType 并在 `surface.ts` 的 `deriveEventMessage` 加投影规则、append 时传 `SurfaceIntent`;关系约束写在自家 invariant 伴生插件(仿 `session/src/invariant.ts`);仓库内事件会被脚本收进 `known-event-types.ts`。
- **拦截一次请求/工具**:挂 `agent/pre-step`、`agent/request`、`tools/pre-execute`/`post-execute` 瀑布。
- **换掉默认驱动**:实现 `Agent` 接口,`ctx.agents.setFactory()` 注册,消费方只依赖 `agent` 包不依赖 `agent-loop`。

## 关键文件速查

- `packages/core/session/src/types.ts`(SessionEventMap/SessionEvent)、`index.ts`(SessionStore)、`surface.ts`(deriveMessages 投影)、`invariant.ts`
- `packages/core/agent/src/types.ts`(Agent)、`index.ts`(AgentRegistry+initiator)、`runtime-types.ts`(agent/* 事件)
- `packages/core/agent-loop/src/agent.ts`(ReactLoopAgent 驱动)、`tool-calls.ts`(工具调度)
- `packages/core/scope/src/index.ts`(createScope/scopeOf/scopeTarget)、`store.ts`(ScopedLayers)
- `packages/core/system-prompt/src/index.ts`(SystemPrompt)
- `packages/core/tools/src/index.ts`(ToolRuntime 管线)、`schema.ts`(schema DSL)、`presentation.ts`
- 文档:`docs/subsystems/core.md`、`session.md`、`tools.md`、`scope.md`、`system-prompt.md`、`docs/architecture.md`、`docs/agent-lifecycle.md`、`docs/tool-execution-pipeline.md`
