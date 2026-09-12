# Codex + DeepSeek 外部基线实测

## 运行范围

- 后端：Codex CLI 0.153.4
- Provider：DeepSeek
- 模型：`deepseek-v4-pro`
- 任务：Galen 配对评测卡 15 张，单次运行（PCTX01–05、PDATA01–05、PTOOL01–05）
- 隔离：临时 `CODEX_HOME`，未改动用户全局 Codex 配置
- 记录配置哈希：`8f1b4730197d3264`

## 结果

| 指标 | 结果 |
|---|---:|
| 硬门通过 | 15/15 |
| 成功率 | 100% |
| Wilson 95% 下限 | 79.61% |
| pass@1 | 100% |
| 总耗时 | 1,222,151 ms |
| 平均耗时 | 81,477 ms |
| 中位耗时 | 61,439 ms |
| P90 耗时 | 146,743 ms |
| 工具错误事件 | 6 |

## 解释

DeepSeek 在 15 张任务卡上均完成了事实保留、文件产物交付和禁止项检查。工具错误事件没有导致硬门失败，主要来自模型尝试使用 PowerShell 环境不支持的 Bash heredoc，随后自行恢复。

批处理第一次运行在 `PTOOL02` 达到 300 秒时中止；适配器随后增加了超时捕获，并对 `PTOOL02–05` 逐张重试。上表使用每张卡最后一次有效单次结果，因此 `15/15` 是最终 pass@1 结果，不代表没有发生过超时。

这是一轮 `pass@1` 能力烟雾基线，不是重复性结论；要比较稳定性，还需要对关键卡片做多次重复运行，并将 Galen 与其他框架置于同一模型、同一工具权限和同一超时策略下。

## 可复现命令

```powershell
python evals/agent/baselines/external_runner.py `
  --backend codex `
  --case all `
  --repeat 1 `
  --codex-home <临时隔离目录> `
  --model-label deepseek/deepseek-v4-pro `
  --output evals/runs/external-codex-deepseek-full-v1.jsonl

rust/target/release/eval.exe reliability `
  --input evals/runs/external-codex-deepseek-full-v1.jsonl `
  --k 1
```
