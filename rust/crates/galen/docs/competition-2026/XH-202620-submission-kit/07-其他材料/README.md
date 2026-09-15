# 07—其他材料：评审核验证据包

本目录不是作品方案的重复说明，而是 Galen 所有量化主张的可复核证据。

## 建议评审顺序

1. 阅读 `Galen-07-其他材料-测试与核验说明.pdf`，查看多智能体对比、消融与回归结果。
2. 运行 `run_judge_benchmark.ps1`，从两套系统共 150 次保存记录重新生成结果表。
3. 打开 `verification-output/judge-summary.csv`，核对 74/75、21/75、工具调用和中位时延。
4. 进入 `evidence-data/representative-cases/`，抽查 PCTX03、PDATA04、PTOOL02 的任务卡、双方轨迹、自动判定与 Galen 实际产物。
5. 使用 `证据总表.md` 从比赛主张定位至原始证据。

默认核验方式不调用模型，也不需要 API Key：

```powershell
.\run_judge_benchmark.ps1
```

在完整仓库且模型配置可用时，可启动实时 Galen 实验：

```powershell
.\run_judge_benchmark.ps1 -Mode Live
```

## 目录结构

- `evidence-data/raw-results/`：Galen 与 Codex + DeepSeek 各 75 次原始 JSONL。
- `evidence-data/representative-cases/`：三张代表任务的完整核验链。
- `evidence-data/SHA256SUMS.txt`：证据文件完整性哈希。
- `paired-benchmark/`：15 张冻结任务卡和固定输入。
- `证据总表.md`：指标、结果、证据位置与核验动作。
- `run_demo_checks.ps1`：基础编译、单元测试与前端检查。
- `复现说明.md`：实验环境与指标定义。

## 当前版本

- GitHub：<https://github.com/Drehabwen/Galen>
- 分支：`galen-research-workbench`
- 提交：`3a179fb`
- 数据：固定脱敏或构造输入，不包含真实个人数据。
