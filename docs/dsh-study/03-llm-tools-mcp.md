# DSH LLM 模型路由与工具系统研究报告

> 研究对象:DeepSeek Harness(DSH)开源仓库(基于 Cordis 运行时的 "everything is a plugin" agent harness)。本文覆盖 `llm / mcp / credentials / skill / web / tools` 六个能力域,路径均相对仓库根。

## 0. 总览:接缝(Seam)+ 瀑布(Waterfall)

DSH 的所有能力域复用同一套架构模式:

- **服务接缝**:一个抽象 Cordis 服务(`ctx.llm`、`ctx.tools`、`ctx.credentials`、`ctx.skills`、`ctx.web`)+ 若干"服务提供者"插件 + "消费者"插件;
- **瀑布事件**:Cordis `waterfall` 事件可被插件按注册序拦截——监听器可调 `next()` 委派,也可短路/替换返回值。重试、路由、审批、注入等横切能力全部由此实现;
- **可合并扩展**:核心词汇表(消息块、来源、结束原因、模型模态)都是 `interface Map`,插件通过 `declare module` 注册新成员,核心代码 switch 后对未知值走兜底。

## 1. LLM 适配器接缝(`ctx.llm`)

关键文件:`packages/llm/llm/src/types.ts`(词汇表)、`message.ts`(消息值)、`index.ts`(运行时与适配器)、`assembler.ts`(折叠器)。

- **消息词汇表**:`Message = { id, role: 'system'|'user'|'assistant', content: ContentBlock[], source }`,不可变(构造即 deep-freeze)。`ContentBlock` 由 `ContentBlockMap` 派生:`text / reasoning / image / tool-call / tool-result`。`source` 是可合并扩展的联合(`user / plugin / model / tool`),模型消息携带 `provider/model` 与适配器私有的 `replayState`(重放状态,仅当同一适配器实例同时拥有历史与目标 provider 时才透传)。
- **原始流协议 `StreamChunk`**:`block-start / text-delta / reasoning-delta / tool-call-delta / block-end / usage / finish`。适配器只发原始分块,消费者用 `BlockAssembler` 折叠成完整块;工具参数在协议层始终是原始 JSON 字符串。
- **新增模型提供商**:继承抽象类 `LlmAdapter`,唯一必须实现的是 `stream(options): AsyncIterable<StreamChunk>`;可选 `providerInfo / listModels / resolveModel / providerRetryPolicy`。然后 `ctx.llm.registerAdapter(['my-provider'], adapter)` 注册路由(返回可原子替换、可释放的 handle)。参考 `packages/llm/llm-deepseek/src/`:`serialize.ts`(请求序列化)+ `sse.ts`(SSE 解析)+ `translate.ts`(把 OpenAI 兼容 wire 消息翻译成 StreamChunk);连接参数与 API Key 每次请求重新解析,配置/密钥轮换无需重启。
- 附:失败统一规范化为 `LlmFailure { message, code, status?, providerRetryAfterMs?, requestId? }`,HTTP 状态映射为稳定错误码(`AUTH / RATE_LIMIT / CONTEXT_WINDOW_EXCEEDED / SERVER` 等)。

**新增提供商清单**:① 实现 `stream()`,把 wire 流翻译为 StreamChunk(可参考 `translate.ts` 的分块增量翻译);② 实现 `serializeRequest` 把 `GenerateOptions`(system/tools/stop/maxTokens)映射到 provider 字段;③ 可选实现 `listModels/resolveModel` 提供模型目录与精确能力(上下文窗口、默认 maxTokens、reasoning effort 枚举);④ 用 `ctx.llm.registerAdapter([...routes], adapter)` 注册,并在 `cordis.yml` 中以插件行启用。

## 2. `llm/stream` 瀑布:流式、拦截与重试

`ctx.llm.stream()` 最终走 `ctx.waterfall('llm/stream', options, () => adapterStream(...))`(见 `packages/llm/llm/src/index.ts`)。监听器可 `yield` 自己的 chunk 短路请求,或调 `next()` 到达最终适配器;适配器边界把选择/分发/迭代的抛错统一规范化为终态 `finish {kind:'error'|'aborted'}`。

