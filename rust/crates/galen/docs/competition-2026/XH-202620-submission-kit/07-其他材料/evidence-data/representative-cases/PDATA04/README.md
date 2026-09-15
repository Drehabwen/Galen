# PDATA04 代表性案例核验

配对重复编号：1

| 系统 | 硬门 | 工具调用 | 总时延 | 失败断言 |
|---|---:|---:|---:|---|
| Galen | 通过 | 2 | 8.6 s | — |
| Codex + DeepSeek | 未通过 | 5 | 15.6 s | 见 codex-run.json |

Galen 产物：galen-artifact.md

核验顺序：先读 `task-card.toml`，再对照两份 `*-run.json` 的 `assertions`、`tool_trace` 与 `artifacts` 字段，最后查看双方最终回答和 Galen 实际产物。
