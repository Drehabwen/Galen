# 上下文工程测评框架设计方案

> 状态：**已实现（v1.1）** · 2026-08-22 · 核心框架已落地并通过编译与单元测试
> 目标：让"上下文策略的好坏"成为**自动、可测量、可量化**的工程指标，并接入 PR / Release Gate。

---

## 1. 背景与目标

Galen 的上下文架构（compact.rs 压缩引擎、ResearchContextPack、GitContext）已经实现"自动压缩 + 预算控制 + 分层摘要"，但缺少闭环验证：**上下文策略改一次，效果是变好还是变坏，目前没有量化证据**。

本方案在现有 eval 骨架上增加"上下文工程"专项测评：

1. **可测量**：同一任务在不同上下文变体下跑，产出 4 个量化指标；
2. **可对比**：用现有 `compare_runs` 做 baseline vs candidate 自动判定；
3. **可门槛**：指标接入 PR Gate / Release Gate，失败即拒绝合并。

**原则：不引入新框架（不接 promptfoo/LangSmith），全部复用 Galen 现有 eval 代码。**

## 2. 现状盘点（复用清单）

| 现有组件 | 位置 | 复用方式 |
|---|---|---|
| `EvalCase`（TOML 契约） | `rust/crates/galen/src-tauri/src/eval.rs` L41 | 增加 `[context]` 变体字段 |
| `RunRecord.context` | eval.rs L191 | 已含 `compactions / required_facts / retained_facts`，召回率直接出数 |
| `RunRecord.usage` | eval.rs L165 | `input` token 用于计算节省率 |
| `compare_runs` | eval.rs L617 | baseline vs candidate → Accept/Reject + 各 delta |
| `ReliabilityReport` | eval.rs L521 | success_rate / wilson_lower_95 / pass_k / agent_index |
| CLI | `cargo run -p galen --bin eval` | run / compare / reliability / validate |
| 压缩引擎 | `rust/crates/runtime/src/compact.rs` | runner 直接调用 `compact_session` 构造压缩变体 |
| 上下文包 | docs/galen-llm-context-tools.md | `build_research_context_pack` 构造全包变体 |

## 3. 核心设计：上下文变体（ContextVariant）

### 3.1 枚举定义

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextVariant {
    /// 现状：默认上下文（基线）
    None,
    /// 压缩引擎处理后的会话（compact_session 输出）
    Compacted,
    /// 科研 5 层上下文包（ResearchContextPack）
    FullPack,
    /// 仅摘要骨架（Scope/Work/Key files 8 字段，无时间线）
    SkeletonOnly,
}
```

### 3.2 EvalCase 扩展（向后兼容）

`EvalCase` 增加可选字段，缺省 = `None`，现有 case 不受影响：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextVariantSpec {
    pub variant: ContextVariant,          // 默认 None
    pub preserve_recent: Option<usize>,   // compact 保留尾部条数（默认 8）
    pub max_tokens: Option<usize>,        // 压缩阈值（默认 50_000，小于运行时 100K 以便测试可复现）
    pub require_fields: Option<Vec<String>>, // 摘要骨架必留字段（E12 用）
}
```

### 3.3 case TOML 示例

```toml
# evals/cases/e11_task_success_after_compaction.toml
schema_version = 1
id = "E11"
name = "压缩后任务成功率保留"
suite = "context-engineering"
risk_tier = "high"
prompt = "根据工作区记忆继续拟定研究方案，生成 output/context-check.md。必须原样保留样本量 48、主要结局 FMA-UE、随机 12 周。"
fixture = "fixtures/e11"
timeout_seconds = 300
max_model_requests = 6
max_tool_calls = 10
max_human_interventions = 0

[context]
variant = "compacted"
preserve_recent = 8
max_tokens = 50000

[required]
facts = ["48", "FMA-UE", "12 周"]
artifacts = ["output/context-check.md"]

[forbidden]
repeated_call_limit = 2
response_patterns = ["样本量 28", "主要结局 Barthel"]
```

## 4. 量化指标（4 个，全部可自动计算）

