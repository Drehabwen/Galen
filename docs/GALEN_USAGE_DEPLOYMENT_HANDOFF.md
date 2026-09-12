# Galen 使用与部署手册（代码交接版）

适用版本：`galen-research-workbench` 分支，当前交接基线 commit：`806ebf5`。

## 1. 项目结构

```text
rust/                         Rust workspace 与 Tauri 后端
rust/crates/galen/             React + TypeScript 前端和桌面应用
rust/crates/galen/src-tauri/   Tauri 命令、科研任务、Connector、MCP、报告生成
rust/crates/medical-core/      PubMed 与医学检索能力
docs/                         产品、开发、数据模型和评测文档
evals/                        评测任务卡、运行记录和报告
scripts/                      构建与平台脚本
```

产品主线：康复工作台/设备/文件 → Connector → RehabID → 数据治理 → 项目上下文 → 科研分析与证据 → PDF 交付。

## 2. Windows 开发环境

安装以下工具：

- Windows 10/11、WebView2
- Visual Studio 2022 Build Tools，勾选“使用 C++ 的桌面开发”
- Rust stable（rustup）
- Node.js 20.x 与 npm
- Python 3.x

确认：

```powershell
rustc --version
cargo --version
node --version
npm --version
python --version
```

## 3. 获取源码

```powershell
git clone -b galen-research-workbench https://github.com/Drehabwen/Galen.git
cd Galen
```

只在 `galen-research-workbench` 分支开发。

## 4. 首次准备

下载 PDF、公式和数据处理所需的 sidecar：

```powershell
cd rust
python scripts/download_sidecars.py
```

安装前端依赖：

```powershell
cd crates/galen
npm ci
```

## 5. 配置模型

应用使用 `%USERPROFILE%\.galen\models.toml`。首次启动会提供配置向导，也可以手动创建：

```toml
[router]
default = "deepseek-v4-flash"
fast = "deepseek-v4-flash"
analysis = "deepseek-v4-pro"

[models.deepseek-v4-flash]
provider = "openai_compat"
api_key = "替换为个人密钥"
model_id = "deepseek-v4-flash"
base_url = "https://api.deepseek.com/v1"

[models.deepseek-v4-pro]
provider = "openai_compat"
api_key = "替换为个人密钥"
model_id = "deepseek-v4-pro"
base_url = "https://api.deepseek.com/v1"
```

密钥只保存在本机配置中，不写入仓库、不放入压缩包、不提交 Git。

## 6. 本地运行

### 桌面应用

```powershell
cd rust/crates/galen
npm run tauri dev
```

默认开发页面：`http://localhost:1420`。应用窗口启动后，输入科研任务即可开始项目对话；Connector、RehabID、分析、证据和报告在同一项目中连续执行。

### 仅运行前端

```powershell
cd rust/crates/galen
npm run dev
```

前端预览地址：`http://localhost:5173`。

## 7. 典型使用流程

1. 创建或打开一个研究项目。
2. 通过 Connector 接入康复师工作台导出、CSV/Excel、量表或设备数据。
3. 让 Galen 检查字段、单位、时间点、缺失和重复记录。
4. 确认 RehabID 与研究队列，提出研究问题或分析目标。
5. 让 Galen 执行统计、可视化、文献核验和证据关联。
6. 点击结论查看数据来源与论文链接。
7. 生成正式 PDF，在应用内预览、下载和继续修改。

## 8. Connector 与 RehabID

康复师工作台是当前首个数据插件。Connector 从系统下载目录读取工作台备份，生成数据预览和标准化导入记录。其他设备可沿同一协议扩展。

RehabID 是脱敏研究身份，负责把同一受试者的多来源、多个时间点记录组织在同一条纵向时间轴中。项目数据和生成产物保存在研究工作区，不覆盖原始工作台数据。

## 9. 验证与测试

Rust workspace：

```powershell
cd rust
cargo check --workspace
cargo test --workspace
```

前端类型与单元测试：

```powershell
cd crates/galen
npx tsc --noEmit
npm test
```

核心评测：

```powershell
cd rust
cargo run -p galen --bin eval -- validate
```

当前交接基线包含项目上下文、连续约束修订、运动疲劳方向切换、证据链和框架对照实验。

## 10. 构建与发布

### Windows NSIS 安装包

```powershell
cd rust/crates/galen
npm run tauri -- build
```

产物目录：

```text
rust/target/release/bundle/nsis/
```

### 只构建前端

```powershell
cd rust/crates/galen
npm run build
```

### macOS

```bash
cd rust/crates/galen
npm run tauri -- build --bundles app,dmg
```

## 11. 报告生成

Galen 使用 Typst sidecar 生成正式科研 PDF。交付前重点检查：

- 图表与正文数字一致；
- 引用可以点击并回到来源；
- PDF 全文预览正常滚动；
- 输出目录中的产物名称和版本清楚。

计划书与技术佐证报告的源文件位于 `docs/proposal/` 和 `rust/crates/galen/docs/technical-evidence-report/`。

## 12. 常见问题

| 现象 | 处理 |
|---|---|
| `npm ci` 失败 | 检查 Node.js 版本与网络；必要时切换 npm 镜像后重试 |
| `cargo check` 很慢 | 首次构建需要下载依赖，等待完成；后续增量构建会明显加快 |
| 应用无法调用模型 | 检查 `%USERPROFILE%\.galen\models.toml` 的模型名、密钥和 `base_url` |
| PDF 无法生成 | 重新运行 `python rust/scripts/download_sidecars.py`，确认 `typst` 已进入 `src-tauri/binaries/` |
| Connector 没有数据 | 将工作台导出文件放入系统下载目录，再刷新 Connector 预览 |
| 页面端口被占用 | 关闭占用 1420/5173 的进程，或修改 `tauri.conf.json` 的 `devUrl` |

## 13. 交接约定

- 先阅读 `AGENTS.md` 和本手册，再修改代码。
- 所有改动在 `galen-research-workbench` 分支完成。
- 提交前运行 Rust、TypeScript 和前端测试。
- UI 颜色引用 `tokens.css`，不在组件中散落硬编码颜色。
- 不提交 API 密钥、个人数据库、`node_modules`、`rust/target` 和本机缓存。

