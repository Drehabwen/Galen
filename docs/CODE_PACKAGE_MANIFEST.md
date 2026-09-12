# Galen 源码交接包清单

本包用于开发者接手 Galen 当前源码并在本地完成运行、测试和 Windows 构建。

## 版本基线

- 仓库：`https://github.com/Drehabwen/Galen`
- 分支：`galen-research-workbench`
- 交接基线：`806ebf5 feat: establish durable PI orchestration kernel`
- 包日期：2026-09-12

## 已包含

- Rust workspace、Tauri 后端、React/TypeScript 前端
- Connector、RehabID、项目上下文、MCP 与科研任务执行代码
- 产品文档、数据模型、评测任务和 GitHub Actions 配置
- `AGENTS.md`、`README.md` 以及 `docs/GALEN_USAGE_DEPLOYMENT_HANDOFF.md`

## 已排除

- `.git`、`.worktrees`、`.codex-temp`、`.playwright-cli`
- `node_modules`、`dist`、`rust/target`、临时目录和构建输出
- 本机模型配置、API 密钥、数据库和缓存
- 旧版视频、PDF 和本地实验产物

## 接手顺序

1. 阅读 `AGENTS.md`。
2. 阅读 `docs/GALEN_USAGE_DEPLOYMENT_HANDOFF.md`。
3. 安装 Rust、Node.js 20、Python 3 和 Windows C++ Build Tools。
4. 下载 sidecar、执行 `npm ci`。
5. 配置 `%USERPROFILE%\\.galen\\models.toml`。
6. 运行 `cargo check --workspace`、`npx tsc --noEmit` 和 `npm test`。
7. 执行 `npm run tauri dev` 启动桌面应用。