围绕 agent 的完整事件链(`packages/core/agent/src/runtime-types.ts`、`packages/core/agent-loop/src/agent.ts`):

```
agent/pre-step(注入消息/技能目录) → agent/request(替换 provider/model/effort)
→ ctx.llm.prepareCall(解析精确模型元数据) → llm/stream(传输层拦截)
→ 失败时 agent/request-error(重试决策) → agent/turn-stopping
```

重试在 `packages/llm/llm-retry/src/index.ts`:`llm-retry` 监听 `agent/request-error`,按 `ResolvedRetryPolicy`(`mode: normal|always`、`retryableCodes`、指数退避 + jitter,尊重 provider 的 `Retry-After`)决策;重试是 **durable** 的——先把 `llm/retry` 事件写入会话日志再等待,崩溃后可恢复。流拦截的典型用法:监听器调 `next()` 前先记录/改写请求,或**不调 next() 直接 yield 自己的 chunk**(如命中缓存、故障降级、测试桩),从而对上层完全透明地替换一次模型调用。token 计量(`packages/llm/token-meter`, `ctx.tokenMeter`)是"重放感知"服务:重放会话事件还原每次请求的 provider `usage`(缓存读写分桶),并对未观测部分用固定密度启发式(4 字符/token)估算上下文压力。

## 3. 模型路由与预设

- **默认模型**:`packages/core/agent-default-model/src/index.ts` 在 settings 命名空间 `agent-default-model` 存 `{ provider, model, reasoningEffort? }`;`ctx.agentDefaultModel.currentSelection()` 读取。入口(如 `packages/bundle/headless`)创建 agent 时把选择写入 `agentOptions`,再 `installModelSelection(agentCtx, ref)`(`packages/core/agent/src/model-selection.ts`)。
- **选择装配**:`installModelSelection` 装两个 scoped 瀑布——`system-prompt/assemble` 把选中模型注入 `{{provider}}/{{model}}` 变量;`agent/request` 把选择覆写进请求配置(并发切换下一 step 生效)。
- **循环内路由**:`agent-loop` 的 `buildRequest` 以"agent 声明路由或持久化 request/header"为种子 → `agent/request` 瀑布(监听器可整体换 provider/model)→ `ctx.llm.prepareCall`(解析 context window、maxTokens 默认、reasoning effort 合法性)→ 记录 `request/header`(请求可从日志完整重建)→ 冻结 `GenerateOptions` → 流式。
- **预设(preset)**:`packages/preset/agent-presets` 中一个 preset 是一个含 `agent.cordis.yml` 的目录,按 roster 在 **standing scope** 挂载一次;其插件/工具/提示词注册进 preset 层,`dsh-scope` 的父链让 agent 视图按 `agent → preset → global` 解析(最近者胜)。preset 不直接声明模型,但可携带模型选择装配插件来影响路由;子 agent 用 `composeFrom` 绑定父 preset 的同一份组合。

## 4. MCP 客户端(`packages/mcp/mcp-client/src`)

- 插件入口 `index.ts`:每个实例连一个外部 MCP 服务器,支持 `stdio`(spawn 子进程)与 `streamable-http`(SSE)两种传输;`serverName` 进程内唯一预留,连接断线按 `ReconnectConfig` 重连。
- 工具桥 `tools.ts`:发现(分页 `tools/list`)→ 每个工具生成确定性公开名 `mcp__<serverName>__<rawName>`(受 DeepSeek 函数名 64 字符约束,必要时追加 SHA-256 哈希后缀)→ 组装 `ToolDefinition`(parameters 取 MCP inputSchema,execute 用**原始 rawName** 发 `tools/call` 并映射 ContentBlock)→ `ctx.tools.register()`。
- **与本地工具注册表的关系**:完全复用 `ctx.tools` 分层注册表(全局层 + 作用域层,`packages/core/tools/src/index.ts`),不新建注册机制;同步采用"先 fetch 全量、后原子 swap"两阶段,失败回滚整代;模型面还受 `tools/pre-execute / execute / post-execute` 瀑布与 `restrict()` 过滤约束。

## 5. 凭据管理(`ctx.credentials`)

