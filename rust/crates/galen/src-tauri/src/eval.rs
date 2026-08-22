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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCase {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub suite: String,
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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequiredOutcome {
    #[serde(default)]
    pub facts: Vec<String>,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForbiddenOutcome {
    #[serde(default = "default_repeat_limit")]
    pub repeated_call_limit: usize,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub response_patterns: Vec<String>,
}

impl Default for ForbiddenOutcome {
    fn default() -> Self {
        Self {
            repeated_call_limit: default_repeat_limit(),
            tools: Vec::new(),
            response_patterns: Vec::new(),
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
        if self.forbidden.repeated_call_limit == 0 {
            return Err(format!("案例 {} 的重复调用上限必须大于 0", self.id));
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
    pub latency: EvalLatency,
    pub usage: EvalUsage,
    pub model_requests: u32,
    pub tools: EvalTools,
    pub tool_trace: Vec<ToolTrace>,
    pub context: EvalContext,
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
        for pattern in &case.forbidden.response_patterns {
            add(
                format!("forbidden_response_pattern:{pattern}"),
                !observation.response.contains(pattern),
                "checked final response".to_string(),
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
        let started_at_ms = now_millis();
        let config_hash = stable_hash(&format!(
            "{}|{}|{}|{}|{}",
            case.id, observation.model, case.prompt, case.max_model_requests, case.max_tool_calls
        ));

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
                required_facts: case.required.facts.len(),
                retained_facts,
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
}

pub fn compare_runs(baseline: &[RunRecord], candidate: &[RunRecord]) -> ComparisonReport {
    let mut reasons = Vec::new();
    let baseline_keys = dataset_keys(baseline);
    let candidate_keys = dataset_keys(candidate);
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

    if ttfr_p90_change.is_some_and(|value| value > 0.10) {
        reasons.push("TTFR P90 恶化超过 10%".to_string());
    }
    if tool_error_rate(candidate) > tool_error_rate(baseline) {
        reasons.push("工具错误率高于基线".to_string());
    }
    if candidate_max_repeat > baseline_max_repeat {
        reasons.push("同参数工具调用重复次数高于基线".to_string());
    }
    let meaningful_gain = ttfr_p50_change.is_some_and(|value| value <= -0.15)
        || total_p50_change.is_some_and(|value| value <= -0.15)
        || token_change.is_some_and(|value| value <= -0.15)
        || quality_delta_points >= 5.0;

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
    }
}

fn dataset_keys(records: &[RunRecord]) -> BTreeMap<(String, String, String), usize> {
    let mut keys = BTreeMap::new();
    for record in records {
        *keys
            .entry((
                record.case_id.clone(),
                record.model.clone(),
                record.config_hash.clone(),
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
        }
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
            },
        );
        assert!(!record.hard_gates_passed);
        assert_eq!(record.tools.max_repeat, 3);
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
}
