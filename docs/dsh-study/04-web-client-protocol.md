# DeepSeek Harness Web 前端 / Client 插件 / 通信协议层研究报告

> 仓库根:`D:\DEV\姿态捕捉\deepseek-harness`。所有路径相对仓库根。核心源码证据来自对 `apps/web`、`packages/client/{web,web-react,runtime,connection,modules,hmr,locale,ui-slots,ui-*}`、`packages/api/{gateway,remotes}`、`packages/typert/*`、`packages/host/{webserver,apiproxy}` 及 `docs/api-gateway.zh.md`、`docs/subsystems/{web,web-server,client-modules,session-projection,typert}.zh.md` 的通读。

## 一、总体架构(先纠正一个常见误解)

DSH 是"两个世界、一个协议":**Host**(Node 进程,浏览器外,跑 agent loop 与全部业务服务)与 **Client**(浏览器内,独立 Cordis 应用 + React UI)。二者不是主从代理,而是"镜像对象层 + 远程调用"关系:Client 侧服务是 Host 服务的状态镜像(由事件帧驱动),写操作走类型化 RPC。

**注意:`packages/web` 不是前端共享代码**——它是"Web 访问能力"(`ctx.web` = search/fetch seam,`packages/web/web` + `tool-web` + `web-search-*` + `web-fetch-http`)。真正的前端壳与共享代码在 `packages/client/web`(shell 内核)、`packages/client/web-react`(React 胶水)、`packages/client/ui-slots`(插槽契约)、`packages/client/runtime`(会话/工作区领域服务)。

## 二、前后端通信模型(connection 包)

`packages/client/connection` 是唯一传输层,分 Host 半与 Client 半:

- **物理传输是"上行 HTTP + 下行 WebSocket"**:浏览器用 `fetch` POST `/api/<method>` 发请求(`packages/client/connection/src/client/rpc.ts`);Host 开两条**只收不发**的 WebSocket downlink `/api/events.mux`、`/api/events.host`(`websocket-downlink.ts`,客户端发消息会被 `close(1008,'downlink only')`)。
- **消息格式:四象限 RPC 判别联合**(`packages/host/apiproxy/src/api/rpc.ts`):
  ```ts
  type RpcMessage = ClientRequest | ServerResponse | ServerRequest | ClientResponse
  // ClientRequest  { type:'client-request', rpcId, method, payload }
  // ServerResponse { type:'server-response', rpcId, result: RpcResult<unknown> }
  // ServerRequest  { type:'server-request',  rpcId, method, payload }  // 下行帧/可应答推送
  // ClientResponse { type:'client-response', rpcId, result }
  type RpcResult<T> = { ok:true; value:T } | { ok:false; error:RpcError }
  ```
  `rpcId` 由发起方铸造、响应回显;`RpcError` 是封闭错误码联合(`RpcErrorDetailsMap`,details 按 code 收窄)。每个方法在 `RpcMethodMap` 里配 Zod schema(`api/rpc-map.ts` + `api/*.schema.ts`),`AbstractApiClient`(`api/fetch/client.ts`)把 schema 化的 `IApiClient` 暴露给浏览器。
- **连接生命周期**:浏览器 `ConnectionController`(`connection/src/client/connection.ts`)把两条流当 `AsyncIterable` pump,每代先做就绪握手(两条流 `onOpen` + 一元 `host.describe` 成功)才置 `connected`,失败按指数退避(500ms→10s,带抖动)重连;`reconnecting` 时清空会话域状态。
- **信任围栏**:所有 `/api` 请求先过 `api-request-trust.ts`(loopback + `trustedHosts`,防 DNS rebinding);`settings/credentials/llm.discoverModels` 等特权方法强制 loopback(`connection/src/index.ts` 的 `PRIVILEGED_METHODS`)。

**数据流**:`ctx.remote.x → connection.rpc.call('/api', '<ns>/<method>', {args}, signal) → POST → trust fence → FetchHandler 分发 → 业务方法 → RpcResult 回传`;Host 事件经两条 WS 下行帧(`MuxFrame`/`HostFrame`,`api/events.ts`)推送。

## 三、API 网关与 Typert 远程调用(api/gateway + api/remotes)

分层:`remotes → gateway → connection → webserver`(`docs/api-gateway.zh.md`)。核心是"**构建期生成类型、运行期严格分发**"的 Typert 机制(`packages/typert/{protocol,generator,registry,loader}`):

