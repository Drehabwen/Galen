//! Reproducible evaluation contracts and baseline comparison for Galen.
//!
//! The evaluator deliberately scores observable state (tool traces, files,
//! timings and usage) before any model-based rubric is considered.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::backend::{ChatRunSummary, ToolTrace};

fn default_schema_version() -> u32 {
    1
}

fn default_timeout_seconds() -> u64 {
    300
}

fn default_max_model_requests() -> u32 {
    8
}

fn default_max_tool_calls() -> usize {
    12
}

fn default_repeat_limit() -> usize {
    2
}

fn default_risk_tier() -> String {
    "standard".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCase {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub suite: String,
    #[serde(default = "default_risk_tier")]
    pub risk_tier: String,
    pub prompt: String,
    #[serde(default)]
    pub fixture: Option<String>,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default = "default_max_model_requests")]
    pub max_model_requests: u32,
    #[serde(default = "default_max_tool_calls")]
    pub max_tool_calls: usize,
    #[serde(default)]
    pub max_human_interventions: u32,
    #[serde(default)]
    pub required: RequiredOutcome,
    #[serde(default)]
    pub forbidden: ForbiddenOutcome,
    #[serde(default)]
    pub structured: StructuredOutcome,
    #[serde(default)]
    pub context: ContextSpec,
}

/// 上下文变体：同一任务在不同上下文构造方式下运行，用于量化上下文策略的影响。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContextVariant {
    /// 默认上下文（基线，无历史注入）
    #[default]
    None,
    /// 完整 seed 上下文（不压缩，作为压缩的真实对照）
    Full,
    /// 压缩引擎（compact_session）处理后的会话
    Compacted,
    /// 科研 5 层上下文包（ResearchContextPack）
    FullPack,
    /// 仅摘要骨架（无保留尾部）
    SkeletonOnly,
}

/// 上下文工程测评的变体规格（TOML `[context]` 段，缺省 = None，向后兼容）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextSpec {
    #[serde(default)]
    pub variant: ContextVariant,
    /// compact 保留尾部消息数（默认 8）
    #[serde(default)]
    pub preserve_recent: Option<usize>,
    /// 压缩触发阈值 token（默认 50_000，保证测试可复现）
    #[serde(default)]
    pub max_tokens: Option<usize>,
    /// 摘要骨架必留字段（默认全部 8 个）
    #[serde(default)]
    pub require_fields: Vec<String>,
}

/// Galen 压缩摘要的固定骨架字段（与 summary_compression.rs::is_core_detail 对齐）。
pub const CONTEXT_SUMMARY_FIELDS: [&str; 8] = [
    "- Scope:",
    "- Current work:",
    "- Pending work:",
    "- Key files referenced:",
    "- Tools mentioned:",
    "- Recent user requests:",
    "- Previously compacted context:",
    "- Newly compacted context:",
];

