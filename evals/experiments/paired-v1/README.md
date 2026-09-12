# Galen 配对评测体系 v1

这个实验包把 Galen 的改进拆成三个可重复层次：

1. **上下文配对**：同一模型、同一任务、同一 fixture，只切换 `full` 与 `fullpack`；
2. **工具经济性**：记录模型请求、工具调用、错误、重复调用、时延和交付；
3. **数据治理**：验证字段、单位、重复/缺失、时间轴、来源冲突和溯源。

原始 JSONL 是事实源；`metrics.csv` 用于画图和统计；`report.md` 面向研发复盘。所有运行都记录 Git commit、dirty 状态、模型、参数和 SHA-256。

## 使用

先构建评测器：

```powershell
cd D:\DEV\Galen-new\rust
cargo build -p galen --bin eval
```

只校验实验包：

```powershell
& D:\DEV\Galen-new\evals\experiments\paired-v1\run.ps1 -ValidateOnly
```

跑一轮冒烟：

```powershell
& D:\DEV\Galen-new\evals\experiments\paired-v1\run.ps1 -Profile smoke
```

跑正式实验（每张卡至少 5 次）：

```powershell
& D:\DEV\Galen-new\evals\experiments\paired-v1\run.ps1 -Profile formal
```

中途终止后，如果每个已有案例文件都已完整，可用同一个 `RunId` 继续：

```powershell
& D:\DEV\Galen-new\evals\experiments\paired-v1\run.ps1 -Profile formal -RunId 20260911-formal-xxxxxxx -Resume
```

## 输出目录

每次运行写入独立目录：

```text
evals/runs/paired-v1/<RunId>/
├── raw/                         # 每张卡的独立 JSONL
├── context-baseline.jsonl       # 完整历史基线
├── context-candidate.jsonl      # fullpack 候选
├── context-comparison.json      # 正式比较结果
├── context-comparison.stderr.txt
├── functional-current.jsonl     # 工具与数据功能基线
├── metrics.csv                  # 按案例聚合指标
├── runs.csv                     # 每次运行的宽表
├── failures.csv                 # 失败断言清单
├── manifest.json                # 环境、参数、哈希
└── report.md                    # 人类可读报告
```

## 读报告的顺序

先看关键事实是否全部保留、旧范围是否泄漏、产物是否真实交付；再看重复成功率和 Wilson 下界；最后才看 Token、TTFR 和工具调用数。效率不能抵消质量退化。

工具/数据组当前明确标为“功能基线”。只有实现真正的 `baseline|galen` 工具策略开关以后，才允许把它们写成相对优越性实验。
