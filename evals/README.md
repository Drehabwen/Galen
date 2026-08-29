# Galen Evals

这套评测直接调用 Galen 的 Rust Agent Loop，检查模型输出、工具轨迹、工作区状态和 Artifact。真实运行记录默认写入 `evals/runs/`，该目录中的 JSONL/HTML 被 Git 忽略，避免提交用户数据或模型输出。

`evals/agent/` 进一步提供 Inspect AI 外部编排层。它采用 τ³-bench 风格的
模拟用户私有目标和 Letta 风格的记忆探针，但最终仍读取 Galen Rust evaluator
产生的文件、工具、Token、延迟和硬门记录。该层不进入桌面应用运行时。

## 命令

在 `rust/` 目录运行：

```powershell
# 只验证 CaseSpec 与 fixture，不调用模型
cargo run -p galen --bin eval -- validate

# 真实运行一个案例；Smoke 阶段可先跑 1 次
cargo run -p galen --bin eval -- run --case E01 --repeat 1

# 不再次调用模型，按当前 CaseSpec 对已有工作区产物重新评分（输出不可覆盖）
cargo run -p galen --bin eval -- rescore --input ../evals/runs/old.jsonl --output ../evals/runs/rescored.jsonl

# PR Gate 至少运行 5 次
cargo run -p galen --bin eval -- run --case E01 --repeat 5 --output ../evals/runs/e01-candidate.jsonl

# 比较基线与候选；只有 Accept 返回成功退出码
cargo run -p galen --bin eval -- compare --baseline ../evals/baselines/e01-pro.jsonl --candidate ../evals/runs/e01-candidate.jsonl

# 汇总可靠率、Wilson 95% 下界、pass^5 与 Galen Agent Index
cargo run -p galen --bin eval -- reliability --input ../evals/runs/e01-candidate.jsonl --k 5

# 验证 RAG 黄金集、ResearchTask 和证据 ID 的完整性（不调用模型）
cargo run -p galen --bin eval -- rag-validate --dataset ../evals/datasets/rag_ais_scoliosis.toml

# 运行确定性检索基准：输出 Recall@K、MRR、nDCG、干扰命中和冷热延迟
cargo run -p galen --bin eval -- rag --dataset ../evals/datasets/rag_ais_scoliosis.toml --repeat 10 --output ../evals/runs/rag-ais-candidate.json

# 比较 RAG 基线与候选；只有无质量回退且存在显著收益时返回 Accept
cargo run -p galen --bin eval -- rag-compare --baseline ../evals/baselines/rag-ais.json --candidate ../evals/runs/rag-ais-candidate.json

# 汇总 Agent JSONL 与 RAG JSON，生成可在 Galen 内预览、可继续转 PDF 的 Markdown 报告
cargo run -p galen --bin eval -- report --agent ../evals/runs/m02.jsonl --rag ../evals/runs/rag-ais-candidate.json --output ../evals/reports/galen-eval.md --title "Galen AIS 测评报告"
```

## 外部 Agent 框架适配器

```powershell
cd evals/agent
python -m venv .venv
.venv\Scripts\python -m pip install -r requirements.txt
.venv\Scripts\python -m pip install -e . --no-deps

# 不调用模型，验证五条模拟用户契约
.venv\Scripts\python -m galen_agent_eval.validate
.venv\Scripts\inspect eval galen_agent_eval/tasks.py@galen_contracts --model mockllm/model

# Inspect 编排 Galen 原生 Rust Agent Loop；先用 --limit 1 做 smoke
.venv\Scripts\inspect eval galen_agent_eval/tasks.py@galen_foundation --model mockllm/model --limit 1
```

## 四层测评架构

1. **RAG 组件层（确定性）**：固定 Evidence、查询与黄金相关 ID，不调用模型，测 Recall@K、Precision@K、MRR、nDCG@K、负样本污染、域外空结果准确率、冷建索引和热检索 P50/P95。
2. **Agent 行为层**：运行完整 Rust Agent Loop，断言是否先调用 `search_evidence`、是否错误访问外部检索、是否循环调用、是否在预算内收敛。
3. **医学交付层**：检查关键事实、证据 ID、引用、禁止内容、Artifact 是否非空且能在 Galen 内预览。
4. **版本可靠性层**：同配置重复运行，比较 Wilson 95% 下界、pass^5、Galen Agent Index、Token、TTFR 和总耗时。

组件层与 Agent 层不能互相替代：组件层失败说明检索器或语料有问题；组件层通过但 Agent 层失败，说明工具选择、提示词、上下文或交付链路有问题。

## RAG 指标与硬门