- 接缝 `packages/credentials/credentials/src/index.ts`:`CredentialRef`(POSIX 环境变量名的 branded 类型);抽象方法 `resolve(ref) / describe(ref) / set(ref, value) / unset(ref)`。**配置里只存引用(环境变量名),不存密钥**。
- 本地提供者 `packages/credentials/credentials-local`:分层解析——继承的进程环境(只读、最优先)> `$DSH_HOME/.credentials.yaml`(托管、可写)> `<cwd>/.env` > `$DSH_HOME/.env`;空值一律视为不存在。文件权限强制 0600(POSIX 检查)、跨进程写锁 + 原子写、chokidar 热更新并触发 `credentials/updated` 事件。
- **注入**:消费方每次操作重新 `resolve`(密钥轮换立即生效,无需重启)——`llm-deepseek` 每次请求解析 Bearer,且"端点快照与密钥同一次解析"保证二者永不跨代错配。

## 6. 技能系统(`ctx.skills`)

- 注册表 `packages/skill/skill/src/index.ts`:分层(host + per-scope)`SkillRegistry`,`registerProvider / register / list / snapshot / get(name)`;发现结果带 `complete` 标记(不完整快照不缓存)。
- 本地提供者 `packages/skill/skill-filesystem`:按 rank 扫描项目 `.dsh/skills`(100)> `.agents/skills`(200)> 自定义目录(300)> 用户 `~/.dsh/skills`(400)> `~/.agents/skills`(500)> bundled(600);接受 `<name>/SKILL.md` 目录束或 `<name>.md` 平铺文件。
- **检索与注入**:消费者 `packages/skill/tool-skill` 在首个 `agent/pre-step` 注入持久化的 `<available_skills>` 目录消息(仅名称+描述,绝不带正文/路径),并注册 `skill({ name })` 工具;模型调用该工具时按名加载完整正文(返回 `<skill_content>`/`<skill_instructions>`),且受 `modelInvocable` 策略门控。目录变更以 `skills/change` 失效事件通知。

## 7. Web 模型工具

- 能力接缝 `packages/web/web`(`ctx.web`):`registerSearchProvider / registerFetchProvider` 注册能力(而非工具);执行期选择 provider(配置 id > 唯一可用者,歧义/缺失有明确错误码)。
- 提供者:`web-search-deepseek` 走 Anthropic 兼容 Messages API + 原生 `web_search_20250305` server tool(与 `llm-deepseek` 共享 API Key,不走 `ctx.llm`);另有 `web-search-exa`、`web-search-perplexity`、`web-fetch-http`(请求约束见 `policy.ts`)。
- 消费者 `packages/web/tool-web`:把模型面 `web_search / web_fetch` 工具(名称、schema、限额、展示)注册到 `ctx.tools`,执行委托 `ctx.web`。换搜索后端不影响模型如何提问——能力接缝的典型收益。边界提示:`web-fetch-http` 目前**未实现 SSRF/私网防护**(源码注释明确),仅有协议白名单、大小与超时约束;凭证注入路径也不统一(DeepSeek 走 `ctx.credentials` 热更新,Exa/Perplexity 读启动环境)。

## 8. 设计亮点小结

1. **可合并扩展的词汇表**(ContentBlockMap/MessageSourceMap/FinishReasonMap):核心与插件同树演进,新增模态需适配器/UI/压缩同步支持。
2. **适配器只懂原始分块**:折叠(BlockAssembler)、装配、重放状态均单点归属,协议层保持 provider 中立。
3. **瀑布贯穿全链路**:路由(agent/request)、传输拦截(llm/stream)、工具执行(tools/pre-execute/execute/post-execute)、注入(pre-step)全部可插拔,横切关注点零侵入。
4. **可重建请求**:request/header + 冻结请求,会话日志可完整重建每次模型调用(含适配器默认值标注)。
5. **密钥零配置化**:配置只存引用、逐操作解析、层级叠加,密钥永不进日志/UI。
6. **能力接缝模式**(Service Definition / Provider / Consumer)统一贯穿 llm、tools、web、skill——注册与消费解耦,是"everything is a plugin"的直接体现。
