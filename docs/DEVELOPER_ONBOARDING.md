# Galen 开发者接入指南（macOS / Windows）

> 目标：让团队成员在 Mac 或 Windows 上直接参与 Galen 的构建与开发。本指南覆盖环境准备、克隆、构建、打包与协作规则。

---

## 一、前置条件

1. 一个 GitHub 账号，并已被仓库管理员添加为 collaborator
2. 一个 DeepSeek API Key（`~/.galen/models.toml` 中配置，各自使用，不要共享）
3. 网络能访问 GitHub 与包注册源（部分网络需代理）

## 二、安装开发工具

### macOS

| 工具 | 版本 | 安装方式 |
|------|------|----------|
| Reasonix（AI 编码客户端） | 最新桌面版 | 官网 [reasonix.io](https://reasonix.io/?download=desktop#start) 下载 dmg；或 `brew install --cask esengine/reasonix` |
| Xcode Command Line Tools | 最新 | `xcode-select --install` |
| Node.js | 20.x | `brew install node@20` 或 nvm |
| Rust | stable | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Python | 3.x | macOS 自带；`brew install python` 亦可 |

### Windows

| 工具 | 版本 | 安装方式 |
|------|------|----------|
| Codex / Reasonix | 最新 | 桌面客户端或 CLI |
| MSVC Build Tools | VS 2022 Build Tools | 勾选「使用 C++ 的桌面开发」+ VC.Tools.x86.x64 |
| Rust | stable | rustup-init.exe |
| Node.js | 20.x | nvm-windows 或官方安装包 |
| WebView2 | 最新 | 系统更新自带，缺失时应用会提示安装 |

## 三、克隆仓库

```bash
git clone -b galen-research-workbench https://github.com/Drehabwen/Galen.git
cd Galen
```

> 只用 `galen-research-workbench` 分支，`main` 是历史导入，不参与开发。

## 四、首次构建

### 1. 下载平台 sidecar（typst / deno / uv）

```bash
cd rust
python3 scripts/download_sidecars.py
```

### 2. 前端依赖

```bash
cd crates/galen
npm ci
```

### 3. 验证编译

```bash
cd ../..   # 回到 rust/
cargo check --workspace
cd crates/galen && npx tsc --noEmit
```

## 五、开发运行

```bash
cd rust/crates/galen
npm run tauri dev
```

首次启动会弹出应用窗口（macOS 需要授权网络 / 辅助功能时请允许）。修改 Rust 或前端代码后，`tauri dev` 会自动热重载。

## 六、打包发布

### macOS（app + dmg）

```bash
cd rust/crates/galen
npm run tauri -- build --bundles app,dmg
```

产物在 `rust/target/release/bundle/`：

- `macos/Galen.app`
- `dmg/Galen_0.1.0_*.dmg`

> 本地构建的 dmg 未签名：首次打开时右键图标 →「打开」，或到「系统设置 → 隐私与安全性」允许。

### Windows（NSIS 安装包）

```bash
cd rust/crates/galen
npm run tauri -- build
```

产物：`rust/target/release/bundle/nsis/Galen_0.1.0_x64-setup.exe`

### CI 自动构建

推送 `galen-research-workbench` 分支会自动触发 [GitHub Actions](https://github.com/Drehabwen/Galen/actions)：

- `galen-macos.yml`：macOS 双架构（Intel x86_64 / Apple Silicon arm64），产出 app + dmg 构建产物
- 构建产物在每次运行的「Artifacts」里下载

## 七、协作规则

1. **分支**：一律在 `galen-research-workbench` 上开发，不新建长期并行分支；大改动先小步提交
2. **提交信息**：简洁说明「做了什么 + 为什么」，如 `fix: PubMed DTD 兼容，检索不再失败`
3. **验证**：提交前必须通过 `cargo check`、`npx tsc --noEmit`；涉及构建链的改动要实际跑一遍 `tauri build`
4. **不提交的内容**：API Key、`~/.galen/`、本地数据库路径、`node_modules`、`rust/target`（已在 .gitignore）
5. **代码风格**：UI 颜色必须引用 `styles/tokens.css` 变量；Rust 遵循 workspace lints（clippy pedantic）
6. **反馈**：体验问题 / 需求发到团队群或仓库 Issue，附操作步骤与截图

## 八、常见问题

| 问题 | 解决 |
|------|------|
| `npm ci` 很慢或失败 | 换 npm 镜像源：`npm config set registry https://registry.npmmirror.com` |
| sidecar 下载失败 | 检查网络；脚本支持断点重跑；或手动下载对应平台二进制放入 `src-tauri/binaries/` |
| macOS 提示「无法打开，因为无法验证开发者」 | 右键 → 打开；或 `xattr -dr com.apple.quarantine Galen.app` |
| `tauri dev` 端口被占用 | 修改 `tauri.conf.json` 的 `devUrl` 端口（默认 1420）后重启 |
| cargo 编译太慢 | 首次全量编译约 5–15 分钟属正常；`cargo check` 比 `cargo build` 快得多 |
| 没有康复数据可测 | 本地配置 `~/.galen/rehab.toml` 指向测试库（只读工具，不写数据） |

---

遇到本指南未覆盖的问题，直接在工作台里问，或提交 Issue。