- `Recall@K`：黄金相关证据有多少进入前 K；医学关键证据不得因性能优化而丢失。
- `Precision@K`：前 K 中相关证据比例，用于观察噪声。
- `MRR`：第一条相关证据的倒数排名，反映首屏可用性。
- `nDCG@K`：综合衡量相关证据的排序位置。
- `forbidden_hits`：明确标记的跨疾病或相邻主题负样本命中数，默认必须为 0。
- `negative_query_accuracy`：对帕金森、糖尿病足等域外问题返回空结果的比例，防止系统为了召回而强行返回无关康复证据。
- `cold_index_ms`：从 Evidence 账本构建派生索引并完成首次查询的耗时。
- `latency_p50_ms / latency_p95_ms`：索引热启动后的检索延迟分布。

当前 AIS 黄金集硬门：Recall@3 = 1.0、MRR ≥ 0.90、nDCG@3 ≥ 0.90、域外空结果准确率 = 1.0、热检索 P95 ≤ 100 ms、冷索引 ≤ 2000 ms、负样本命中为 0。

## 可靠性口径

- `success_rate`：通过全部硬门的运行比例。
- `wilson_lower_95`：成功率的 Wilson 95% 置信区间下界；发布决策优先看这个保守值。
- `pass_k`：从同一 case/model/config 的运行中抽取 k 次且全部成功的概率，用于暴露偶发失败。
- `galen_agent_index`：能力 30%、重复可靠性 25%、状态安全 20%、交付 15%、效率 10% 的几何加权分。
- `qualified`：只有所有运行全部通过硬门时为 `true`。总分不能抵消隐私泄露、状态损坏、虚假交付等严重失败。

`risk_tier` 支持 `standard`、`high` 和 `critical`。高风险与关键案例在 Release Gate 中必须零硬门失败。

## 数据规则

- `cases/`：版本化的 TOML 评测契约。
- `datasets/`：确定性 RAG 黄金查询、相关证据 ID、负样本与阈值。
- `fixtures/`：只读原始输入；Runner 将其复制到临时目录，绝不原地修改。
- `runs/`：本地不可变 JSONL；保存完整最终响应、工具轨迹和临时工作区位置。
- `baselines/`：只有通过正式审核的基线才能提交。
- `reports/`：后续生成的机器可读/HTML 对比报告。

### 真实病例数据集试点

`case-datasets/ais-textbook-pilot-v1/` 保存病例 21-30 的本地科研评测试点：
10 个纵向病例、40 个任务和 40 个与隐藏黄金答案隔离的输入记录。原始
PDF、整页 OCR 与病例图片不进入数据集目录。构建与校验：

```powershell
python scripts/evals/ais_textbook_dataset.py build
python scripts/evals/ais_textbook_dataset.py validate
python scripts/evals/ais_textbook_dataset.py export-galen --split development
python -m unittest discover -s scripts/evals -p "test_ais_textbook_dataset.py"
```

数据集按 `case_id` 分组切分；同一病例的抽取、时间截断、纵向总结和科研
边界任务不得跨越 development/validation/hidden。只有 `verified` 观察值能
进入硬门，来源冲突保留为 `disputed`。

AIS 病例使用 `[structured]` 评分契约：观察值按同一行内的“观察 ID、数值/文本、
单位、容差”匹配，不再依赖固定 Markdown 或逐字前缀；科研边界使用语义断言。
T2 来源封闭任务还会检查带医学单位、Cobb/ATR/Risser 语境的数字，只允许输入
记录声明的数值。输入外阈值、随访周期和成熟度数字属于硬门失败。生成器同时在
提示中约束模型只描述待采集变量，不自行补充数值建议，并提供真实观察 ID 示例，
禁止用字面“观察ID”代替 ID。Agent Loop 对单次流事件设置 30 秒空闲看门狗；首次
中断会写入 `__stream_retry__` 轨迹并续跑一次，第二次失败才结束任务。

`rag` 命令输出单个不可覆盖的 JSON 报告，并记录 Git dirty 状态以及“查询契约 + Evidence 语料”联合哈希；`rag-compare` 只比较数据哈希一致的报告。候选的 Recall@K 不允许下降，MRR/nDCG 下降超过 0.02、负样本增加、域外空结果准确率下降或 P95 延迟恶化超过 10% 均判定为负优化。

`report` 命令不会修改原始运行记录，也拒绝覆盖已有报告。它汇总 Agent 可靠性、耗时、Token、证据检索/引用覆盖率、硬门失败，以及 RAG 逐查询排名和所有门禁，生成稳定的 Markdown 中间产物。正式对外 PDF 应由该 Markdown 和对应原始 JSON/JSONL 共同生成，避免图表与事实源脱节。

单次通过只代表链路可运行，不代表候选版本优于基线。PR Gate 每个 case/model/config 至少需要 5 次；正式 Release 基线应积累 20～30 次，才使用 P90 作稳定结论。