- **声明**:业务服务继承 `TypertRemoteService`(基类把 service key 与 wire namespace 绑定,`protocol/src/index.ts`),方法加 `@Remote('create')` 或 `@RemoteScope('agent','current')`。装饰器只把标记写进模块私有 `WeakMap`(SRC 回退用),**真正的类型契约由生成器在构建期产生**。
- **生成器** `packages/typert/generator`:以 Host 侧 `ts.Program` 为种子做严格分析(禁泛型/解构/可选参),产出每个业务包的四个文件:`lib/typert.host.{js,d.ts}`(Host 反射)与 `lib/typert.remote-client.{js,d.ts}`(Host-for-Client 贡献,含 `InvocationDescriptor[]` 与 zod codec + 声明合并 `TypertRemoteMap`/`TypertRemoteScopeMap`,d.ts.map 让编辑器从 Client 调用跳回 Host 源方法)。`InvocationDescriptor`(`protocol/src/types.ts`)含 `namespace/method/invocation(direct|context)/parameters[]/cancellation/result`,是**双侧共享的单一真源**;复杂对象(如 `Agent` 参数)经 `TypertLookupMap` 声明 + `ctx.typert.lookups` 提供方在 Host 侧把 wire id 解析回对象。
- **Host 网关** `packages/api/gateway/src/index.ts`:`TypertGatewayService`(`ctx.typertGateway`)在 `/api` 上 `intercept` 两段式 endpoint(`<ns>/<method>`,只认领有严格描述符或 SRC 标记的端点),`invoke()` 依次做:描述符解析 → `args` 字段集精确校验 → codec 校验 wire 值 → lookup/context 解析 → 调 live Cordis Service → 校验返回值;失败在进业务代码前/出业务代码后即抛 `TypertGatewayError`。
- **Client 网关** `packages/api/gateway/src/client/index.ts`:`ClientRemoteService` 注册为 `ctx.remote`(Cordis Service),`$mount(contribution)` 把生成贡献挂成 `remote.<namespace>` 子服务上的**具体函数**(`Object.defineProperty` getter,无 JS Proxy);`$on/$dispatch` 支撑单向 Host 事件转发。`packages/api/remotes/src/client/index.ts` 显式选装 5 个命名空间:commands/goals/dynamicRunner/pluginInventory/messageFeedback。
- **两个命名空间**:直接调用 `ctx.remote.<ns>.<method>(...)`;作用域调用 `agentCtx.remote.<ns>.<method>(...)`。`AgentContext` 定义于 `packages/client/runtime/src/client/agents/scope.ts`:
  ```ts
  type AgentContext = Omit<Context,'remote'> & { remote: TypertClientRemote & TypertRemoteScopeApi<'agent'> }
  ```
  每个 session 一个 scope fiber(打 `SessionId` 标签),`ctx.typert.contexts.registerClient('agent',{identity})` 从调用方 ctx 读出 id,省略掉那个 lookup 参数。

## 四、Client 插件体系与 UI 组合

**一个 client 插件的构成**(以 `ui-subagent` 为例):
- `package.json` 声明 `dsh.client: { platform:'web', inject:[...] }` 并导出 `exports["./client"]`;
- `src/client/index.ts` 导出 `apply(ctx)`(Cordis 插件体),用 `ctx.slots.inject(key, () => ctx.slots.register({name, key/id, order, locale}, Component))` 注册 UI,`ctx.conversationEvents.register(definition)` 注册会话节点,`ctx.locale.register(ns, {zh, en})` 注册语言包(语言包**本地打包**,偏好持久化走 Host settings);
- 打包用 `packages/client/tsdown.client.ts` 的 `clientBundle(id, entries)`:产出 `lib/client.js` 为闭包工厂(`window.__ModuleLoader__.load({id, factory})`,CSS Modules 内联注入 `<style data-plugin>`,另有"bundle purity gate"禁止跨插件值导入)。

**模块系统 = 浏览器端的懒加载 CJS 表**(`packages/client/modules/src/client/`):执行 bundle 只注册 factory,副作用在**物化**(首次 require)时才跑;`ClientModuleSystem` 是 vendored cordis Loader 的 `internal` 实现,loader 只负责 fiber 生命周期。

**UI 组合 = 声明合并 + 插槽(SlotMap)**:`packages/client/ui-slots` 定义空 `SlotMap`/`LocaleNamespaceMap`,各插件 `declare module` 合并自己的槽契约与词典键;`ctx.slots.register` 一次调用携带组件 + 子槽声明 + store 座位 + 注入面。槽有四种 `SlotKind`(single/list/keyed/chain)与三种 `SlotScope`(root/session-maybe/session)。根布局 `ui-layout/AppFrame.tsx` 只做 `renderSlot('sidebar'|'conversation'|'details'|'shell.overlay', ...)`,整棵树由插件填充。

**keyed renderer 的典型例子**(`ui-conversation/src/client/chat/register-node-renderers.ts`):
```ts
ctx.slots.inject('conversation.chat.node', () => ctx.slots.register(
  { name:'conversation.chat.node', key:'tool-call', locale: NS }, ToolCallNodeView))
```
即"一个槽位 + 按 `key`(节点 kind)分派组件"——`ChatNodeSeat` 按节点 kind 渲染对应组件,第三方可注入新 key。

