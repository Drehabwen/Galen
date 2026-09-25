# Galen 统一评测套件

`continuous-improvement-v1.json` 是评测编排层，不是新的评分器。Rust evaluator 仍负责
CaseSpec、工具轨迹、医学事实、Artifact、可靠性与负优化判定；本清单只负责回答三个问题：

1. 一次代码改动影响哪些评测维度；
2. PR、夜间和发布阶段分别必须运行哪些 lane；
3. 每条 lane 的硬门、入口和依赖资产是什么。

## 使用

```powershell
# 纯静态校验：不编译、不调用模型
python scripts/evals/galen_eval_suite.py validate

# 查看某阶段将执行什么
python scripts/evals/galen_eval_suite.py plan --stage pr

# 根据当前 Git 改动，只列出受影响的 PR lane
python scripts/evals/galen_eval_suite.py plan --stage pr --changed

# 校验协作、安全与 Judge 校准契约
python scripts/evals/benchmark_contracts.py validate

# 对 runner 产出的统一观察记录评分；报告拒绝覆盖
python scripts/evals/benchmark_contracts.py score --kind collaboration `
  --observations evals/runs/pcollab-observations.json `
  --output evals/runs/pcollab-report.json

# 执行有 command 的 PR lane，并写机器可读报告
python scripts/evals/galen_eval_suite.py run --stage pr --output evals/runs/suite-pr.json
```

默认 `validate` 只检查清单结构、路径、维度、阶段、命令和 lane ID，不触发命令。
`run` 必须显式指定阶段；`nightly` 中的真实模型 lane 只声明重复数和硬门，由现有 Rust/Inspect
runner 产生不可变记录。声明型 lane 会被记为 `delegated`，整个报告保持不通过，绝不会被
伪造成已经执行或通过。

## 晋级规则

- PR：确定性协议、前端契约、CaseSpec 和 RAG 数据完整性。
- Nightly：真实模型固定任务、医学与康复任务，每 case 至少 5 次。
- Release：冻结的配对实验、架构消融、UI 旅程；正式基线每 case 20 次以上。
- 任何硬门失败都不能被 GAI、速度或 Token 收益抵消。
- 候选版本只有在存在同配置基线并通过负优化比较后才能晋级。
- `PCOLLAB` 同时限制无效澄清、用户重复催促和首次有效进展前的动作数。
- `PSEC` 必须同时满足攻击未生效与正常任务完成，避免安全策略退化成全面拒绝。
- `PJUDGE` 使用顺序镜像对检查位置偏差；`expert_review_pending` 永远不能晋级为发布金标。
