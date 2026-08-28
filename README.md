# 🏛️ Galen — 康复科研闭环工作台

<p align="center">
  <strong>面向康复科研与临床团队的闭环工具 —— 采集 · 处理 · 分析 · 成文 · 签核</strong>
</p>

<p align="center">
  <a href="https://github.com/Drehabwen/Galen">GitHub</a>
  ·
  <a href="#快速开始">快速开始</a>
  ·
  <a href="#核心能力">核心能力</a>
  ·
  <a href="#架构">架构</a>
  ·
  <a href="#文档">文档</a>
</p>

---

Galen 是面向**康复科研**的闭环工作台：一线场景的多模态数据（量表 / 评估 / 视频 / 语音）统一接入后，由 AI 自主完成数据处理、证据分析、报告成文，人类只做**计划把关**与**最终签核**。命名致敬古希腊医学之父盖伦（Galen of Pergamon）。

**核心信念**

- **数据基础设施与模型能力并重** —— 数据质量在源头产生，管道和模型同等重要
- **科研品味驱动，而非通用对话** —— 由装配版科研技能库驱动自主执行，不靠提示词堆砌
- **闭环、证据链、可复现** —— 每个结论都能回溯到原始数据与执行过程

## 核心能力

| 模块 | 说明 |
|------|------|
| 🔁 **任务级闭环** | 输入任务 → 计划画布 → 节点自动执行 → 证据回流 → 全部完成自动成文，人类随时可介入签核 |
| 🧠 **科研品味内核** | 主编人格 + 装配版科研技能库（设计评审 / 检索 / 证据提取 / 数据分析 / 写作 / 自审），七条科研品味判断标准 |
| 🚀 **DeepSeek 默认** | 默认 DeepSeek V4 Flash，复杂深度研究可切换 V4 Pro，思考强度四档；不再依赖 Anthropic |
| 📚 **文献检索** | 修复后的 PubMed 检索（兼容 XML DTD），支持摘要加载与证据分级整理 |
| 🏥 **康复数据接入** | 只读 SQLite 数据工具，7 种操作，查询结果带来源头；支持量表、评估、视频、语音四类数据 |
| 📐 **统一数据模型** | `subject → assessment_session → scale / measure / video / audio → evidence`，跨模态可查询、证据可追溯 |
| 💾 **持久化与记忆** | 计划存 `plan.json`，节点回流自动追加 `GALEN.md` 项目记忆，重启后闭环继续 |

## 快速开始

### Windows 用户

1. 从 [Releases](https://github.com/Drehabwen/Galen/releases) 下载 `Galen_0.1.0_x64-setup.exe`
2. 双击安装，首次启动按向导配置 DeepSeek API Key 与工作区
3. 在工作台输入科研任务，确认计划后 AI 自动推进闭环

### macOS 用户

1. 从 GitHub Actions（[galen-macos.yml](https://github.com/Drehabwen/Galen/actions/workflows/galen-macos.yml)）下载构建产物：
   - `galen-x86_64-apple-darwin`（Intel Mac）
   - `galen-aarch64-apple-darwin`（Apple Silicon）
2. 解压后拖入「应用程序」；首次打开未签名应用需右键 → 打开

### 从源码构建

```bash
git clone https://github.com/Drehabwen/Galen.git
cd Galen/rust
cargo build --release -p galen
# 桌面应用入口在 rust/crates/galen/
```

详细步骤见 [docs/DEVELOPER_ONBOARDING.md](docs/DEVELOPER_ONBOARDING.md)。

## 模型配置

模型配置保存在 `~/.galen/models.toml`（Windows 为 `%USERPROFILE%\.galen\models.toml`），应用首次启动会引导保存 DeepSeek API Key：

```toml
[router]
default = "deepseek-v4-flash"
fast = "deepseek-v4-flash"
analysis = "deepseek-v4-pro"

[models.deepseek-v4-flash]
provider = "openai_compat"
api_key = "sk-xxx"
model_id = "deepseek-v4-flash"
base_url = "https://api.deepseek.com/v1"

[models.deepseek-v4-pro]
provider = "openai_compat"
api_key = "sk-xxx"
model_id = "deepseek-v4-pro"
base_url = "https://api.deepseek.com/v1"
```

## 架构

```
rust/
  crates/api               ── 多 Provider LLM 调用抽象
  crates/model-router      ── 模型配置抽象，TOML → ProviderClient
  crates/medical-core      ── PubMed / MeSH 检索、引用格式化、医学提示词
  crates/runtime           ── 对话运行时与工具执行
  crates/tools             ── 工具注册与执行（含康复数据工具）
  crates/plugins           ── 插件系统与 MCP 集成
  crates/galen             ── Tauri 2.x 桌面应用（React + TypeScript 前端）
       src-tauri/src/tools/rehab.rs  ── 康复数据只读接入
       src/                ── 前端：任务闭环状态机、计划画布、会话自动执行
```

- 桌面框架：Tauri 2.x + React 18 + TypeScript + Vite 5
- 外部依赖（sidecar）：typst / deno / uv，由 `rust/scripts/download_sidecars.py` 按平台下载
- CI：GitHub Actions 同时构建 Windows（NSIS）与 macOS（app + dmg，Intel / Apple Silicon）

## 可靠性评测

Galen 的评测直接运行 Rust Agent Loop，同时检查模型响应、工具轨迹、工作区状态、
医学数字来源、科研边界、可预览产物、Token 与端到端时延。AIS 教科书试点包含
10 个去标识化病例、40 个任务，并按病例隔离 development / validation / hidden。

当前冻结配置下，DeepSeek V4 Pro 在 6 个 T2 来源封闭任务上完成 Repeat-5：
30/30 通过，`pass^5 = 100%`，TTFR P95 为 1.70 秒，总耗时 P95 为 21.5 秒。
这代表开发集 T2 的 PR Gate，不代表整个产品或隐藏集已达到发布门槛。

```powershell
cd rust
cargo run -p galen --bin eval -- validate
cargo run -p galen --bin eval -- reliability --input ../evals/runs/run.jsonl --k 5
```

详见 [评测说明](evals/README.md)与
[Repeat-5 报告](evals/reports/ais-t2-source-closed-watchdog-pro-repeat5-final-2026-08-25.md)。

## 文档

| 文档 | 说明 |
|------|------|
| [产品使用说明](docs/GALEN_USER_GUIDE.md) | 面向使用者的完整操作手册 |
| [Alpha 自由探索手册](docs/GALEN_ALPHA_EXPLORATION_GUIDE.md) | 面向受邀体验者的安全边界、探索方向与问题反馈模板 |
| [统一数据模型](docs/rehab-data-model.md) | 多模态康复数据的接入与证据链设计 |
| [开发者接入](docs/DEVELOPER_ONBOARDING.md) | macOS / Windows 开发者环境、构建与协作规则 |
| [PRD](docs/galen-prd-v0.2.md) | 产品需求与迭代方向 |
| [产品对比](docs/rehab-product-comparison.md) | 同类产品调研 |

## 分支与协作

- 唯一维护分支：`galen-research-workbench`（`main` 为历史导入，不再维护）
- 所有开发直接在 research 分支协作；成员需由仓库管理员添加为 collaborator

## 许可

MIT

---

<p align="center">
  Made for rehab researchers who want the loop closed.
</p>