**会话日志 → UI 的完整链路**(`runtime/src/client/`):
```
WS mux 帧 'session/event' → SessionManager.handleMuxEnvelope → session.acceptLiveEvent
  → ConversationNodeAssembler(按 seq 合并 match)
  → 每个 ConversationNodeDefinition: match(event)→{id,role} / start / update / buildViewNode
  → chat 快照 → useSession(selector) → React
```
`ConversationNodeDefinition`(`contract/conversation.ts`):`match` 从事件提取稳定业务身份,`start/update` 纯函数折叠状态,`buildViewNode` 产出 `ConversationViewNode{kind,id,target,data}`;引擎按 Location(turn/step) 增量重建,`replace/apply` 差异更新快照。历史窗口由 `session.history` 分页回填,`ToolEventView` 等渲染意图由 Host 随事件帧附带(`MuxFrame['session/event'].view`)。

## 五、HMR(`packages/client/hmr`)

不依赖 Vite `hot.accept`,是**整 bundle 热替换**:
- Node 半(`src/index.ts`):一个 interval 对 boot 图内每个 bundle 做 stat 轮询,rev 变化 → `ctx.clientModules.rebuilt(id)`;通过 SSE 通道 `GET /plugins/events` 广播 `{type:'graph'|'rebuilt', id, rev}` 帧;
- 浏览器半(`src/client/index.ts`):`EventSource` 收帧,按序执行 `invalidate → prefetch(旧 fiber 仍在服务)→ registry.delete(先删 runtime 记录,避免 Loader 自毁分支)→ 排干旧 fiber → 移除 <style data-plugin> → entry.refresh() 重新物化 → fiber.await()`;
- **级联零成本**:cordis fiber 的激活 epoch 绑定 provider 的 uid,替换 provider fiber 自动重挂所有依赖方;失败无回滚,留 FAILED 状态可见。

## 六、apps/web 的 boot 流程(`window.__DSH_BOOT__`)

`apps/web` 只是薄入口:`main.ts` 十行,`new AppWebEntry(el).run()`;**不是独立应用**(vite.config.ts 的 `rejectStandaloneServe` 插件在 `serve` 下直接抛错,必须 `pnpm dsh web` 提供注入)。完整链路:

1. **注入**:Host 的 `ClientModuleRegistry`(`packages/client/modules/src/index.ts`,`ctx.clientModules`)增量扫描 Loader 中声明 `dsh.client` 的包,组合出 `WebBootGraph{rev, entries: WebBootEntry[]}`(`WebBootEntry = {id, url:'/plugins/<id>/client.js?rev=<rev>', rev, inject?, immediately?}`),经 `ctx.webServer.tapIndex(injectBootManifest)` 在**每次服务 index.html 时**把 `<script>window.__DSH_BOOT__ = {...}</script>`(转义 `<`)插入 `<head>` 首位——注入发生在服务时而非构建时。
2. **两阶段 boot**(`packages/client/web/src/boot.tsx` 的 `AppWebEntry.run()`):
   - 模块面:解析 `__DSH_BOOT__` → 建 `ClientModuleSystem`(含静态种子 `seed.ts`/`platform.ts`,注册 app-shell 伪条目)→ 渲染加载页 → 并行预取 `immediately` 层;
   - 插件面:`new Context()` → 挂 vendored `cordis-plugin-loader` → `loader.internal = modules`(把模块表作为 import 契约)→ 为 `[modules, ...图行, app-shell]` 逐行 `loader.create` → `loader.await()` → 全 fiber 必须 ACTIVE(否则聚合 fail-loud)→ `settled` 信号一次性切到真实 UI。
3. `app-shell`(`app-shell.ts`)装 `ctx.slots.install(createSlotRenderer())` 并 `buildRenderApp({ctx})`,`AppRoot` 经 `renderSlot('root')` 渲染整棵树;`node-module-stub.ts` 是 `node:module` 的浏览器替代(vite alias 指向它,`createRequire` 永远抛错作哨兵)。

## 七、关键设计亮点

