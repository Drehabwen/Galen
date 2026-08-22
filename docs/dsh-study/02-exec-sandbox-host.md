# DSH 执行、沙箱与宿主层研究报告

> 范围:packages/{sandbox,subprocess,shell,terminal,code-runtime,fs,host,e2b} 与 docs/subsystems 对应文档。DSH 的核心组织方式是 **capability seam(能力缝)**:每个能力拆成三层——Service Definition(抽象服务,注册 `ctx.xxx`)、Service Provider(平台实现,以插件加载)、Consumer(工具层/其他能力)。Cordis 提供依赖注入、事件与生命周期("everything is a plugin")。

## 1. 沙箱设计(packages/sandbox)

- **抽象缝 `ctx.sandbox`**(`packages/sandbox/sandbox/src/index.ts`):唯一方法 `confine(argv: readonly string[], policy: SandboxPolicy): ConfinedArgv`。它只做 argv 包装,不负责 spawn。
- **模式词汇**:`SandboxMode = 'read-only' | 'workspace-write' | 'danger-full-access'`,只管**文件效果**,网络与进程可见性不在词汇表内。`danger-full-access` 不进沙箱(消费者直接跑原 argv);`workspace-write` 允许写 workspace 根 + 后端承诺的临时区。
- **策略按调用携带(per-call)**:`SandboxPolicy` 每次执行解析一次,不是 provider 级固定状态——同一 provider 可同时服务不同边界,审批过的加宽重试就是一次携带更宽策略的新调用。`ctx.sandboxPolicy`(`packages/sandbox/sandbox-policy/src/index.ts`)负责解析:优先级为 **审批过的显式 mode > 会话 `sandbox/mode` 事件 > 部署默认(read-only,安全默认)**;workspaceRoot 取会话不可变 cwd(经 `canonicalPath` 文件系统语义规范化),agentless 调用回退部署根。
- **本地 provider 的 runner 链**(`packages/sandbox/sandbox-local/src/index.ts`):Linux = `bwrap` 优先、`landlock-run` 次之;macOS = `sandbox-exec`(Seatbelt);Windows = **ACL 受限令牌 runner**(`sandbox-windows-acl`)。多候选用**功能探针**(真实跑一次 `true`/`--probe`)仲裁并缓存;唯一候选不探测。无可用后端抛 `SandboxUnavailableError`(`SANDBOX_UNAVAILABLE`),**绝不允许静默无沙箱执行(fail-closed)**。
- **argv 包装形态**:`[runner, ...profileArgs, '--', ...原argv]`。bwrap profile:基础 `--ro-bind / / --dev /dev --proc /proc --die-with-parent`,workspace-write 追加 `--tmpfs /tmp --bind <root> <root>`;landlock profile:只读 `/`,可写 `/dev/null`(+`/tmp`、workspaceRoot);Seatbelt 生成 SBPL `(deny file-write*)` + 子路径放行,且与进程内 fs 围栏共用 `writableRoots` 帮助函数,保证二者永不分叉。
- **landlock-run(native)**(`native/landlock-run/packages/entry/src/main.c`):纯 C11、静态 musl、除 libc 无依赖、直接调 Landlock syscall(create_ruleset/add_rule/restrict_self),**单文件即全部审计面**。流程:按 ABI(最高 5)协商访问位 → `O_PATH` 打开规则路径 → `PR_SET_NO_NEW_PRIVS`(顺带中和 setuid/setgid)→ `restrict_self` → `execvp`。旧 ABI 只报 `partial enforcement` 不拒绝;规则集创建/内核未强制则**失败退出 125 且不 exec**(fail-closed)。`--probe` 在短命进程里真装一套最大规则集来报告 full/partial。
- **结果分类事实**:`ConfinedArgv` 携带该后端专属的 `denialSignatures`(EROFS/EACCES/EPERM 等**方言**,禁止跨后端并集)与 `runnerFailureRules`(exit 码门 + 致命 stderr 签名,先于 denial 判定"runner 没跑起来")。
- Windows ACL runner:workspace 根用派生 SID 授予**常驻 ACE**(跨会话复用缓存),每个 live session 拿**随机私有 temp 目录 + 独立 SID**(provider dispose 时撤销);因 `WRITE_RESTRICTED` 必须保留 Everyone 与 NTFS 硬链接别名,如实报告 `partial`。

