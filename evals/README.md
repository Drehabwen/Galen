# Galen Evals

这套评测直接调用 Galen 的 Rust Agent Loop，检查模型输出、工具轨迹、工作区状态和 Artifact。真实运行记录默认写入 `evals/runs/`，该目录中的 JSONL/HTML 被 Git 忽略，避免提交用户数据或模型输出。

## 命令

在 `rust/` 目录运行：

```powershell
# 只验证 CaseSpec 与 fixture，不调用模型
cargo run -p galen --bin eval -- validate

# 真实运行一个案例；Smoke 阶段可先跑 1 次
cargo run -p galen --bin eval -- run --case E01 --repeat 1

# PR Gate 至少运行 5 次
cargo run -p galen --bin eval -- run --case E01 --repeat 5 --output ../evals/runs/e01-candidate.jsonl

# 比较基线与候选；只有 Accept 返回成功退出码
cargo run -p galen --bin eval -- compare --baseline ../evals/baselines/e01-pro.jsonl --candidate ../evals/runs/e01-candidate.jsonl
```

## 数据规则

- `cases/`：版本化的 TOML 评测契约。
- `fixtures/`：只读原始输入；Runner 将其复制到临时目录，绝不原地修改。
- `runs/`：本地不可变 JSONL；保存完整最终响应、工具轨迹和临时工作区位置。
- `baselines/`：只有通过正式审核的基线才能提交。
- `reports/`：后续生成的机器可读/HTML 对比报告。

单次通过只代表链路可运行，不代表候选版本优于基线。PR Gate 每个 case/model/config 至少需要 5 次；正式 Release 基线应积累 20～30 次，才使用 P90 作稳定结论。