/// 计算摘要骨架字段覆盖率（hit, total）。
#[must_use]
pub fn summary_field_coverage(summary: &str, require_fields: &[String]) -> (usize, usize) {
    if require_fields.is_empty() {
        let hit = CONTEXT_SUMMARY_FIELDS
            .iter()
            .filter(|field| summary.contains(**field))
            .count();
        (hit, CONTEXT_SUMMARY_FIELDS.len())
    } else {
        let hit = require_fields
            .iter()
            .filter(|field| summary.contains(field.as_str()))
            .count();
        (hit, require_fields.len())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequiredOutcome {
    #[serde(default)]
    pub facts: Vec<String>,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    /// Exact ordered sequence of ordinary tool calls. Use this for recovery
    /// contracts where arguments, failures, and ordering are part of success.
    #[serde(default)]
    pub tool_sequence: Vec<ExpectedToolCall>,
    /// Evidence IDs that must be both retrieved and cited in the final output.
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedToolCall {
    pub tool: String,
    #[serde(default)]
    pub input_contains: String,
    #[serde(default)]
    pub is_error: Option<bool>,
}

/// Machine-checkable content expectations. These assertions deliberately
/// tolerate harmless prose/Markdown variation while preserving numeric and
/// source-grounding hard gates.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StructuredOutcome {
    #[serde(default)]
    pub observations: Vec<ExpectedObservation>,
    #[serde(default)]
    pub source_closed_degree_values: bool,
    #[serde(default)]
    pub allowed_degree_values: Vec<f64>,
    #[serde(default)]
    pub source_closed_numeric_values: bool,
    #[serde(default)]
    pub allowed_numeric_values: Vec<f64>,
    #[serde(default)]
    pub require_causal_boundary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedObservation {
    pub id: String,
    pub value: ExpectedObservationValue,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub tolerance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExpectedObservationValue {
    Number(f64),
    Text(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForbiddenOutcome {
    #[serde(default = "default_repeat_limit")]
    pub repeated_call_limit: usize,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub response_patterns: Vec<String>,
    /// Evidence IDs that must neither be retrieved nor cited.
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

impl Default for ForbiddenOutcome {
    fn default() -> Self {
        Self {
            repeated_call_limit: default_repeat_limit(),
            tools: Vec::new(),
            response_patterns: Vec::new(),
            evidence_ids: Vec::new(),
        }
    }
}

impl EvalCase {
    pub fn from_path(path: &Path) -> Result<Self, String> {
        let source = std::fs::read_to_string(path)
            .map_err(|error| format!("读取评测案例 {} 失败: {error}", path.display()))?;
        let case: Self = toml::from_str(&source)
            .map_err(|error| format!("解析评测案例 {} 失败: {error}", path.display()))?;
        case.validate()?;
        Ok(case)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!("案例 {} 使用不支持的 schema_version", self.id));
        }
        if self.id.trim().is_empty() || self.name.trim().is_empty() || self.prompt.trim().is_empty()
        {
            return Err("评测案例 id、name 和 prompt 均不能为空".to_string());
        }
        if self.max_model_requests == 0 || self.max_tool_calls == 0 || self.timeout_seconds == 0 {
            return Err(format!("案例 {} 的运行预算必须大于 0", self.id));
        }
        if !matches!(self.risk_tier.as_str(), "standard" | "high" | "critical") {
            return Err(format!(
                "案例 {} 的 risk_tier 必须是 standard、high 或 critical",
                self.id
            ));
        }
        if self.forbidden.repeated_call_limit == 0 {
            return Err(format!("案例 {} 的重复调用上限必须大于 0", self.id));
        }
        for observation in &self.structured.observations {
            if observation.id.trim().is_empty()
                || !observation.tolerance.is_finite()
                || observation.tolerance < 0.0
            {
                return Err(format!("案例 {} 的结构化观察配置无效", self.id));
            }
            match &observation.value {
                ExpectedObservationValue::Number(value) if !value.is_finite() => {
                    return Err(format!("案例 {} 的结构化观察数值必须有限", self.id));
                }
                ExpectedObservationValue::Text(value) if value.trim().is_empty() => {
                    return Err(format!("案例 {} 的结构化观察文本不能为空", self.id));
                }
                _ => {}
            }
        }
        for call in &self.required.tool_sequence {
            if call.tool.trim().is_empty() {
                return Err(format!("案例 {} 的工具序列包含空工具名", self.id));
            }
        }
        if self
            .structured
            .allowed_degree_values
            .iter()
            .chain(self.structured.allowed_numeric_values.iter())
            .any(|value| !value.is_finite())
        {
            return Err(format!("案例 {} 的允许数值必须有限", self.id));
        }
        for artifact in &self.required.artifacts {
            let path = Path::new(artifact);
            if path.is_absolute()
                || path
                    .components()
                    .any(|part| matches!(part, std::path::Component::ParentDir))
            {
                return Err(format!("案例 {} 包含不安全的 Artifact 路径", self.id));
            }
        }
        if self
            .required
            .evidence_ids
            .iter()
            .any(|id| id.trim().is_empty() || self.forbidden.evidence_ids.contains(id))
            || self
                .forbidden
                .evidence_ids
                .iter()
                .any(|id| id.trim().is_empty())
        {
            return Err(format!(
                "案例 {} 的必需与禁止 evidence_ids 必须非空且不能重叠",
                self.id
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub name: String,
    pub pass: bool,
    pub hard_gate: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvalLatency {
    pub context_ms: u64,
    pub mcp_ms: u64,
    pub ttft_ms: Option<u64>,
    pub ttfr_ms: Option<u64>,
    pub total_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvalUsage {
    pub input: u64,
    pub output: u64,
    pub cache_create: u64,
    pub cache_read: u64,
}

impl EvalUsage {
    pub fn total(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_create)
            .saturating_add(self.cache_read)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvalTools {
    pub calls: usize,
    pub errors: usize,
    pub max_repeat: usize,
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvalArtifacts {
    pub required: usize,
    pub valid: usize,
    pub previewable: usize,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvalContext {
    pub compactions: u32,
    pub required_facts: usize,
    pub retained_facts: usize,
    #[serde(default)]
    pub summary_field_coverage: Option<(usize, usize)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvalGrounding {
    pub required_evidence: usize,
    pub retrieved_required: usize,
    pub cited_required: usize,
    pub forbidden_hits: usize,
    pub local_search_calls: usize,
    pub external_search_calls: usize,
    pub retrieved_evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroundingSummary {
    pub runs_with_required_evidence: usize,
    pub required_evidence: usize,
    pub retrieved_required: usize,
    pub cited_required: usize,
    pub retrieval_coverage: Option<f64>,
    pub citation_coverage: Option<f64>,
    pub forbidden_hits: usize,
    pub local_search_calls: usize,
    pub external_search_calls: usize,
}

/// Five observable dimensions used by the Galen Agent Index.  These values
/// are deliberately computed from traces and workspace state rather than an
/// unvalidated model judge.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReliabilityDimensions {
    pub capability: f64,
    pub repeatability: f64,
    pub state_safety: f64,
    pub delivery: f64,
    pub efficiency: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub schema_version: u32,
    pub run_id: String,
    pub case_id: String,
    pub commit: String,
    pub model: String,
    pub config_hash: String,
    pub run_index: u32,
    pub started_at_ms: u128,
    pub workspace: String,
    pub final_response: String,
    pub hard_gates_passed: bool,
    pub quality_score: f64,
    #[serde(default)]
    pub dimensions: ReliabilityDimensions,
    pub latency: EvalLatency,
    pub usage: EvalUsage,
    pub model_requests: u32,
    pub tools: EvalTools,
    pub tool_trace: Vec<ToolTrace>,
    pub context: EvalContext,
    #[serde(default)]
    pub grounding: EvalGrounding,
    pub artifacts: EvalArtifacts,
    pub assertions: Vec<AssertionResult>,
}

pub struct RunObservation<'a> {
    pub commit: &'a str,
    pub model: &'a str,
    pub run_index: u32,
    pub run_ok: bool,
    pub response: &'a str,
    pub summary: &'a ChatRunSummary,
    pub traces: &'a [ToolTrace],
    pub workspace: &'a Path,
    pub summary_field_coverage: Option<(usize, usize)>,
}

impl RunRecord {
    pub fn evaluate(case: &EvalCase, observation: RunObservation<'_>) -> Self {
        let mut assertions = Vec::new();
        let mut add = |name: String, pass: bool, detail: String| {
            assertions.push(AssertionResult {
                name,
                pass,
                hard_gate: true,
                detail,
            });
        };

        add(
            "run_completed".to_string(),
            observation.run_ok,
            if observation.run_ok { "Ok" } else { "Err" }.to_string(),
        );

        let ordinary_traces: Vec<&ToolTrace> = observation
            .traces
            .iter()
            .filter(|trace| trace.tool != "__convergence__")
            .collect();
        let errors = ordinary_traces
            .iter()
            .filter(|trace| trace.is_error)
            .count();
        let expected_errors = case
            .required
            .tool_sequence
            .iter()
            .filter(|call| call.is_error == Some(true))
            .count();
        let unexpected_errors = errors.saturating_sub(expected_errors);
        let mut repeats: HashMap<(&str, &str), usize> = HashMap::new();
        let mut names = Vec::new();
        for trace in &ordinary_traces {
            *repeats
                .entry((trace.tool.as_str(), trace.input.as_str()))
                .or_default() += 1;
            if !names.contains(&trace.tool) {
                names.push(trace.tool.clone());
            }
        }
        let retrieved_evidence_ids = retrieved_evidence_ids(&ordinary_traces);
        let local_search_calls = ordinary_traces
            .iter()
            .filter(|trace| trace.tool == "search_evidence")
            .count();
        let external_search_calls = ordinary_traces
            .iter()
            .filter(|trace| {
                matches!(
                    trace.tool.as_str(),
                    "search_pubmed" | "search_rehab_literature"
                )
            })
            .count();
        let max_repeat = repeats.values().copied().max().unwrap_or_default();
        add(
            "model_request_budget".to_string(),
            observation.summary.model_request_count <= case.max_model_requests,
            format!(
                "{}/{}",
                observation.summary.model_request_count, case.max_model_requests
            ),
        );
        add(
            "tool_call_budget".to_string(),
            ordinary_traces.len() <= case.max_tool_calls,
            format!("{}/{}", ordinary_traces.len(), case.max_tool_calls),
        );
        add(
            "no_repeated_call_loop".to_string(),
            max_repeat <= case.forbidden.repeated_call_limit,
            format!(
                "max_repeat={max_repeat}, limit={}",
                case.forbidden.repeated_call_limit
            ),
        );

        for tool in &case.required.tools {
            add(
                format!("required_tool:{tool}"),
                names.contains(tool),
                format!("observed={}", names.join(",")),
            );
        }
        if !case.required.tool_sequence.is_empty() {
            let expected = &case.required.tool_sequence;
            let exact = ordinary_traces.len() == expected.len()
                && ordinary_traces
                    .iter()
                    .zip(expected)
                    .all(|(actual, wanted)| {
                        actual.tool == wanted.tool
                            && (wanted.input_contains.is_empty()
                                || actual.input.contains(&wanted.input_contains))
                            && wanted.is_error.is_none_or(|value| actual.is_error == value)
                    });
            let observed = ordinary_traces
                .iter()
                .map(|trace| format!("{}:{}:{}", trace.tool, trace.is_error, trace.input))
                .collect::<Vec<_>>()
                .join(" -> ");
            add(
                "required_tool_sequence".to_string(),
                exact,
                format!("observed={observed}"),
            );
        }
        for tool in &case.forbidden.tools {
            add(
                format!("forbidden_tool:{tool}"),
                !names.contains(tool),
                format!("observed={}", names.join(",")),
            );
        }

        let searchable = searchable_output(observation.workspace, observation.response);
        let mut retained_facts = 0;
        for fact in &case.required.facts {
            let pass = searchable.contains(fact);
            retained_facts += usize::from(pass);
            add(
                format!("required_fact:{fact}"),
                pass,
                if pass { "retained" } else { "missing" }.to_string(),
            );
        }
        for observation in &case.structured.observations {
            let result = match_expected_observation(&searchable, observation);
            retained_facts += usize::from(result.pass);
            add(
                format!("structured_observation:{}", observation.id),
                result.pass,
                result.details,
            );
        }
        if case.structured.require_causal_boundary {
            let pass = contains_causal_boundary(&searchable);
            retained_facts += usize::from(pass);
            add(
                "causal_boundary".to_string(),
                pass,
                if pass {
                    "found semantic single-case causal/effectiveness limitation"
                } else {
                    "missing semantic single-case causal/effectiveness limitation"
                }
                .to_string(),
            );
        }
        if case.structured.source_closed_degree_values {
            let unsupported = unsupported_degree_values(&searchable, &case.structured);
            add(
                "source_closed_degree_values".to_string(),
                unsupported.is_empty(),
                if unsupported.is_empty() {
                    "all reported degree values are source-backed".to_string()
                } else {
                    format!("unsupported degree values: {unsupported:?}")
                },
            );
        }
        if case.structured.source_closed_numeric_values {
            let unsupported = unsupported_medical_numeric_values(&searchable, &case.structured);
            add(
                "source_closed_numeric_values".to_string(),
                unsupported.is_empty(),
                if unsupported.is_empty() {
                    "all medical numeric claims are source-backed".to_string()
                } else {
                    format!("unsupported medical numeric values: {unsupported:?}")
                },
            );
        }
        let mut retrieved_required = 0;
        let mut cited_required = 0;
        for evidence_id in &case.required.evidence_ids {
            let retrieved = retrieved_evidence_ids.contains(evidence_id);
            let cited = searchable.contains(evidence_id);
            retrieved_required += usize::from(retrieved);
            cited_required += usize::from(cited);
            add(
                format!("required_evidence_retrieved:{evidence_id}"),
                retrieved,
                if retrieved { "retrieved" } else { "missing" }.to_string(),
            );
            add(
                format!("required_evidence_cited:{evidence_id}"),
                cited,
                if cited { "cited" } else { "not cited" }.to_string(),
            );
        }
        let mut forbidden_evidence_hits = 0;
        for evidence_id in &case.forbidden.evidence_ids {
            let retrieved = retrieved_evidence_ids.contains(evidence_id);
            let cited = searchable.contains(evidence_id);
            forbidden_evidence_hits += usize::from(retrieved || cited);
            add(
                format!("forbidden_evidence:{evidence_id}"),
                !retrieved && !cited,
                format!("retrieved={retrieved}, cited={cited}"),
            );
        }
        for pattern in &case.forbidden.response_patterns {
            add(
                format!("forbidden_response_pattern:{pattern}"),
                !observation.response.contains(pattern),
                "checked final response".to_string(),
            );
        }

        if let Some((hit, total)) = observation.summary_field_coverage {
            let threshold = if case.context.require_fields.is_empty() {
                (total * 6 / 8).max(1)
            } else {
                total
            };
            add(
                "context_field_coverage".to_string(),
                hit >= threshold,
                format!("{hit}/{total} 摘要骨架字段保留（阈值 {threshold}）"),
            );
        }

        let mut valid_artifacts = 0;
        let mut previewable = 0;
        let mut artifact_files = Vec::new();
        for artifact in &case.required.artifacts {
            let path = observation.workspace.join(artifact);
            let valid = path
                .metadata()
                .map(|metadata| metadata.is_file() && metadata.len() > 0)
                .unwrap_or(false);
            if valid {
                valid_artifacts += 1;
                artifact_files.push(artifact.clone());
                if is_previewable(&path) {
                    previewable += 1;
                }
            }
            add(
                format!("required_artifact:{artifact}"),
                valid,
                if valid {
                    "present and non-empty"
                } else {
                    "missing or empty"
                }
                .to_string(),
            );
            add(
                format!("previewable_artifact:{artifact}"),
                valid && is_previewable(&path),
                if valid && is_previewable(&path) {
                    "supported by Galen preview"
                } else {
                    "missing or unsupported preview format"
                }
                .to_string(),
            );
        }

        let hard_gates_passed = assertions.iter().all(|assertion| assertion.pass);
        let quality_score = if assertions.is_empty() {
            0.0
        } else {
            assertions.iter().filter(|assertion| assertion.pass).count() as f64
                / assertions.len() as f64
        };
        let capability = assertion_score(&assertions, |name| {
            name == "run_completed"
                || name.starts_with("required_fact:")
                || name.starts_with("structured_observation:")
                || name == "causal_boundary"
                || name.starts_with("required_tool:")
                || name == "required_tool_sequence"
                || name.starts_with("required_evidence_")
        });
        let repeatability = assertion_score(&assertions, |name| {
            matches!(
                name,
                "model_request_budget" | "tool_call_budget" | "no_repeated_call_loop"
            )
        }) * if unexpected_errors == 0 { 1.0 } else { 0.8 };
        let state_safety = assertion_score(&assertions, |name| {
            name == "run_completed"
                || name.starts_with("forbidden_tool:")
                || name.starts_with("forbidden_response_pattern:")
                || name.starts_with("forbidden_evidence:")
                || name == "source_closed_degree_values"
                || name == "source_closed_numeric_values"
        });
        let delivery = if case.required.artifacts.is_empty() {
            1.0
        } else {
            (valid_artifacts + previewable) as f64 / (case.required.artifacts.len() * 2) as f64
        };
        let efficiency = mean(
            [
                budget_score(
                    observation.summary.model_request_count as usize,
                    case.max_model_requests as usize,
                ),
                budget_score(ordinary_traces.len(), case.max_tool_calls),
                budget_score(
                    observation.summary.total_ms as usize,
                    case.timeout_seconds.saturating_mul(1_000) as usize,
                ),
            ]
            .into_iter(),
        );
        let dimensions = ReliabilityDimensions {
            capability,
            repeatability,
            state_safety,
            delivery,
            efficiency,
        };
        let started_at_ms = now_millis();
        let case_config = serde_json::to_string(case).unwrap_or_else(|_| case.id.clone());
        let config_hash = stable_hash(&format!("{}|{case_config}", observation.model));

        Self {
            schema_version: 1,
            run_id: format!("{}-{}-{}", case.id, started_at_ms, observation.run_index),
            case_id: case.id.clone(),
            commit: observation.commit.to_string(),
            model: observation.model.to_string(),
            config_hash,
            run_index: observation.run_index,
            started_at_ms,
            workspace: observation.workspace.display().to_string(),
            final_response: observation.response.to_string(),
            hard_gates_passed,
            quality_score,
            dimensions,
            latency: EvalLatency {
                context_ms: observation.summary.context_assembly_ms,
                mcp_ms: observation.summary.mcp_setup_ms,
                ttft_ms: observation.summary.ttft_ms,
                ttfr_ms: observation.summary.ttfr_ms,
                total_ms: observation.summary.total_ms,
            },
            usage: EvalUsage {
                input: observation.summary.input_tokens,
                output: observation.summary.output_tokens,
                cache_create: observation.summary.cache_creation_input_tokens,
                cache_read: observation.summary.cache_read_input_tokens,
            },
            model_requests: observation.summary.model_request_count,
            tools: EvalTools {
                calls: ordinary_traces.len(),
                errors,
                max_repeat,
                names,
            },
            tool_trace: observation.traces.to_vec(),
            context: EvalContext {
                compactions: observation.summary.compaction_count,
                required_facts: case.required.facts.len()
                    + case.structured.observations.len()
                    + usize::from(case.structured.require_causal_boundary),
                retained_facts,
                summary_field_coverage: observation.summary_field_coverage,
            },
            grounding: EvalGrounding {
                required_evidence: case.required.evidence_ids.len(),
                retrieved_required,
                cited_required,
                forbidden_hits: forbidden_evidence_hits,
                local_search_calls,
                external_search_calls,
                retrieved_evidence_ids,
            },
            artifacts: EvalArtifacts {
                required: case.required.artifacts.len(),
                valid: valid_artifacts,
                previewable,
                files: artifact_files,
            },
            assertions,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseReliabilityReport {
    pub case_id: String,
    pub model: String,
    pub config_hash: String,
    pub runs: usize,
    pub successes: usize,
    pub success_rate: f64,
    pub wilson_lower_95: f64,
    pub pass_k: Option<f64>,
    pub quality_mean: f64,
    pub dimensions: ReliabilityDimensions,
    pub grounding: GroundingSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReliabilityReport {
    pub qualified: bool,
    pub runs: usize,
    pub successes: usize,
    pub hard_gate_failures: usize,
    pub success_rate: f64,
    pub wilson_lower_95: f64,
    pub pass_k_k: usize,
    pub pass_k: Option<f64>,
    pub galen_agent_index: f64,
    pub dimensions: ReliabilityDimensions,
    pub grounding: GroundingSummary,
    pub cases: Vec<CaseReliabilityReport>,
}

/// Aggregate immutable run records into a reliability profile.  `pass^k` is
/// the probability that k draws without replacement are all successful; it
/// exposes intermittent failures that pass@1 and averages hide.
pub fn reliability_report(records: &[RunRecord], k: usize) -> ReliabilityReport {
    let k = k.max(1);
    let mut groups: BTreeMap<(String, String, String), Vec<&RunRecord>> = BTreeMap::new();
    for record in records {
        groups
            .entry((
                record.case_id.clone(),
                record.model.clone(),
                record.config_hash.clone(),
            ))
            .or_default()
            .push(record);
    }

    let cases = groups
        .into_iter()
        .map(|((case_id, model, config_hash), runs)| {
            let successes = runs.iter().filter(|run| run.hard_gates_passed).count();
            let dimensions = mean_dimensions(runs.iter().copied());
            let grounding = grounding_summary(runs.iter().copied());
            CaseReliabilityReport {
                case_id,
                model,
                config_hash,
                runs: runs.len(),
                successes,
                success_rate: rate(successes, runs.len()),
                wilson_lower_95: wilson_lower_95(successes, runs.len()),
                pass_k: pass_k_probability(successes, runs.len(), k),
                quality_mean: mean(runs.iter().map(|run| run.quality_score)),
                dimensions,
                grounding,
            }
        })
        .collect::<Vec<_>>();

    let successes = records.iter().filter(|run| run.hard_gates_passed).count();
    let mut dimensions = mean_dimensions(records.iter());
    // Repeated reliability is a property of distributions, not a single run.
    // Use the conservative confidence lower bound across case/config groups.
    dimensions.repeatability = if cases.is_empty() {
        0.0
    } else {
        mean(cases.iter().map(|case| case.wilson_lower_95))
    };
    let pass_values = cases
        .iter()
        .filter_map(|case| case.pass_k)
        .collect::<Vec<_>>();
    let pass_k = (!pass_values.is_empty()).then(|| mean(pass_values.into_iter()));
    let galen_agent_index = geometric_agent_index(&dimensions);
    let hard_gate_failures = records.len().saturating_sub(successes);
    let grounding = grounding_summary(records.iter());

    ReliabilityReport {
        qualified: !records.is_empty() && hard_gate_failures == 0,
        runs: records.len(),
        successes,
        hard_gate_failures,
        success_rate: rate(successes, records.len()),
        wilson_lower_95: wilson_lower_95(successes, records.len()),
        pass_k_k: k,
        pass_k,
        galen_agent_index,
        dimensions,
        grounding,
        cases,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonDecision {
    Accept,
    Reject,
    InsufficientData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub decision: ComparisonDecision,
    pub reasons: Vec<String>,
    pub baseline_runs: usize,
    pub candidate_runs: usize,
    pub quality_delta_points: f64,
    pub ttfr_p50_change: Option<f64>,
    pub ttfr_p90_change: Option<f64>,
    pub total_p50_change: Option<f64>,
    pub token_change: Option<f64>,
    pub tool_error_rate_change: Option<f64>,
    pub baseline_max_repeat: usize,
    pub candidate_max_repeat: usize,
    pub baseline_reliability_lower_95: f64,
    pub candidate_reliability_lower_95: f64,
    pub baseline_pass_5: Option<f64>,
    pub candidate_pass_5: Option<f64>,
    pub baseline_agent_index: f64,
    pub candidate_agent_index: f64,
}

pub fn compare_runs(baseline: &[RunRecord], candidate: &[RunRecord]) -> ComparisonReport {
    compare_runs_impl(baseline, candidate, false)
}

/// 忽略 config_hash 的对比：用于同一 case 不同上下文变体（如 none vs compacted）的消融对比。
#[must_use]
pub fn compare_runs_ignore_config(
    baseline: &[RunRecord],
    candidate: &[RunRecord],
) -> ComparisonReport {
    compare_runs_impl(baseline, candidate, true)
}

fn compare_runs_impl(
    baseline: &[RunRecord],
    candidate: &[RunRecord],
    ignore_config: bool,
) -> ComparisonReport {
    let mut reasons = Vec::new();
    let baseline_keys = dataset_keys(baseline, ignore_config);
    let candidate_keys = dataset_keys(candidate, ignore_config);
    let insufficient_per_case = baseline_keys
        .values()
        .chain(candidate_keys.values())
        .any(|count| *count < 5);
    if baseline_keys.keys().collect::<BTreeSet<_>>()
        != candidate_keys.keys().collect::<BTreeSet<_>>()
        || insufficient_per_case
    {
        reasons.push(
            "基线与候选必须包含相同的 case/model/config，且每个组合至少运行 5 次".to_string(),
        );
        return ComparisonReport {
            decision: ComparisonDecision::InsufficientData,
            reasons,
            baseline_runs: baseline.len(),
            candidate_runs: candidate.len(),
            quality_delta_points: 0.0,
            ttfr_p50_change: None,
            ttfr_p90_change: None,
            total_p50_change: None,
            token_change: None,
            tool_error_rate_change: None,
            baseline_max_repeat: 0,
            candidate_max_repeat: 0,
            baseline_reliability_lower_95: 0.0,
            candidate_reliability_lower_95: 0.0,
            baseline_pass_5: None,
            candidate_pass_5: None,
            baseline_agent_index: 0.0,
            candidate_agent_index: 0.0,
        };
    }
    if candidate.iter().any(|run| !run.hard_gates_passed) {
        reasons.push("候选版本触发硬质量门".to_string());
    }

    let baseline_quality = mean(baseline.iter().map(|run| run.quality_score));
    let candidate_quality = mean(candidate.iter().map(|run| run.quality_score));
    let quality_delta_points = (candidate_quality - baseline_quality) * 100.0;
    if quality_delta_points < -3.0 {
        reasons.push(format!("综合质量下降 {quality_delta_points:.2} 个百分点"));
    }

    let baseline_ttfr = values(baseline.iter().filter_map(|run| run.latency.ttfr_ms));
    let candidate_ttfr = values(candidate.iter().filter_map(|run| run.latency.ttfr_ms));
    let ttfr_p50_change = change(
        quantile(&baseline_ttfr, 0.5),
        quantile(&candidate_ttfr, 0.5),
    );
    let ttfr_p90_change = change(
        quantile(&baseline_ttfr, 0.9),
        quantile(&candidate_ttfr, 0.9),
    );
    let total_p50_change = change(
        quantile(
            &values(baseline.iter().map(|run| run.latency.total_ms)),
            0.5,
        ),
        quantile(
            &values(candidate.iter().map(|run| run.latency.total_ms)),
            0.5,
        ),
    );
    let token_change = change(
        Some(mean(baseline.iter().map(|run| run.usage.total() as f64))),
        Some(mean(candidate.iter().map(|run| run.usage.total() as f64))),
    );
    let tool_error_rate_change = change(
        Some(tool_error_rate(baseline)),
        Some(tool_error_rate(candidate)),
    );
    let baseline_max_repeat = baseline
        .iter()
        .map(|run| run.tools.max_repeat)
        .max()
        .unwrap_or_default();
    let candidate_max_repeat = candidate
        .iter()
        .map(|run| run.tools.max_repeat)
        .max()
        .unwrap_or_default();
    let baseline_reliability = reliability_report(baseline, 5);
    let candidate_reliability = reliability_report(candidate, 5);

    if ttfr_p90_change.is_some_and(|value| value > 0.10) {
        reasons.push("TTFR P90 恶化超过 10%".to_string());
    }
    if tool_error_rate(candidate) > tool_error_rate(baseline) {
        reasons.push("工具错误率高于基线".to_string());
    }
    if candidate_max_repeat > baseline_max_repeat {
        reasons.push("同参数工具调用重复次数高于基线".to_string());
    }
    if candidate_reliability.wilson_lower_95 + 0.02 < baseline_reliability.wilson_lower_95 {
        reasons.push("可靠率 Wilson 95% 下界相对基线下降超过 2 个百分点".to_string());
    }
    if candidate_reliability.galen_agent_index + 2.0 < baseline_reliability.galen_agent_index {
        reasons.push("Galen Agent Index 相对基线下降超过 2 分".to_string());
    }

    // 上下文工程 Gate：仅当运行记录携带 summary_field_coverage 时启用（M1/M3/M4）
    let has_ctx_metrics = baseline
        .iter()
        .chain(candidate.iter())
        .any(|run| run.context.summary_field_coverage.is_some());
    if has_ctx_metrics {
        let base_rate = baseline.iter().filter(|r| r.hard_gates_passed).count() as f64
            / baseline.len().max(1) as f64;
        let cand_rate = candidate.iter().filter(|r| r.hard_gates_passed).count() as f64
            / candidate.len().max(1) as f64;
        if base_rate > 0.0 && cand_rate / base_rate < 0.90 {
            reasons.push(format!(
                "上下文任务成功率保留度 {:.0}% 低于 90%（原始 {:.0}% → 压缩 {:.0}%）",
                cand_rate / base_rate * 100.0,
                base_rate * 100.0,
                cand_rate * 100.0
            ));
        }
        let coverage = mean(candidate.iter().filter_map(|run| {
            run.context.summary_field_coverage.map(|(hit, total)| {
                if total == 0 {
                    0.0
                } else {
                    hit as f64 / total as f64
                }
            })
        }));
        if coverage.is_finite() && coverage < 0.75 {
            reasons.push(format!(
                "摘要骨架字段覆盖率 {:.0}% 低于 75%（6/8）",
                coverage * 100.0
            ));
        }
        if token_change.is_some_and(|value| value > -0.30) {
            reasons.push(format!(
                "上下文 token 节省率不足 30%（变化 {:.0}%）",
                token_change.unwrap_or(0.0) * 100.0
            ));
        }
    }
    let meaningful_gain = ttfr_p50_change.is_some_and(|value| value <= -0.15)
        || total_p50_change.is_some_and(|value| value <= -0.15)
        || token_change.is_some_and(|value| value <= -0.15)
        || quality_delta_points >= 5.0
        || candidate_reliability.wilson_lower_95 >= baseline_reliability.wilson_lower_95 + 0.05
        || candidate_reliability.galen_agent_index >= baseline_reliability.galen_agent_index + 5.0;

    let rejected = !reasons.is_empty();
    let decision = if rejected {
        ComparisonDecision::Reject
    } else if meaningful_gain {
        reasons.push("通过非劣效门且至少一个主指标获得有意义改善".to_string());
        ComparisonDecision::Accept
    } else {
        reasons.push("未发现超过自然波动阈值的有效收益".to_string());
        ComparisonDecision::InsufficientData
    };

    ComparisonReport {
        decision,
        reasons,
        baseline_runs: baseline.len(),
        candidate_runs: candidate.len(),
        quality_delta_points,
        ttfr_p50_change,
        ttfr_p90_change,
        total_p50_change,
        token_change,
        tool_error_rate_change,
        baseline_max_repeat,
        candidate_max_repeat,
        baseline_reliability_lower_95: baseline_reliability.wilson_lower_95,
        candidate_reliability_lower_95: candidate_reliability.wilson_lower_95,
        baseline_pass_5: baseline_reliability.pass_k,
        candidate_pass_5: candidate_reliability.pass_k,
        baseline_agent_index: baseline_reliability.galen_agent_index,
        candidate_agent_index: candidate_reliability.galen_agent_index,
    }
}

fn dataset_keys(
    records: &[RunRecord],
    ignore_config: bool,
) -> BTreeMap<(String, String, String), usize> {
    let mut keys = BTreeMap::new();
    for record in records {
        *keys
            .entry((
                record.case_id.clone(),
                record.model.clone(),
                if ignore_config {
                    String::new()
                } else {
                    record.config_hash.clone()
                },
            ))
            .or_default() += 1;
    }
    keys
}

fn tool_error_rate(records: &[RunRecord]) -> f64 {
    let calls = records.iter().map(|run| run.tools.calls).sum::<usize>();
    let errors = records.iter().map(|run| run.tools.errors).sum::<usize>();
    if calls == 0 {
        0.0
    } else {
        errors as f64 / calls as f64
    }
}

fn assertion_score<F>(assertions: &[AssertionResult], select: F) -> f64
where
    F: Fn(&str) -> bool,
{
    let selected = assertions
        .iter()
        .filter(|assertion| select(&assertion.name))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        1.0
    } else {
        selected.iter().filter(|assertion| assertion.pass).count() as f64 / selected.len() as f64
    }
}

fn budget_score(actual: usize, budget: usize) -> f64 {
    if budget == 0 {
        return 0.0;
    }
    if actual <= budget {
        1.0 - 0.5 * actual as f64 / budget as f64
    } else {
        0.5 * budget as f64 / actual as f64
    }
}

fn rate(successes: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        successes as f64 / total as f64
    }
}

/// Two-sided 95% Wilson score interval lower bound for a Bernoulli rate.
pub fn wilson_lower_95(successes: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let n = total as f64;
    let p = successes as f64 / n;
    let z = 1.959_963_984_540_054_f64;
    let z2 = z * z;
    let center = p + z2 / (2.0 * n);
    let margin = z * ((p * (1.0 - p) + z2 / (4.0 * n)) / n).sqrt();
    ((center - margin) / (1.0 + z2 / n)).clamp(0.0, 1.0)
}

pub fn pass_k_probability(successes: usize, total: usize, k: usize) -> Option<f64> {
    if k == 0 || total < k {
        return None;
    }
    if successes < k {
        return Some(0.0);
    }
    Some(
        (0..k)
            .map(|index| (successes - index) as f64 / (total - index) as f64)
            .product(),
    )
}

fn mean_dimensions<'a, I>(records: I) -> ReliabilityDimensions
where
    I: Iterator<Item = &'a RunRecord>,
{
    let values = records
        .map(|run| {
            let dimensions = &run.dimensions;
            let legacy = dimensions.capability == 0.0
                && dimensions.repeatability == 0.0
                && dimensions.state_safety == 0.0
                && dimensions.delivery == 0.0
                && dimensions.efficiency == 0.0
                && run.quality_score > 0.0;
            if legacy {
                ReliabilityDimensions {
                    capability: run.quality_score,
                    repeatability: if run.hard_gates_passed { 1.0 } else { 0.0 },
                    state_safety: if run.hard_gates_passed { 1.0 } else { 0.0 },
                    delivery: if run.artifacts.required == 0 {
                        1.0
                    } else {
                        (run.artifacts.valid + run.artifacts.previewable) as f64
                            / (run.artifacts.required * 2) as f64
                    },
                    efficiency: 1.0,
                }
            } else {
                dimensions.clone()
            }
        })
        .collect::<Vec<_>>();
    if values.is_empty() {
        return ReliabilityDimensions::default();
    }
    let n = values.len() as f64;
    ReliabilityDimensions {
        capability: values.iter().map(|value| value.capability).sum::<f64>() / n,
        repeatability: values.iter().map(|value| value.repeatability).sum::<f64>() / n,
        state_safety: values.iter().map(|value| value.state_safety).sum::<f64>() / n,
        delivery: values.iter().map(|value| value.delivery).sum::<f64>() / n,
        efficiency: values.iter().map(|value| value.efficiency).sum::<f64>() / n,
    }
}

fn grounding_summary<'a, I>(records: I) -> GroundingSummary
where
    I: Iterator<Item = &'a RunRecord>,
{
    let values = records.collect::<Vec<_>>();
    let required_evidence = values
        .iter()
        .map(|run| run.grounding.required_evidence)
        .sum::<usize>();
    let retrieved_required = values
        .iter()
        .map(|run| run.grounding.retrieved_required)
        .sum::<usize>();
    let cited_required = values
        .iter()
        .map(|run| run.grounding.cited_required)
        .sum::<usize>();
    GroundingSummary {
        runs_with_required_evidence: values
            .iter()
            .filter(|run| run.grounding.required_evidence > 0)
            .count(),
        required_evidence,
        retrieved_required,
        cited_required,
        retrieval_coverage: (required_evidence > 0)
            .then_some(retrieved_required as f64 / required_evidence as f64),
        citation_coverage: (required_evidence > 0)
            .then_some(cited_required as f64 / required_evidence as f64),
        forbidden_hits: values.iter().map(|run| run.grounding.forbidden_hits).sum(),
        local_search_calls: values
            .iter()
            .map(|run| run.grounding.local_search_calls)
            .sum(),
        external_search_calls: values
            .iter()
            .map(|run| run.grounding.external_search_calls)
            .sum(),
    }
}

fn geometric_agent_index(dimensions: &ReliabilityDimensions) -> f64 {
    let weighted = [
        (dimensions.capability, 0.30),
        (dimensions.repeatability, 0.25),
        (dimensions.state_safety, 0.20),
        (dimensions.delivery, 0.15),
        (dimensions.efficiency, 0.10),
    ];
    if weighted.iter().any(|(value, _)| *value <= 0.0) {
        return 0.0;
    }
    100.0
        * weighted
            .iter()
            .map(|(value, weight)| weight * value.ln())
            .sum::<f64>()
            .exp()
}

pub fn append_jsonl(path: &Path, record: &RunRecord) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("创建评测结果目录失败: {error}"))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("打开评测记录失败: {error}"))?;
    serde_json::to_writer(&mut file, record)
        .map_err(|error| format!("序列化评测记录失败: {error}"))?;
    writeln!(file).map_err(|error| format!("写入评测记录失败: {error}"))
}

pub fn load_jsonl(path: &Path) -> Result<Vec<RunRecord>, String> {
    let file = std::fs::File::open(path)
        .map_err(|error| format!("打开评测记录 {} 失败: {error}", path.display()))?;
    BufReader::new(file)
        .lines()
        .enumerate()
        .filter_map(|(index, line)| match line {
            Ok(value) if value.trim().is_empty() => None,
            Ok(value) => Some(
                serde_json::from_str(&value)
                    .map_err(|error| format!("第 {} 行不是有效 RunRecord: {error}", index + 1)),
            ),
            Err(error) => Some(Err(format!("读取第 {} 行失败: {error}", index + 1))),
        })
        .collect()
}

pub fn discover_cases(dir: &Path) -> Result<Vec<(PathBuf, EvalCase)>, String> {
    let mut paths = std::fs::read_dir(dir)
        .map_err(|error| format!("读取案例目录 {} 失败: {error}", dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .map(|path| EvalCase::from_path(&path).map(|case| (path, case)))
        .collect()
}

fn retrieved_evidence_ids(traces: &[&ToolTrace]) -> Vec<String> {
    let mut ids = Vec::new();
    for trace in traces
        .iter()
        .filter(|trace| trace.tool == "search_evidence" && !trace.is_error)
    {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&trace.output) else {
            continue;
        };
        let Some(results) = value.get("results").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for result in results {
            let Some(id) = result
                .get("evidence")
                .and_then(|evidence| evidence.get("id"))
                .and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            if !ids.iter().any(|existing| existing == id) {
                ids.push(id.to_string());
            }
        }
    }
    ids
}

struct StructuredMatch {
    pass: bool,
    details: String,
}

fn match_expected_observation(text: &str, expected: &ExpectedObservation) -> StructuredMatch {
    let windows = text.match_indices(&expected.id).map(|(start, _)| {
        let line_start = text[..start]
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        let suffix = &text[start..];
        let line_end = start + suffix.find('\n').unwrap_or(suffix.len());
        &text[line_start..line_end.min(line_start + 320)]
    });

    for window in windows {
        let unit_matches =
            expected.unit.trim().is_empty() || unit_present(window, expected.unit.trim());
        match &expected.value {
            ExpectedObservationValue::Number(expected_value) => {
                if let Some(actual) = standalone_numbers(window)
                    .into_iter()
                    .find(|actual| (*actual - expected_value).abs() <= expected.tolerance)
                {
                    let pass = unit_matches;
                    if pass {
                        return StructuredMatch {
                            pass: true,
                            details: format!("matched value={actual}, unit={}", expected.unit),
                        };
                    }
                }
            }
            ExpectedObservationValue::Text(expected_value) => {
                if window
                    .to_lowercase()
                    .contains(&expected_value.to_lowercase())
                    && unit_matches
                {
                    return StructuredMatch {
                        pass: true,
                        details: format!("matched text={expected_value}"),
                    };
                }
            }
        }
    }

    StructuredMatch {
        pass: false,
        details: format!(
            "missing id/value/unit tuple: id={}, value={:?}, unit={}",
            expected.id, expected.value, expected.unit
        ),
    }
}

fn unit_present(text: &str, expected: &str) -> bool {
    match expected.to_ascii_lowercase().as_str() {
        "deg" | "degree" | "degrees" => {
            let lower = text.to_ascii_lowercase();
            lower.contains("deg") || text.contains('°') || text.contains('度')
        }
        unit => text.to_ascii_lowercase().contains(unit),
    }
}

fn contains_causal_boundary(text: &str) -> bool {
    let has_single_case =
        text.contains("单病例") || text.to_ascii_lowercase().contains("single case");
    let has_limit = [
        "不能",
        "无法",
        "不足以",
        "不可",
        "不应",
        "cannot",
        "insufficient",
    ]
    .iter()
    .any(|token| text.to_ascii_lowercase().contains(token));
    let has_target = ["因果", "疗效", "causal", "effectiveness", "efficacy"]
        .iter()
        .any(|token| text.to_ascii_lowercase().contains(token));
    has_single_case && has_limit && has_target
}

fn unsupported_degree_values(text: &str, structured: &StructuredOutcome) -> Vec<f64> {
    let allowed = allowed_numeric_tuples(structured, false);
    unsupported_values(degree_values(text), &allowed)
}

fn unsupported_medical_numeric_values(text: &str, structured: &StructuredOutcome) -> Vec<f64> {
    let allowed = allowed_numeric_tuples(structured, true);
    let mut candidates = medical_unit_values(text);
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        for (index, _) in lower.match_indices("risser") {
            let suffix = &line[index + "risser".len()..];
            let segment = suffix
                .split(['，', '。', '；', ',', ';'])
                .next()
                .unwrap_or(suffix);
            candidates.extend(standalone_numbers(segment));
        }
    }
    unsupported_values(candidates, &allowed)
}

fn allowed_numeric_tuples(
    structured: &StructuredOutcome,
    include_general: bool,
) -> Vec<(f64, f64)> {
    let mut allowed = structured
        .allowed_degree_values
        .iter()
        .map(|value| (*value, 1e-6))
        .collect::<Vec<_>>();
    if include_general {
        allowed.extend(
            structured
                .allowed_numeric_values
                .iter()
                .map(|value| (*value, 1e-6)),
        );
    }
    for observation in &structured.observations {
        if let ExpectedObservationValue::Number(value) = &observation.value {
            allowed.push((*value, observation.tolerance));
        }
    }
    allowed
}

fn unsupported_values(candidates: Vec<f64>, allowed: &[(f64, f64)]) -> Vec<f64> {
    let mut unsupported = Vec::new();
    for value in candidates {
        let pass = allowed
            .iter()
            .any(|(allowed_value, tolerance)| (value - allowed_value).abs() <= *tolerance);
        if !pass
            && !unsupported
                .iter()
                .any(|seen: &f64| (*seen - value).abs() < 1e-6)
        {
            unsupported.push(value);
        }
    }
    unsupported.sort_by(f64::total_cmp);
    unsupported
}

fn medical_unit_values(text: &str) -> Vec<f64> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut values = Vec::new();
    for index in 0..chars.len() {
        let single_char_unit = matches!(chars[index], '°' | '度' | '岁' | '年' | '月' | '天' | '%');
        let suffix = chars[index..]
            .iter()
            .take(8)
            .collect::<String>()
            .to_ascii_lowercase();
        let word_unit = ["deg", "year", "month", "week", "day", "hour", "percent"]
            .iter()
            .any(|unit| suffix.starts_with(unit));
        if single_char_unit || word_unit {
            values.extend(values_before_unit(&chars, index));
        }
    }
    values
}

fn degree_values(text: &str) -> Vec<f64> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut values = Vec::new();
    for index in 0..chars.len() {
        let is_marker = chars[index] == '°'
            || chars[index] == '度'
            || chars[index..]
                .iter()
                .take(3)
                .collect::<String>()
                .eq_ignore_ascii_case("deg");
        if !is_marker {
            continue;
        }
        values.extend(values_before_unit(&chars, index));
    }
    values
}

fn values_before_unit(chars: &[char], unit_start: usize) -> Vec<f64> {
    let mut value_end = unit_start;
    while value_end > 0 && chars[value_end - 1].is_whitespace() {
        value_end -= 1;
    }
    if value_end > 0 && chars[value_end - 1] == '个' {
        value_end -= 1;
    }
    let Some((first_start, last)) = previous_number(chars, value_end) else {
        return Vec::new();
    };
    let mut values = vec![last];
    let mut cursor = first_start;
    while cursor > 0 && chars[cursor - 1].is_whitespace() {
        cursor -= 1;
    }
    if cursor > 0 && matches!(chars[cursor - 1], '-' | '–' | '—' | '~' | '至') {
        if let Some((_, range_start)) = previous_number(chars, cursor - 1) {
            values.push(range_start);
        }
    }
    values
}

fn previous_number(chars: &[char], mut end: usize) -> Option<(usize, f64)> {
    while end > 0 && chars[end - 1].is_whitespace() {
        end -= 1;
    }
    if end == 0 || !chars[end - 1].is_ascii_digit() {
        return None;
    }
    let mut start = end - 1;
    while start > 0 && (chars[start - 1].is_ascii_digit() || chars[start - 1] == '.') {
        start -= 1;
    }
    if start > 0
        && matches!(chars[start - 1], '-' | '+' | '−')
        && (start == 1 || !chars[start - 2].is_ascii_digit())
    {
        start -= 1;
    }
    let value = chars[start..end]
        .iter()
        .map(|value| if *value == '−' { '-' } else { *value })
        .collect::<String>()
        .parse::<f64>()
        .ok()?;
    Some((start, value))
}

fn standalone_numbers(text: &str) -> Vec<f64> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut values = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let starts_number = chars[index].is_ascii_digit()
            || ((matches!(chars[index], '-' | '+' | '−'))
                && chars.get(index + 1).is_some_and(char::is_ascii_digit));
        let attached_to_identifier = index > 0 && chars[index - 1].is_ascii_alphanumeric();
        if !starts_number || attached_to_identifier {
            index += 1;
            continue;
        }
        let start = index;
        if matches!(chars[index], '-' | '+' | '−') {
            index += 1;
        }
        while index < chars.len() && (chars[index].is_ascii_digit() || chars[index] == '.') {
            index += 1;
        }
        if let Ok(value) = chars[start..index]
            .iter()
            .map(|value| if *value == '−' { '-' } else { *value })
            .collect::<String>()
            .parse::<f64>()
        {
            values.push(value);
        }
    }
    values
}

fn searchable_output(workspace: &Path, response: &str) -> String {
    let mut text = response.to_string();
    for relative in ["output", ".galen"] {
        let root = workspace.join(relative);
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.metadata().is_ok_and(|meta| meta.len() <= 1_000_000) {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        text.push('\n');
                        text.push_str(&content);
                    }
                }
            }
        }
    }
    text
}

fn is_previewable(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "md" | "txt" | "csv" | "json" | "html" | "pdf" | "png" | "jpg" | "svg"
            )
        })
}

fn stable_hash(value: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn values<I>(iter: I) -> Vec<f64>
where
    I: Iterator<Item = u64>,
{
    iter.map(|value| value as f64).collect()
}

fn mean<I>(iter: I) -> f64
where
    I: Iterator<Item = f64>,
{
    let values = iter.collect::<Vec<_>>();
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn quantile(values: &[f64], q: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let position = q * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let weight = position - lower as f64;
    Some(sorted[lower] * (1.0 - weight) + sorted[upper] * weight)
}

fn change(baseline: Option<f64>, candidate: Option<f64>) -> Option<f64> {
    match (baseline, candidate) {
        (Some(base), Some(next)) if base > 0.0 => Some((next - base) / base),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary() -> ChatRunSummary {
        ChatRunSummary {
            model_request_count: 2,
            input_tokens: 100,
            output_tokens: 20,
            ttft_ms: Some(100),
            ttfr_ms: Some(200),
            total_ms: 500,
            ..ChatRunSummary::default()
        }
    }

    fn case() -> EvalCase {
        EvalCase {
            schema_version: 1,
            id: "E01".to_string(),
            name: "quick".to_string(),
            suite: "smoke".to_string(),
            risk_tier: "standard".to_string(),
            prompt: "answer".to_string(),
            fixture: None,
            timeout_seconds: 10,
            max_model_requests: 3,
            max_tool_calls: 4,
            max_human_interventions: 0,
            required: RequiredOutcome {
                facts: vec!["FMA-UE".to_string()],
                ..RequiredOutcome::default()
            },
            forbidden: ForbiddenOutcome::default(),
            structured: StructuredOutcome::default(),
            context: ContextSpec::default(),
        }
    }

    fn reliability_record(pass: bool, run_index: u32) -> RunRecord {
        reliability_record_with_coverage(pass, run_index, None)
    }

    fn reliability_record_with_coverage(
        pass: bool,
        run_index: u32,
        coverage: Option<(usize, usize)>,
    ) -> RunRecord {
        RunRecord {
            schema_version: 1,
            run_id: format!("r-{run_index}"),
            case_id: "E01".to_string(),
            commit: "c".to_string(),
            model: "m".to_string(),
            config_hash: "h".to_string(),
            run_index,
            started_at_ms: 0,
            workspace: "workspace".to_string(),
            final_response: String::new(),
            hard_gates_passed: pass,
            quality_score: if pass { 1.0 } else { 0.5 },
            dimensions: ReliabilityDimensions {
                capability: if pass { 1.0 } else { 0.5 },
                repeatability: if pass { 1.0 } else { 0.0 },
                state_safety: if pass { 1.0 } else { 0.0 },
                delivery: if pass { 1.0 } else { 0.0 },
                efficiency: 0.8,
            },
            latency: EvalLatency::default(),
            usage: EvalUsage::default(),
            model_requests: 1,
            tools: EvalTools::default(),
            tool_trace: Vec::new(),
            context: EvalContext {
                summary_field_coverage: coverage,
                ..EvalContext::default()
            },
            grounding: EvalGrounding::default(),
            artifacts: EvalArtifacts::default(),
            assertions: Vec::new(),
        }
    }

    #[test]
    fn summary_field_coverage_counts_skeleton_fields() {
        let summary = "Summary:\n- Scope: 40 earlier messages compacted.\n- Current work: finish.\n- Pending work: next.\n- Key files referenced: a.rs.\n- Tools mentioned: read_file.\n- Recent user requests: x.\n- Previously compacted context: y.\n- Newly compacted context: z.\n- Key timeline:\n  - user: hi";
        let (hit, total) = summary_field_coverage(summary, &[]);
        assert_eq!((hit, total), (8, 8));
        let partial = summary_field_coverage("Summary:\n- Scope: a.", &[]);
        assert_eq!(partial, (1, 8));
        let required = summary_field_coverage(
            summary,
            &[
                "- Scope:".to_string(),
                "- Key files referenced:".to_string(),
            ],
        );
        assert_eq!(required, (2, 2));
    }

    #[test]
    fn structured_observation_accepts_harmless_format_variants() {
        let expected = ExpectedObservation {
            id: "C024-B-T".to_string(),
            value: ExpectedObservationValue::Number(27.0),
            unit: "deg".to_string(),
            tolerance: 0.0,
        };
        for output in [
            "C024-B-T=27 deg",
            "观察ID=C024-B-T 27 deg",
            "| C024-B-T | 27 | 度 |",
            "观察ID=27 deg（C024-B-T，胸段 Cobb 角）",
        ] {
            assert!(
                match_expected_observation(output, &expected).pass,
                "{output}"
            );
        }
        assert!(!match_expected_observation("C024-B-T=28 deg", &expected).pass);
    }

    #[test]
    fn causal_boundary_is_semantic_not_fixed_prefix() {
        assert!(contains_causal_boundary("单病例不能证明因果疗效。"));
        assert!(contains_causal_boundary(
            "A single case is insufficient to establish causal effectiveness."
        ));
        assert!(!contains_causal_boundary("该病例证明治疗有效。"));
    }

    #[test]
    fn source_closed_degree_gate_catches_prior_unsupported_threshold() {
        let structured = StructuredOutcome {
            observations: vec![ExpectedObservation {
                id: "C025-B-T".to_string(),
                value: ExpectedObservationValue::Number(45.0),
                unit: "deg".to_string(),
                tolerance: 0.0,
            }],
            source_closed_degree_values: true,
            ..StructuredOutcome::default()
        };
        let output = "观察ID C025-B-T = 45 deg\n通常 Cobb ≥ 40–50° 时需专科评估";
        assert_eq!(
            unsupported_degree_values(output, &structured),
            vec![40.0, 50.0]
        );
        assert!(unsupported_degree_values("C025-B-T=45°", &structured).is_empty());
        assert!(unsupported_degree_values(
            "2019-05 基线，11 岁，Risser 0；观察ID C025-B-T = 45 deg",
            &structured
        )
        .is_empty());
    }

    #[test]
    fn source_closed_numeric_gate_catches_new_maturation_claims() {
        let structured = StructuredOutcome {
            observations: vec![ExpectedObservation {
                id: "C025-B-T".to_string(),
                value: ExpectedObservationValue::Number(45.0),
                unit: "deg".to_string(),
                tolerance: 0.0,
            }],
            source_closed_numeric_values: true,
            allowed_numeric_values: vec![0.0, 11.0, 2019.0],
            ..StructuredOutcome::default()
        };
        let output = "基线 2019-05，年龄 11 岁，Risser 0；C025-B-T=45 deg。达骨骼成熟时（Risser ≥ 4–5 或稳定 ≥ 2 年）。";
        assert_eq!(
            unsupported_medical_numeric_values(output, &structured),
            vec![2.0, 4.0, 5.0]
        );
        assert!(unsupported_medical_numeric_values(
            "4. 补充 Risser 征与月经初潮状态。",
            &structured
        )
        .is_empty());
        assert_eq!(
            unsupported_medical_numeric_values("建议每 4–6 个月随访。", &structured),
            vec![4.0, 6.0]
        );
        let negative = StructuredOutcome {
            allowed_numeric_values: vec![-3.0, 0.0, 2019.0],
            source_closed_numeric_values: true,
            ..StructuredOutcome::default()
        };
        assert!(unsupported_medical_numeric_values(
            "胸椎 ATR −3°；Risser 0，基线日期 2019-05。",
            &negative
        )
        .is_empty());
    }

    #[test]
    fn scorer_v2_reclassifies_old_t2_failures_without_hiding_grounding_error() {
        let workspace = std::env::temp_dir().join("galen-eval-scorer-v2-regression");
        std::fs::create_dir_all(&workspace).unwrap();
        let evaluate = |observations: &[(&str, f64)], response: &str| {
            let mut value = case();
            value.required.facts.clear();
            value.structured = StructuredOutcome {
                observations: observations
                    .iter()
                    .map(|(id, number)| ExpectedObservation {
                        id: (*id).to_string(),
                        value: ExpectedObservationValue::Number(*number),
                        unit: "deg".to_string(),
                        tolerance: 0.0,
                    })
                    .collect(),
                source_closed_degree_values: true,
                require_causal_boundary: true,
                ..StructuredOutcome::default()
            };
            RunRecord::evaluate(
                &value,
                RunObservation {
                    commit: "abc",
                    model: "model",
                    run_index: 1,
                    run_ok: true,
                    response,
                    summary: &summary(),
                    traces: &[],
                    workspace: &workspace,
                    summary_field_coverage: None,
                },
            )
        };

        let c023 = evaluate(
            &[("C023-B-T", 9.0), ("C023-B-L", 32.0)],
            "C023-B-T=9 deg\nC023-B-L=32 deg\n单病例不能证明因果疗效。",
        );
        let c024 = evaluate(
            &[
                ("C024-B-T", 27.0),
                ("C024-B-TL", 30.0),
                ("C024-B-ATR-T", 11.0),
                ("C024-B-ATR-TL", 8.0),
            ],
            "观察ID=C024-B-T 27 deg\n观察ID=C024-B-TL 30 deg\n观察ID=C024-B-ATR-T 11 deg\n观察ID=C024-B-ATR-TL 8 deg\n科研边界：单病例不能证明因果疗效。",
        );
        let c026 = evaluate(
            &[("C026-B-T", 40.0), ("C026-B-L", 22.0)],
            "观察ID=C026-B-T 40 deg\n观察ID=C026-B-L 22 deg\n科研边界：单病例不能证明因果疗效。",
        );
        let c025 = evaluate(
            &[("C025-B-T", 45.0)],
            "观察ID C025-B-T = 45 deg\n通常 Cobb ≥ 40–50° 时需专科评估\n科研边界：单病例不能证明因果疗效。",
        );

        assert!(c023.hard_gates_passed);
        assert!(c024.hard_gates_passed);
        assert!(c026.hard_gates_passed);
        assert!(!c025.hard_gates_passed);
        assert!(c025.assertions.iter().any(|assertion| {
            assertion.name == "source_closed_degree_values" && !assertion.pass
        }));
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn context_gate_rejects_low_retention() {
        // 基线全过 5 次；候选 3/5 过（保留度 60% < 90%），覆盖率 7/8 达标
        let baseline = (0..5)
            .map(|i| reliability_record(true, i))
            .collect::<Vec<_>>();
        let candidate = (0..5)
            .map(|i| reliability_record_with_coverage(i < 3, i, Some((7, 8))))
            .collect::<Vec<_>>();
        let report = compare_runs(&baseline, &candidate);
        assert!(matches!(report.decision, ComparisonDecision::Reject));
        assert!(
            report
                .reasons
                .iter()
                .any(|reason| reason.contains("保留度")),
            "应报告保留度不足: {:?}",
            report.reasons
        );
    }

    #[test]
    fn context_gate_accepts_high_retention_with_savings() {
        // 基线 4/5 过；候选 5/5 全过（保留度 125%），覆盖率 8/8，质量提升
        let baseline = (0..5)
            .map(|i| reliability_record(i != 0, i))
            .collect::<Vec<_>>();
        let candidate = (0..5)
            .map(|i| reliability_record_with_coverage(true, i, Some((8, 8))))
            .collect::<Vec<_>>();
        let report = compare_runs(&baseline, &candidate);
        assert!(
            matches!(report.decision, ComparisonDecision::Accept),
            "应接受保留度达标且质量提升: {:?}",
            report.reasons
        );
    }

    #[test]
    fn repeated_identical_tool_call_fails_hard_gate() {
        let workspace = std::env::temp_dir().join("galen-eval-repeat-test");
        std::fs::create_dir_all(&workspace).unwrap();
        let traces = (0..3)
            .map(|turn| ToolTrace {
                turn,
                tool: "read_file".to_string(),
                input: "same".to_string(),
                output: "ok".to_string(),
                is_error: false,
            })
            .collect::<Vec<_>>();
        let summary = summary();
        let record = RunRecord::evaluate(
            &case(),
            RunObservation {
                commit: "abc",
                model: "model",
                run_index: 1,
                run_ok: true,
                response: "FMA-UE",
                summary: &summary,
                traces: &traces,
                workspace: &workspace,
                summary_field_coverage: None,
            },
        );
        assert!(!record.hard_gates_passed);
        assert_eq!(record.tools.max_repeat, 3);
    }

    #[test]
    fn exact_tool_sequence_checks_arguments_errors_and_order() {
        let workspace = std::env::temp_dir().join("galen-eval-tool-sequence-test");
        std::fs::create_dir_all(&workspace).unwrap();
        let mut value = case();
        value.required.tool_sequence = vec![
            ExpectedToolCall {
                tool: "read_file".to_string(),
                input_contains: "inputs/missing.md".to_string(),
                is_error: Some(true),
            },
            ExpectedToolCall {
                tool: "read_file".to_string(),
                input_contains: "inputs/brief.md".to_string(),
                is_error: Some(false),
            },
        ];
        let traces = vec![ToolTrace {
            turn: 1,
            tool: "read_file".to_string(),
            input: r#"{"path":"inputs/brief.md"}"#.to_string(),
            output: "FMA-UE".to_string(),
            is_error: false,
        }];
        let summary = summary();
        let record = RunRecord::evaluate(
            &value,
            RunObservation {
                commit: "abc",
                model: "model",
                run_index: 1,
                run_ok: true,
                response: "FMA-UE",
                summary: &summary,
                traces: &traces,
                workspace: &workspace,
                summary_field_coverage: None,
            },
        );
        assert!(!record.hard_gates_passed);
        assert!(record
            .assertions
            .iter()
            .any(|assertion| { assertion.name == "required_tool_sequence" && !assertion.pass }));
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn grounding_requires_evidence_to_be_retrieved_and_cited() {
        let workspace = std::env::temp_dir().join("galen-eval-grounding-test");
        std::fs::create_dir_all(&workspace).unwrap();
        let mut value = case();
        value.required.facts.clear();
        value.required.tools = vec!["search_evidence".to_string()];
        value.required.evidence_ids = vec!["ev-good".to_string()];
        value.forbidden.evidence_ids = vec!["ev-bad".to_string()];
        let traces = vec![ToolTrace {
            turn: 1,
            tool: "search_evidence".to_string(),
            input: r#"{"query":"施罗斯"}"#.to_string(),
            output: r#"{"results":[{"evidence":{"id":"ev-good"}}]}"#.to_string(),
            is_error: false,
        }];
        let record = RunRecord::evaluate(
            &value,
            RunObservation {
                commit: "abc",
                model: "model",
                run_index: 1,
                run_ok: true,
                response: "结论依据 ev-good。",
                summary: &summary(),
                traces: &traces,
                workspace: &workspace,
                summary_field_coverage: None,
            },
        );
        assert!(record.hard_gates_passed);
        assert_eq!(record.grounding.retrieved_required, 1);
        assert_eq!(record.grounding.cited_required, 1);
        assert_eq!(record.grounding.forbidden_hits, 0);
        assert_eq!(record.grounding.local_search_calls, 1);
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn comparator_accepts_material_speedup_without_quality_loss() {
        let make = |ttfr, total, quality, hard_gates_passed| RunRecord {
            schema_version: 1,
            run_id: "r".to_string(),
            case_id: "E01".to_string(),
            commit: "c".to_string(),
            model: "m".to_string(),
            config_hash: "h".to_string(),
            run_index: 1,
            started_at_ms: 0,
            workspace: "workspace".to_string(),
            final_response: String::new(),
            hard_gates_passed,
            quality_score: quality,
            dimensions: ReliabilityDimensions {
                capability: quality,
                repeatability: if hard_gates_passed { 1.0 } else { 0.0 },
                state_safety: if hard_gates_passed { 1.0 } else { 0.0 },
                delivery: 1.0,
                efficiency: 1.0,
            },
            latency: EvalLatency {
                ttfr_ms: Some(ttfr),
                total_ms: total,
                ..EvalLatency::default()
            },
            usage: EvalUsage {
                input: 100,
                ..EvalUsage::default()
            },
            model_requests: 1,
            tools: EvalTools::default(),
            tool_trace: Vec::new(),
            context: EvalContext::default(),
            grounding: EvalGrounding::default(),
            artifacts: EvalArtifacts::default(),
            assertions: Vec::new(),
        };
        let baseline = (0..5)
            .map(|_| make(1_000, 2_000, 0.95, true))
            .collect::<Vec<_>>();
        let candidate = (0..5)
            .map(|_| make(700, 1_500, 0.95, true))
            .collect::<Vec<_>>();
        let report = compare_runs(&baseline, &candidate);
        assert!(matches!(report.decision, ComparisonDecision::Accept));

        let unsafe_candidate = (0..5)
            .map(|_| make(500, 1_000, 0.99, false))
            .collect::<Vec<_>>();
        let report = compare_runs(&baseline, &unsafe_candidate);
        assert!(matches!(report.decision, ComparisonDecision::Reject));
    }

    #[test]
    fn rejects_unsafe_artifact_paths() {
        let mut value = case();
        value.required.artifacts.push("../escape.md".to_string());
        assert!(value.validate().is_err());
    }

    #[test]
    fn reliability_report_exposes_intermittent_failure() {
        let records = (1..=10)
            .map(|index| reliability_record(index != 10, index))
            .collect::<Vec<_>>();
        let report = reliability_report(&records, 5);
        assert!(!report.qualified);
        assert_eq!(report.successes, 9);
        assert!((report.success_rate - 0.9).abs() < 1e-9);
        assert!((report.pass_k.unwrap() - 0.5).abs() < 1e-9);
        assert!(report.wilson_lower_95 < report.success_rate);
        assert!(report.galen_agent_index > 0.0);
    }

    #[test]
    fn reliability_report_qualifies_all_passed_runs() {
        let records = (1..=5)
            .map(|index| reliability_record(true, index))
            .collect::<Vec<_>>();
        let report = reliability_report(&records, 5);
        assert!(report.qualified);
        assert_eq!(report.pass_k, Some(1.0));
        assert!(report.wilson_lower_95 < 1.0);
    }
}