## 2. subprocess / shell / terminal / code-runtime 的关系

- **`ctx.subprocess`**(`packages/subprocess/subprocess/src/index.ts`,实现 `subprocess-local`):`resolveExecutable(command)`、`spawn(spec)`、`spawnTerminal(spec)`。spawn spec **全显式、无默认**;环境基座 `scrubbedParentEnv()` 剥掉 `*KEY*|*PASSWORD*|*SECRET*|*TOKEN*` 与全部 `DSH_*` 再合并显式 env;终止是**进程树级** SIGTERM→grace→SIGKILL(Windows `taskkill /T`);收集输出用**字节偏移 reader + spill 文件**(截断时保留完整流)。`spawnTerminal` 是唯一非管道原语,基于 node-pty,拥有控制终端、前台进程组、TERM→KILL 静默。
- **shell**:`ctx.shell`(`packages/shell/shell`)负责 `resolve`(补默认/封顶)/`run`(前台)/`start`(后台)。`bash-sandbox` 把 `['bash','-c',command]` 交给 `ctx.sandbox.confine`,得到包装 argv 后仍走 `bash-local` 的 subprocess 路径;结果附带 `ShellSandboxInfo`(mode/denied/enforcement/runnerFailed)。`tool-bash` 把后台句柄适配进通用 jobs 运行时。
- **terminal**:`ctx.terminals`(`packages/terminal/terminal`)是后端注册表(`registerBackend`)+ 精确 Agent 所有权会话;`terminal-bash` 后端注入 `terminals+sandboxPolicy+subprocess`,**PTY 顶层 shell 的 argv 同样经 `ctx.sandbox.confine` 包裹**(danger-full-access 除外),经 `ctx.subprocess.spawnTerminal` 启动;并设 **sandbox-mode 围栏**:会话存活期间拒绝 `sandbox/mode` 切换。
- **code-runtime**:`ctx.codeRuntime.run(request)` 在 worker-thread 子进程中执行模型程序,注入宿主绑定命名空间,返回值必须无损 JSON;`isolation` 只是诊断标签、非安全声明;失败为独立正交的 `kind` 枚举(exception/timeout/abort/worker-exit/invalid-output/output-limit)。

## 3. fs 与"同一执行世界"

- **`ctx.fs`**(`packages/fs/fs`,实现 `fs-local`):`resolve` 产出不透明 `FsTarget`,`processPath(target)` 返回**子进程可打开的规范绝对路径**、`fileUrl` 返回 file: URI、`contains` 做包含测试——消费方永远不解析 targetKey。
- **共享世界的关键**:subprocess 提供者与 fs 提供者**在同一路径/进程命名空间**运行(spawn cwd、可执行文件、普通进程、终端会话与挂载的 fs 属同一世界)。因此挂载 `fs-e2b` + `subprocess-e2b`(packages/e2b,实验 POC)把执行世界整体放进 E2B 远程 Linux 沙箱后,**Bash、PTY、LSP 消费方无需任何 E2B 专属 fork**——它们只经 `ctx.fs`/`ctx.subprocess` 委托执行世界操作,文件工具与 E2B 支撑的 Bash 进程看到同一个世界。
- **写前观察(read-before-write)**:`writeText`/`editText` 接受版本守卫(`createIfAbsent`/`replaceIfVersion`);`dsh-tool-fs` 派发 `fs/write-intent`、`fs/edit-intent` 单槽瀑布与 `fs/observed` 记录事件,`dsh-fs-observation-policy` 插件(不注册服务)用 WeakMap 记录"已见/缺席"状态并决定守卫——策略通过**事件**而非服务叠加,卸载插件即退回裸写。`fs-sandbox` 按策略围栏拒绝越界写并报 `FS_SANDBOX_DENIED`(区别于宿主内核的 `FS_PERMISSION_DENIED`);`containment.ts` 用词法快速路径 + dev/ino 身份回退识别别名等价根。

## 3.5 数据流示例:一次受限 bash 调用

