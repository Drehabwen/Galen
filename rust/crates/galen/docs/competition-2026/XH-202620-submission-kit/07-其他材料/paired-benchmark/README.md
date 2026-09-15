# 配对基准任务包

本目录包含 15 张可运行 TOML 任务卡和固定脱敏输入：

- `cases/pctx01-pctx05`：上下文保持、修订、范围切换、干扰和文件事实保留；
- `cases/ptool01-ptool05`：读写、检索、跨文件、失败恢复和交付产物；
- `cases/pdata01-pdata05`：字段规范、质量检查、时间轴、冲突来源和溯源。

## 验证任务卡

```powershell
D:\DEV\Galen-new\rust\target\debug\eval.exe validate --cases D:\DEV\Galen-new\evals\cases\paired
```

## 配对运行

上下文组使用同一案例分别运行 `--variant full`（完整历史基线）和 `--variant fullpack`（Galen 研究上下文包）；冒烟可用 `--repeat 1–3`，正式 `eval compare` 要求每个案例/模型/配置至少 `--repeat 5`。运行结果用 `eval compare --baseline FILE --candidate FILE --ignore-config` 比较。

工具组和数据组保持同一模型、同一工具注册表和同一 fixture，仅替换 baseline / Galen 工具策略；记录成功率、调用数、重复调用、Token、时延、产物和引用覆盖率。

所有任务使用脱敏合成输入，不含真实个人数据。任务卡里的 `facts`、`artifacts`、`tools` 和 `tool_sequence` 是自动判定条件，不是人工印象分。
