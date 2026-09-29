# Galen 项目交接与贡献指南

> 本文是新维护者的第一入口。目标不是一次读懂全部代码，而是在不依赖原作者口头解释的情况下，完成：克隆、启动、验证、修改、提交 PR 和发布。

## 1. 项目使命

Galen 是康复科研编排与证据计算节点；[RehabMain](https://github.com/Drehabwen/Rehab) 是真实世界康复数据与现场执行节点。两者通过连接器形成：

```text
真实评估 → 数据治理 → 科研分析 → 行动反馈 → 新评估
```

Galen 的长期价值不在于复刻通用聊天 Agent，而在于把真实康复任务、可追溯证据、科研执行工具、连接器和评测体系组织成可复现的闭环。

### 不应偏离的原则

1. 不用功能数量代替真实科研价值。
2. 不让无法追溯来源的检索结果进入正式研究产物。
3. 不用“编译成功”代替真实用户路径验证。
4. 新能力优先通过稳定扩展接口接入，不继续膨胀主组件和核心循环。
5. 患者数据默认本地优先、最小披露、可审计。

## 2. 当前所有权与协作方式

- GitHub 仓库：<https://github.com/Drehabwen/Galen>
- 当前默认开发分支：`galen-research-workbench`
- 日常维护者：处理 Issue、短分支、PR、测试和版本发布。
- 项目发起人：保留战略方向、数据边界和重大架构决策权。

不要共享个人 GitHub 密码或模型 API Key。仓库权限通过 GitHub Collaborator / Organization 管理；密钥通过本地配置或 GitHub Actions Secrets 管理。

## 3. 从全新克隆开始

### 环境要求

- Git
- Node.js 20
- Rust stable；Windows 需要 Visual Studio 2022 Build Tools 的“使用 C++ 的桌面开发”
- Python 3
- Windows WebView2

### 克隆与准备

```bash
git clone -b galen-research-workbench https://github.com/Drehabwen/Galen.git
cd Galen

python rust/scripts/download_sidecars.py

cd rust/crates/galen
npm ci
```

### 首次验证

```bash
cd rust/crates/galen
npm test
npm run build

cd ../..
cargo test --workspace
cargo clippy --workspace
```

### 启动桌面应用

```bash
cd rust/crates/galen
npm run tauri dev
```

模型配置、平台差异和常见问题见 [开发者接入指南](docs/DEVELOPER_ONBOARDING.md)。

## 4. 代码地图

```text
Galen/
├── rust/
│   ├── crates/galen/          # Tauri 桌面应用、React UI、应用命令
│   ├── crates/runtime/        # Agent 执行运行时
│   ├── crates/model-router/   # 模型供应商与路由
│   ├── crates/medical-core/   # 医学检索与领域能力
│   └── crates/api/            # 跨模块公共协议
├── scripts/evals/             # 评测运行器、预检与评分
├── evals/                     # 评测合同、基线和报告
├── docs/                      # 架构、产品、复盘和研究文档
└── .github/workflows/         # CI、Windows/macOS 构建与发布
```

开始修改前，先判断代码属于哪一层。UI 不应直接实现连接器协议、模型路由或科研治理规则；Rust 核心也不应包含特定页面状态。

## 5. 稳定扩展面

后续功能优先落在五类扩展面：

```text
Galen Kernel
├── Connector API       # 真实世界数据与反馈行动
├── Tool API            # 检索、文件、分析、代码执行
├── Skill API           # 科研方法与领域流程
├── Model Provider API  # DeepSeek及兼容模型
└── Evaluation API      # 任务、轨迹、证据、结果评分
```

如果一个新功能必须同时修改多个核心模块，先在 PR 中解释现有扩展面为什么无法承载它，不要直接引入新的跨层依赖。

## 6. RehabMain 连接器基线

RehabMain 是 Galen 的第一个真实世界连接器，不是普通文件导入器。

- RehabMain 向 `%LOCALAPPDATA%\Rehab\GalenConnector\latest.json` 发布最小研究快照。
- Galen 治理数据并将接受、排除、纠正、复采、复测决定写入同目录的 `feedback.json`。
- 每条观察必须保留 `source_record_id`，能回溯到来源评估。
- 写入必须幂等；重复读取不能制造重复研究事件或重复行动。

协议和闭环设计见 [真实世界康复数据闭环](docs/real-world-rehabilitation-data-loop.md)。修改连接器时，至少运行连接器领域测试和 Rehab 上下文组件测试。

## 7. 推荐的贡献流程

不要直接在默认分支上堆积长期未提交修改。每个任务使用短分支：

```bash
git switch galen-research-workbench
git pull --ff-only
git switch -c contrib/<short-topic>

# 修改并验证
git add <与任务有关的文件>
git commit -m "feat: describe the user-visible outcome"
git push -u origin contrib/<short-topic>
```

然后创建 PR，目标分支选择 `galen-research-workbench`。PR 必须说明：

1. 用户可观察到的变化。
2. 为什么需要修改。
3. 实际运行过哪些验证。
4. 是否改变数据、协议、隐私边界或兼容性。
5. 出错时如何回滚。

禁止提交：API Key、患者原始数据、个人路径、`node_modules`、`rust/target`、本地模型配置和未脱敏日志。

## 8. 按改动类型验证

| 改动 | 最低验证 |
|---|---|
| React / TypeScript | `npm test`、`npm run build` |
| Rust 核心 | 对应 crate 测试、`cargo test --workspace` |
| Rust 公共接口 | `cargo clippy --workspace` 加调用方测试 |
| 连接器协议 | 领域合同测试、幂等测试、一次真实双向闭环 |
| 评测体系 | 合同校验、固定样例、与现有基线比较 |
| Windows 打包 | 从最终 EXE 启动，不以编译成功代替启动验证 |
| 发布链 | GitHub Actions 成功、Release 资产存在、下载后可启动 |

不要为了每次小改动反复全量打包，但涉及启动、资源、sidecar、安装或更新时，必须验证最终用户产物。

## 9. 发布

Windows 发布工作流位于 `.github/workflows/galen-windows-release.yml`。

- 推送 `galen-research-workbench` 会预热和验证 Windows 构建缓存。
- 推送 `v*` 标签或手动运行工作流会创建正式 Release。
- 正式发布依赖仓库 Secrets 中的 Tauri 更新签名密钥。
- Release 必须同时检查安装包、便携版、版本号和公开下载页面。

维护者没有发布密钥时，仍可贡献代码和运行本地构建；正式发布由拥有发布权限的人执行。

## 10. 首批接手任务

按顺序完成，而不是同时扩张功能：

1. **干净克隆验证**：在新目录或新机器按本文启动 Galen，修正文档中的全部断点。
2. **统一验证入口**：建立 `scripts/verify.ps1`，串联前端、Rust、连接器合同和核心评测冒烟。
3. **提炼 Connector Core**：从 RehabMain 实现中抽出 manifest、事件信封、游标、幂等和行动协议。
4. **Research Folder Connector**：读取指定文件夹的 CSV、PDF、Markdown，作为第二个参考连接器。
5. **连接器测试工具包**：同一套合同测试可验证 RehabMain 与 Research Folder。
6. **评测回归门槛**：为检索相关性、停止决策、持续执行、证据追溯建立固定基线。

第二个连接器完成前，不要设计庞大的“万能插件平台”。两个真实实现共同需要的部分，才进入公共核心。

## 11. 接手完成标准

维护者能够独立完成以下任务，才算真正接手：

- 从全新克隆启动桌面应用。
- 解释 Galen、RehabMain、连接器和评测体系的关系。
- 修复一个真实 Issue，并通过 PR 合入。
- 新增或扩展一个连接器而不破坏核心闭环。
- 运行评测并解释能力为什么上升或下降。
- 构建并发布一个 Windows 版本。
- 在没有原作者即时帮助的情况下处理一次用户反馈。

建议用一次完整的 Research Folder Connector PR 作为接手考试。

## 12. 文档索引

- [README](README.md)：产品定位和快速入口
- [开发者接入指南](docs/DEVELOPER_ONBOARDING.md)：环境与平台构建
- [真实世界数据闭环](docs/real-world-rehabilitation-data-loop.md)：RehabMain 双向连接器
- [项目哲学](PHILOSOPHY.md)：长期设计原则
- [路线图](ROADMAP.md)：阶段目标
- [评测框架](docs/context-engineering-eval-framework.md)：上下文与行为评测
- [模型兼容性](docs/MODEL_COMPATIBILITY.md)：模型供应商适配

当文档与代码冲突时，以当前默认分支的可执行行为和自动化测试为准，并通过 PR 同步修正文档。