工具层拿到调用后:`ctx.sandboxPolicy.resolve({session})` 解析出 mode/workspaceRoot(会话 cwd)→ `ctx.shell.resolve(request)` 补齐超时/输出上限 → `bash-sandbox` 将 `['bash','-c',command]` 交给 `ctx.sandbox.confine()` 得到 `[bwrap|landlock-run, ...profile, '--', 'bash','-c',command]` → 该 argv 经 `ctx.subprocess.spawn` 以净化环境启动 → 子进程内核强制文件效果,stderr 产生方言拒绝签名 → 执行器按 `runnerFailureRules`→`denialSignatures` 顺序归类,写入 `ShellRunResult.sandbox` 并渲染 `[exit code: N]` 标记 → 模型若遇拒绝,可按提示用 `sandbox_permissions + justification` 重试,`approveEscalation` 先经 `ctx.approval` 拿到 `allowed-once` 才以更宽策略重新解析并执行。fs 侧对称:写/改前 `fs/*` 事件瀑布按观察状态选守卫,再落到 `writeText`/`editText` 的原子替换。

## 4. 审批/权限与执行管线

- **`ctx.approval`**(`packages/interaction/user-approval`):会话策略 `ask`(默认,走回答者瀑布)/`never`(确定性拒绝);`request()` 在开放 turn 内追加 `approval/asked`+`approval/decided` 审计对,输出封闭的 `ApprovalOutcome`,`'allowed-once'` 是唯一放行,其余一律 fail-closed。
- **沙箱加宽阶梯**(`packages/sandbox/sandbox/src/escalation.ts`):`WIDER_MODES` 定义严格加宽(read-only→workspace-write/danger-full-access;workspace-write→danger-full-access);`sandbox_permissions` 与 `justification` 必须成对;拒绝标记 `[sandbox: file access denied under X mode]` + 提示标记;`approveEscalation` 在**任何执行之前**先过 `ctx.approval`,只有该精确调用获批才重试。
- **permission-presets**(`packages/interaction/permission-presets`):把两个独立旋钮——`sandbox/mode` 与 `approval/policy`——打包成命名预设;默认表:`workspace-write`(workspace-write + ask)与 `danger-full-access`(danger-full-access + never)。它只记录意图并写穿各旋钮的规范 setter,不拥有任何强制。

## 5. host 侧组织(packages/host)

- **webserver**(`host/webserver/src/index.ts`):纯 `node:http` 载体,`ctx.webServer.register/registerUpgrade/registerFallback/tapIndex`。匹配顺序 exact → 最长前缀 → **唯一 fallback 席位**;处理抛错只回 400 绝不杀进程;dispose 时 `closeAllConnections()` 强制关闭 SSE 等长连接。只允许绑 `127.0.0.1`(默认)或 `0.0.0.0`,无 TLS/认证。
- **frontend-static**:占领 fallback 席位服务 SPA dist——越界 403、miss 回退 index.html(200,SPA 路由)、非 GET/HEAD 405、未知扩展 octet-stream;index 响应过 `applyIndexTaps`(注入 boot manifest)。Electron 不走此服务器(file:// + IPC 桥)。
- **apiproxy**(`host/apiproxy/src/api-proxy.ts`):RPC 桥,`RpcRequest<P>`→`RpcResponse<T>` 且回显 `rpcId`,把会话/工具/审批/设置/凭证/workspace 等全部域映射到 wire;域错误收窄为稳定错误码。
- 其余:plugin-inventory(插件清单)、directory-picker(native/auto/browse 变体,win32 原生对话框)。

## 6. 安全模型与设计亮点

**安全边界**:沙箱只约束文件效果,不管网络与进程可见性;`partial` 强制如实上报(旧 Landlock ABI、Windows ACL),消费者不得把 partial 当 full;无可用后端一律 fail-closed;子进程环境净化(凭据 + `DSH_*` 命名空间)、树级终止防孤儿、输出有界 + spill、拒绝标记按后端方言精确匹配。

**设计亮点**:(1) seam 三层拆分使"换后端不改消费者"——挂两个 E2B 适配器即整体迁移执行世界;(2) per-call 策略 + 单处解析(`ctx.sandboxPolicy`),bash/fs/terminal 共用同一策略源;(3) 功能探针仲裁 runner 链,真实执行才可信;(4) landlock-run 单 C 文件即审计面,ABI 协商 + no_new_privs;(5) fs 观察策略以事件瀑布实现,无服务依赖、可整体卸载;(6) 审批与沙箱加宽以结构化标记教会模型自我纠错,而放行永远需要人类/机器审批闸门。
