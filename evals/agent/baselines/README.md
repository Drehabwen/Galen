# 外部 Agent 基线适配器

这里的脚本只负责把外部 Agent 放进 Galen 的同题评测，不把任何上游代码复制进产品。

## 统一边界

- 每个案例使用同一份 `evals/cases/paired` 任务卡和 fixture；
- 外部 Agent 在独立临时工作区运行，只能看到该案例的输入；
- 模型、网络和超时参数在运行清单中记录；
- 适配器把最终回答、文件、工具调用和耗时转换成 Galen `RunRecord` 兼容 JSONL；
- 原始事件保存在该运行目录的 `raw-events/`，规范化记录才进入比较器。

## Codex CLI

本机有 `codex` 时运行：

```powershell
cd D:\DEV\Galen-new
python evals/agent/baselines/external_runner.py `
  --backend codex `
  --case PDATA02 `
  --repeat 1 `
  --output evals/runs/external-codex-smoke.jsonl
```

### 使用隔离的 DeepSeek Provider

不要切换正在使用的 Codex 全局配置。为子进程准备临时目录，放入
`config.deepseek.toml`（命名为 `config.toml`）和 `auth.deepseek.json`
（命名为 `auth.json`），然后通过 `--codex-home` 注入：

```powershell
python evals/agent/baselines/external_runner.py `
  --backend codex `
  --case all `
  --repeat 1 `
  --codex-home <临时隔离目录> `
  --model-label deepseek/deepseek-v4-pro `
  --output evals/runs/external-codex-deepseek-full-v1.jsonl
```

适配器只把该目录传给子进程的 `CODEX_HOME`，不会覆盖用户的
`C:\Users\labops\.codex\config.toml` 或 `auth.json`。运行结束后应删除临时
目录中的凭据文件；不要把凭据提交到仓库。

正式比较前必须把 `--repeat` 提高到 5，并保持与 Galen 相同的模型、网络和任务卡版本。适配器不会把缺失的 token 或工具事件猜成真实值；拿不到的字段写 0 或空值并在 `manifest.json` 标明。

### 严格对齐验收

严格模式不只检查回答文本，还检查可观察的 Codex JSON 事件：必需工具、工具顺序、最大工具调用数、重复调用上限、必需事实、禁用词和产物交付。工具事件按 `file_change`、读写型命令、`web_search` 做保守语义归一化；未观察到的调用不会补记。

2026-09-11 的同模型严格基线运行参数为：15 张配对任务卡、固定 fixture、`deepseek/deepseek-v4-flash`、每卡 5 次、隔离 `CODEX_HOME`。原始结果见 `evals/runs/external-codex-deepseek-flash-strict-v1.jsonl`，总结见同目录的 `*-summary.md`。

## 其他框架

Claude Code、OpenCode、OpenHands 采用同一个 `ExternalBackend` 接口。当前本机已下载 Claude Code 与 OpenCode，并在 `evals/external-runtimes/config/` 写入了与 Galen 相同的 DeepSeek 本地配置；运行时请使用 `evals/external-runtimes/with-deepseek.ps1`，不要把 key 复制到用户级配置。只有完成一次单案例 smoke、并能输出原始事件时，才允许加入正式比较。

### 本地三框架先导适配器

`framework_pilot.py` 使用同一任务卡和临时工作区，调用隔离 wrapper，并保留原始事件。示例：

```powershell
python evals/agent/baselines/framework_pilot.py `
  --backend opencode `
  --case all `
  --repeat 1 `
  --model-label opencode+deepseek-v4-flash `
  --output evals/runs/pilot-opencode-all-v1.jsonl
```

`--backend` 可选 `codex`、`claude`、`opencode`。这只是先导适配器；正式比较前仍需固定各 CLI 的事件归一化、预算和权限边界，再按协议增加重复次数。

## 2026-09-12 大规模批次约定

- Codex 适配器默认以 `--disable plugins` 启动，避免插件市场同步、路径过长等启动噪声占用任务预算；不会修改用户正在使用的 Codex 配置。
- Claude 的预算留出一次产物确认回合；供应商超时或预算耗尽仍按失败记录，不把“写出文件”自动改判为通过。
- OpenCode 使用 `--pure --agent build --variant low`，并在配置中拒绝工作目录外访问；若模型仍反复越界，记录为适配器边界失败，不与 Galen/Codex 主统计混合。
- Windows 超时会杀掉整个子进程树；`--start-index` 可从指定重复编号恢复一张卡，避免重跑已经完成的记录。

最终 15 卡×5 次对齐结果：

- `evals/runs/large-full-galen-20260912-final.jsonl`
- `evals/runs/large-codex-deepseek-flash-20260912-complete.jsonl`
- `evals/runs/large-comparison-20260912-final.md`
