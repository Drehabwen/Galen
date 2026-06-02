# VIBE Paper

<p align="center">
  <strong>🦞 医学科研助手 — 读写一体，一条龙解决</strong>
</p>

<p align="center">
  <a href="https://github.com/Drehabwen/VIBE-paper">GitHub</a>
  ·
  <a href="#快速开始">快速开始</a>
  ·
  <a href="#功能">功能</a>
  ·
  <a href="#模型配置">模型配置</a>
</p>

---

VIBE Paper 是一个**原生桌面应用**，专为医学生设计。双击 exe 即可启动，无需终端、无需编程。

**核心理念：** 把你留在应用里 —— 搜文献、读论文、写文章、格式化引用，一站式完成。

## 功能

| 模块 | 说明 |
|------|------|
| 🤖 **多模型聊天** | 支持 Claude / GPT / 本地模型，流式对话，配置零门槛 |
| 📚 **文献检索** | PubMed 搜索，一键加载摘要，MeSH 术语查询 |
| 📝 **引用格式化** | 支持 APA、Vancouver、BibTeX、RIS、MLA 五种格式 |
| 📄 **工作区** | 左栏查看论文、编辑文档、预览模板 |
| 🎨 **暗色主题** | 护眼深色界面，中文字体原生支持 |

## 快速开始

### Windows 用户（推荐）

1. 从 [Releases](../../releases) 下载 `vibe-paper.exe`
2. 双击运行，窗口自动打开
3. 选模型、开聊

### 从源码构建

```bash
git clone https://github.com/Drehabwen/VIBE-paper
cd VIBE-paper/rust
cargo build --release -p claw-gui
# exe 在 target/release/claw-gui.exe
```

## 模型配置

在 `%USERPROFILE%\.claw\models.toml` 中配置你的模型：

```toml
[router]
default = "sonnet"

[models.sonnet]
provider = "anthropic"
api_key = "sk-ant-xxx"
model_id = "claude-sonnet-4-6"

[models.gpt4o]
provider = "openai"
api_key = "sk-xxx"
model_id = "gpt-4o"
```

没有配置文件时，应用会自动检测环境变量中的 API key 并使用内置默认模型列表。

## 架构

```
model-router/     ── 模型配置抽象，TOML → ProviderClient
medical-core/     ── PubMed/MeSH 客户端，引用格式化引擎
claw-gui/         ── egui 桌面应用，2 栏布局，流式聊天
api/              ── 多 Provider LLM 调用（Anthropic / OpenAI / 兼容）
runtime/          ── 对话运行时 & 工具执行
```

新增 crate 不修改现有 api/runtime/tools 核心逻辑，完全向后兼容。

## 许可

MIT

---

<p align="center">
  Made for medical students who just want to get work done.
</p>