| # | 指标 | 公式 | 数据来源 | 新增代码 |
|---|---|---|---|---|
| M1 | **任务成功率保留度** ⭐ | `success_rate(compacted) ÷ success_rate(none)` | compare_runs 的 success_rate | 无（直接对比） |
| M2 | **信息召回率** | `retained_facts ÷ required_facts` | RunRecord.context | 无（已记录） |
| M3 | **摘要字段覆盖率** | 摘要中 8 骨架字段出现数 ÷ 8 | 解析 compacted 摘要文本 | 新增 1 个断言函数 |
| M4 | **token 节省率** | `1 − input(compacted) ÷ input(none)` | RunRecord.usage.input | 无（已记录） |

8 骨架字段白名单（与 `summary_compression.rs::is_core_detail` 对齐）：

```
Scope / Current work / Pending work / Key files referenced
Tools mentioned / Recent user requests / Previously compacted context / Newly compacted context
```

M3 断言实现（eval.rs 新增）：

```rust
fn summary_field_coverage(summary: &str) -> (usize, usize) {
    const FIELDS: [&str; 8] = [
        "- Scope:", "- Current work:", "- Pending work:", "- Key files referenced:",
        "- Tools mentioned:", "- Recent user requests:",
        "- Previously compacted context:", "- Newly compacted context:",
    ];
    let hit = FIELDS.iter().filter(|f| summary.contains(**f)).count();
    (hit, FIELDS.len())
}
```

## 5. case 套件（suite = "context-engineering"）

| Case | 变体 | 测什么 | Gate 用途 |
|---|---|---|---|
| E07（已有） | none | 关键事实保留（信息召回基线） | 回归 |
| **E11** | none vs compacted | 压缩后任务成功率保留度（M1） | 核心门槛 |
| **E12** | compacted | 摘要骨架 8 字段覆盖率（M3） | 核心门槛 |
| **E13** | compacted ×3 轮 | 连续多次压缩不丢信息（merge 分层正确性） | 回归 |
| **E14** | compacted | 压缩边界不拆散 ToolUse/ToolResult 对（回归 compact.rs 400 bug） | 回归 |

E13 实现要点：runner 先压缩一次 → 继续追加对话 → 再压缩，共 3 轮，最后检查 required facts 仍在。

E14 实现要点：fixture 提供一段含 ToolUse→ToolResult 的 seed 消息，强制压缩边界落在配对中间，断言 run 不报错且事实保留。

## 6. runner 改动点

`eval run` 构造 agent 上下文处增加分支（复用现有 compact_session，不新写压缩逻辑）：

```rust
fn build_context_messages(case: &EvalCase, session: &Session) -> Vec<ConversationMessage> {
    let spec = case.context.as_ref();
    match spec.map(|s| s.variant) {
        Some(ContextVariant::Compacted) | Some(ContextVariant::SkeletonOnly) => {
            let cfg = CompactionConfig {
                max_estimated_tokens: spec.max_tokens.unwrap_or(50_000),
                preserve_recent_messages: spec.preserve_recent.unwrap_or(8),
            };
            let result = compact_session(session, cfg);
            result.compacted_session.messages
        }
        Some(ContextVariant::FullPack) => build_research_context_pack(case),
        _ => session.messages.clone(),
    }
}
```

要点：

1. **同一 prompt / 同一 fixture，只变上下文生成方式**——跑出的差异即上下文策略差异，其余变量全部锁死；
2. `config_hash` 必须把 variant 掺入（一行改动），保证 compare 按 case+model+config 正确分组；
3. seed 会话构造：为压缩变体预填充超过阈值的上下文消息（用 fixture 提供）。

## 7. Gate 规则（写入 compare_runs 的 reason 判定）

```text
Accept 需要同时满足：
  ① M1 任务成功率保留度 ≥ 0.90    （压缩后成功率掉不超过 10%）
  ② M3 字段覆盖率 ≥ 6/8            （骨架完整性）
  ③ M4 token 节省率 ≥ 30%          （压缩确实省钱）
  ④ candidate 所有 hard_gates 通过  （现有逻辑，不含糊）
```

CLI 用法：