1. **类型安全的远程调用**:一份 `InvocationDescriptor` 双侧消费,同一 zod schema 在 Host/Client 两侧边界互验;生成 d.ts.map 让 IDE 从 Client 调用直接跳到 Host 实现;Client 用具体函数而非 Proxy,卸载即收回方法并中止在途调用。
2. **"镜像对象层"而非远程代理**:Client service 状态由帧驱动(全量值、last-wins),写走 typed RPC,重连靠 `session/subscribed` 等基线帧收敛——天然支持断线重连与多标签页。
3. **严格构建流水线**:Host tsc → tsdown(host,跑 generator)→ Client tsc → tsdown(client),双 build face(`DSH_BUILD_FACE`)保证"先有约定、后有消费";源码运行有 SRC 弱回退兜底。
4. **Shell 自足原则**:加载页与模块系统本身不依赖任何插件 bundle,插件全挂时页面仍能显示失败报告;bundle purity gate + 惰性 CJS 把跨插件耦合压缩到"只能通过 Cordis 服务/槽协作"。
5. **会话投影(session-projection)**:Host 把每个事件的纯折叠结果以全量值推送(`session/projection` 帧),客户端"higher-seq-wins"落库,UI 拿成品值、从不自己折叠事件。
6. **传输分层**:HTTP/WS 只是 carrier,`RpcMethodMap`+schema 与 `Connection` 解耦——换 carrier 不动 Remote 描述符与业务代码。

**核心文件索引**:`apps/web/src/main.ts`、`packages/client/web/src/boot.tsx`、`packages/client/modules/src/{index.ts,client/manifest.ts,client/system.ts}`、`packages/client/connection/src/{index.ts,rpc.ts,websocket-downlink.ts,client/connection.ts,client/rpc.ts}`、`packages/host/apiproxy/src/api/{rpc.ts,events.ts,rpc-map.ts}`、`packages/api/gateway/src/{index.ts,client/index.ts}`、`packages/api/remotes/src/client/index.ts`、`packages/typert/protocol/src/{index.ts,types.ts}`、`packages/typert/generator/src/emitter.ts`、`packages/client/runtime/src/client/{index.ts,agents/scope.ts,contract/conversation.ts,sessions/*}`、`packages/client/ui-slots/src/{index.ts,renderer.ts}`、`packages/client/hmr/src/{index.ts,client/index.ts}`、`packages/client/tsdown.client.ts`、`docs/api-gateway.zh.md`、`docs/subsystems/{client-modules,web,web-server,session-projection}.zh.md`。

---

## 附录:补充调研增量(精确实现细节)

> 收编自 api/gateway+remotes 深挖,与上文主报告结论一致,以下为更精确的落地细节。

### A1. Remote 命名空间 vs legacy ApiProxy 的实际边界

**session / workspace / llm 目前不是 Remote namespace**——它们仍走 legacy ApiProxy 的一元路由 `session.* / workspace.* / llm.*`(`packages/host/apiproxy/src/fetch/handler.ts` 的 `UNARY_ROUTES`)。

真正被 `api/remotes` Client assembly 挂载的 **5 个 Remote 命名空间**及对应业务服务:

| namespace | 业务服务 | 源文件 |
|---|---|---|
| `goals` | GoalService | `packages/goal/goal/src/index.ts` |
| `commands` | CommandRuntime | `packages/interaction/commands/src/index.ts` |
| `dynamicCordisRunner` | DynamicCordisRunnerService | `packages/extensions/cordis-host-runner/src/index.ts` |
| `pluginInventory` | PluginInventoryGateway | `packages/host/plugin-inventory/src/index.ts` |
| `messageFeedback` | MessageFeedbackService | `packages/feedback/message-feedback/src/index.ts` |

### A2. Host 侧 Typert 注册链

`packages/typert/loader/src/index.ts`(typert-loader)监听 loader entry mount → import 业务包 `./typert` 产物 → `validateTypertManifest` → `ctx.typert.register(TYPERT)`(`packages/typert/registry/src/service.ts`),卸载时 withdraw;Gateway 每次调用**实时查** `ctx.typert.local`(不缓存注册结果)。

### A3. 事件转发 allowlist

`API_REMOTE_FORWARDED_EVENTS`(`packages/api/remotes/src/remote-events.ts`)共 **11 个** Host cordis 事件:`agent-preset/selected`、`commands/change`、`credentials/updated`、`cordis/*`、`llm/adapters-updated`、`settings/document-updated`,经 `host/remote-event` 下行帧 → Client `ctx.remote.$dispatch` → `$on`。

### A4. 命名澄清(仓库中不存在这些字面类型)

| 误以为存在 | 实际对应物 |
|---|---|
| `GatewayOptions` | `TypertGatewayBindingOptions{namespace?}` |
| `RemoteOptions` | `ApiRemoteAgentOptions{agentOptions?, setup?}` |
| `TypedRemote` | `TypertClientRemote`(= $mount/$on/$dispatch + 生成命名空间) |

另:`TypertGatewayErrorCode` 共 **17 个**稳定错误码。

### A5. 双向校验同一 schema

生成的 zod schema **同时存在于** Host `typert.host.js` 与 Client `typert.remote-client.js`,两侧各自在边界 parse(Client 校验出参、Host 校验入参与返回值);`src-json` 弱 codec 仅强制 JSON-safe。**类型安全依赖构建顺序**(Host 先生成、Client 后编译),运行时不做 TS 分析(除 SRC 弱解析)。