```powershell
cd rust
# 基线：原始上下文 ×5
cargo run -p galen --bin eval -- run --case E11 --repeat 5 --output ../evals/runs/e11-none.jsonl
# 候选：压缩后上下文 ×5
cargo run -p galen --bin eval -- run --case E11 --repeat 5 --output ../evals/runs/e11-compacted.jsonl
# 对比 → Accept/Reject
cargo run -p galen --bin eval -- compare --baseline ../evals/runs/e11-none.jsonl --candidate ../evals/runs/e11-compacted.jsonl
```

## 8. 报告与趋势

- 每次 PR 自动跑 E11/E12 → `evals/reports/context-engineering-{date}.json`（机器可读）+ HTML 三线图（保留度 / 覆盖率 / 节省率）；
- 累积后即可回答：**"改一次压缩策略，三个指标动没动、动多少"**；
- Release 基线沿用现有约定：20-30 次运行，用 P90 下结论；E11 保留度 P90 ≥ 0.9。

## 9. 实施顺序与工作量

| 步骤 | 内容 | 工作量 |
|---|---|---|
| 1 | `ContextVariant` + `EvalCase.context` 字段（serde 向后兼容） | 小（~0.5h） |
| 2 | runner 按 variant 构造上下文 + config_hash 掺入 variant | 中（~2h） |
| 3 | M3 字段覆盖率断言函数 + E12 case | 小（~1h） |
| 4 | E11 / E13 / E14 case + fixtures | 中（~2h） |
| 5 | compare_runs 增加 4 条 Gate reason | 小（~0.5h） |
| 6 | 报告输出（json + HTML 趋势） | 中（~2h） |
| **合计** | | **约 1-2 天** |

## 10. 风险与注意

1. **种子会话的构造**是压缩变体可复现的关键——fixture 固定 seed 消息，禁止随机生成；
2. `estimate_message_tokens` 是字节/4 的粗估——E11 的 `max_tokens` 阈值定 50K 是为保证稳定触发压缩，不要用运行时 100K（不可复现）；
3. M1 的分子分母都要求 ≥5 次运行（沿用 compare_runs 现有约束），单次通过不代表优于基线；
4. E14 是回归防线：compact.rs 的边界回退逻辑（L125-161）必须有 case 常驻，防止未来改动重新引入 400 错误。

---

## 11. 实现记录（v1.1，2026-08-22）

已落地（代码 + 验证）：

| 项 | 位置 | 验证 |
|---|---|---|
| ContextVariant / ContextSpec / summary_field_coverage | `rust/crates/galen/src-tauri/src/eval.rs` | 单元测试 8/8 通过 |
| EvalCase.context（TOML `[context]` 段，向后兼容） | 同上 | `eval validate` 10 case 全 OK |
| E11 / E12 case + fixtures | `evals/cases/e11_*.toml`、`e12_*.toml`、`evals/fixtures/e11|e12/` | validate 通过 |
| runner 变体构造（seed 会话 → compact_session → history 注入） | `rust/crates/galen/src-tauri/src/bin/eval.rs` | cargo check 通过 |
| --variant CLI（none/compacted/skeleton/fullpack） | 同上 | 同上 |
| M3 覆盖率断言（context_field_coverage） | eval.rs RunRecord::evaluate | 测试覆盖 |
| compare 上下文 Gate（M1/M3/M4） | eval.rs compare_runs | context_gate_* 测试覆盖 |
| config_hash 掺入 variant | eval.rs | 保证按 case+model+config 正确分组 |

用法：

```powershell
cd rust
# 基线（原始上下文）与候选（压缩后）各 5 次
cargo run -p galen --bin eval -- run --case E11 --repeat 5 --output ../evals/runs/e11-none.jsonl
cargo run -p galen --bin eval -- run --case E11 --repeat 5 --output ../evals/runs/e11-compacted.jsonl
# 对比（上下文 Gate 自动启用：保留度 ≥90% / 覆盖率 ≥6-8 / 节省率 ≥30%）
cargo run -p galen --bin eval -- compare --baseline ../evals/runs/e11-none.jsonl --candidate ../evals/runs/e11-compacted.jsonl
```

待办：FullPack 变体暂与 Compacted 同路径（ResearchContextPack 后续接入）；E13（三轮压缩）/ E14（边界 400 回归）case 待建。
