//! Deterministic component evaluation for Galen's local evidence retrieval.
//!
//! Agent evals answer whether the whole workflow succeeds. This module keeps
//! retrieval quality independently measurable without model or network noise.

use std::collections::HashSet;
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::eval::ComparisonDecision;

fn default_top_k() -> usize {
    5
}

fn default_min_recall() -> f64 {
    0.80
}

fn default_min_mrr() -> f64 {
    0.70
}

fn default_min_ndcg() -> f64 {
    0.75
}

fn default_max_p95_ms() -> u64 {
    100
}

fn default_max_cold_ms() -> u64 {
    2_000
}

fn default_min_negative_accuracy() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagBenchmarkSpec {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub fixture: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(default)]
    pub thresholds: RagThresholds,
    pub queries: Vec<RagQuerySpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagThresholds {
    #[serde(default = "default_min_recall")]
    pub min_recall_at_k: f64,
    #[serde(default = "default_min_mrr")]
    pub min_mrr: f64,
    #[serde(default = "default_min_ndcg")]
    pub min_ndcg_at_k: f64,
    #[serde(default = "default_max_p95_ms")]
    pub max_p95_ms: u64,
    #[serde(default = "default_max_cold_ms")]
    pub max_cold_index_ms: u64,
    #[serde(default = "default_min_negative_accuracy")]
    pub min_negative_query_accuracy: f64,
    #[serde(default = "default_true")]
    pub require_zero_forbidden_hits: bool,
}

fn default_true() -> bool {
    true
}

impl Default for RagThresholds {
    fn default() -> Self {
        Self {
            min_recall_at_k: default_min_recall(),
            min_mrr: default_min_mrr(),
            min_ndcg_at_k: default_min_ndcg(),
            max_p95_ms: default_max_p95_ms(),
            max_cold_index_ms: default_max_cold_ms(),
            min_negative_query_accuracy: default_min_negative_accuracy(),
            require_zero_forbidden_hits: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagQuerySpec {
    pub id: String,
    pub query: String,
    #[serde(default)]
    pub relevant: Vec<String>,
    #[serde(default)]
    pub forbidden: Vec<String>,
    #[serde(default)]
    pub expected_empty: bool,
}

impl RagBenchmarkSpec {
    pub fn from_path(path: &Path) -> Result<Self, String> {
        let source = std::fs::read_to_string(path)
            .map_err(|error| format!("读取 RAG 数据集 {} 失败: {error}", path.display()))?;
        let spec: Self = toml::from_str(&source)
            .map_err(|error| format!("解析 RAG 数据集 {} 失败: {error}", path.display()))?;
        spec.validate()?;
        Ok(spec)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!("RAG 数据集 {} 的 schema_version 必须为 1", self.id));
        }
        if self.id.trim().is_empty()
            || self.name.trim().is_empty()
            || self.fixture.trim().is_empty()
        {
            return Err("RAG 数据集 id、name 和 fixture 均不能为空".to_string());
        }
        if self.top_k == 0 || self.top_k > 20 {
            return Err(format!("RAG 数据集 {} 的 top_k 必须在 1..=20", self.id));
        }
        if self.queries.is_empty() {
            return Err(format!("RAG 数据集 {} 至少需要一个查询", self.id));
        }
        for value in [
            self.thresholds.min_recall_at_k,
            self.thresholds.min_mrr,
            self.thresholds.min_ndcg_at_k,
            self.thresholds.min_negative_query_accuracy,
        ] {
            if !(0.0..=1.0).contains(&value) {
                return Err(format!("RAG 数据集 {} 的质量阈值必须在 0..=1", self.id));
            }
        }
        let mut ids = HashSet::new();
        for query in &self.queries {
            if query.id.trim().is_empty() || query.query.trim().is_empty() {
                return Err(format!(
                    "RAG 数据集 {} 的每个查询必须包含 id 和 query",
                    self.id
                ));
            }
            if query.expected_empty != query.relevant.is_empty() {
                return Err(format!(
                    "查询 {} 必须是含 relevant 的正查询，或 expected_empty=true 的负查询",
                    query.id
                ));
            }
            if !ids.insert(query.id.as_str()) {
                return Err(format!("RAG 数据集 {} 包含重复查询 {}", self.id, query.id));
            }
            if query.relevant.iter().any(|id| query.forbidden.contains(id)) {
                return Err(format!(
                    "查询 {} 的 relevant 与 forbidden 不能重叠",
                    query.id
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagQueryResult {
    pub query_id: String,
    pub query: String,
    pub relevant_ids: Vec<String>,
    pub retrieved_ids: Vec<String>,
    pub forbidden_hits: Vec<String>,
    pub expected_empty: bool,
    pub empty_query_pass: bool,
    pub recall_at_k: f64,
    pub precision_at_k: f64,
    pub reciprocal_rank: f64,
    pub ndcg_at_k: f64,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RagAggregateMetrics {
    pub recall_at_k: f64,
    pub precision_at_k: f64,
    pub mrr: f64,
    pub ndcg_at_k: f64,
    pub forbidden_hits: usize,
    pub negative_query_accuracy: f64,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub cold_index_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagGateResult {
    pub name: String,
    pub pass: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagBenchmarkReport {
    pub schema_version: u32,
    pub dataset_id: String,
    pub dataset_name: String,
    #[serde(default)]
    pub dataset_hash: String,
    pub engine: String,
    pub tokenizer: String,
    pub commit: String,
    pub started_at_ms: u128,
    pub repeat: usize,
    pub top_k: usize,
    pub hard_gates_passed: bool,
    pub aggregate: RagAggregateMetrics,
    pub gates: Vec<RagGateResult>,
    pub queries: Vec<RagQueryResult>,
}

pub fn run_rag_benchmark(
    spec: &RagBenchmarkSpec,
    workspace: &Path,
    commit: &str,
    repeat: usize,
) -> Result<RagBenchmarkReport, String> {
    spec.validate()?;
    if repeat == 0 {
        return Err("RAG benchmark repeat 必须大于 0".to_string());
    }

    let cold_started = Instant::now();
    crate::evidence_search::search_evidence(workspace, &spec.queries[0].query, spec.top_k)?;
    let cold_index_ms = elapsed_ms(cold_started);

    let mut query_results = Vec::with_capacity(spec.queries.len());
    let mut all_latencies = Vec::with_capacity(spec.queries.len() * repeat);
    for query in &spec.queries {
        let mut latencies = Vec::with_capacity(repeat);
        let mut retrieved_ids = Vec::new();
        for run_index in 0..repeat {
            let started = Instant::now();
            let hits =
                crate::evidence_search::search_evidence(workspace, &query.query, spec.top_k)?;
            let latency = elapsed_ms(started);
            latencies.push(latency as f64);
            all_latencies.push(latency as f64);
            if run_index == 0 {
                retrieved_ids = hits.into_iter().map(|hit| hit.evidence.id).collect();
            }
        }
        query_results.push(score_query(query, &retrieved_ids, spec.top_k, &latencies));
    }

    let aggregate = aggregate_metrics(&query_results, &all_latencies, cold_index_ms);
    let gates = evaluate_gates(&aggregate, &spec.thresholds);
    let hard_gates_passed = gates.iter().all(|gate| gate.pass);
    let dataset_hash = dataset_hash(spec, workspace)?;
    Ok(RagBenchmarkReport {
        schema_version: 1,
        dataset_id: spec.id.clone(),
        dataset_name: spec.name.clone(),
        dataset_hash,
        engine: "tantivy-bm25".to_string(),
        tokenizer: "jieba-search-mode".to_string(),
        commit: commit.to_string(),
        started_at_ms: now_millis(),
        repeat,
        top_k: spec.top_k,
        hard_gates_passed,
        aggregate,
        gates,
        queries: query_results,
    })
}

fn score_query(
    spec: &RagQuerySpec,
    retrieved: &[String],
    top_k: usize,
    latencies: &[f64],
) -> RagQueryResult {
    let relevant: HashSet<&str> = spec.relevant.iter().map(String::as_str).collect();
    let retrieved = retrieved.iter().take(top_k).cloned().collect::<Vec<_>>();
    let hits = retrieved
        .iter()
        .filter(|id| relevant.contains(id.as_str()))
        .count();
    let reciprocal_rank = retrieved
        .iter()
        .position(|id| relevant.contains(id.as_str()))
        .map_or(0.0, |rank| 1.0 / (rank + 1) as f64);
    let dcg = retrieved
        .iter()
        .enumerate()
        .filter(|(_, id)| relevant.contains(id.as_str()))
        .map(|(rank, _)| 1.0 / ((rank + 2) as f64).log2())
        .sum::<f64>();
    let ideal_hits = relevant.len().min(top_k);
    let idcg = (0..ideal_hits)
        .map(|rank| 1.0 / ((rank + 2) as f64).log2())
        .sum::<f64>();
    let forbidden_hits = retrieved
        .iter()
        .filter(|id| spec.forbidden.contains(id))
        .cloned()
        .collect();
    let empty_query_pass = !spec.expected_empty || retrieved.is_empty();
    RagQueryResult {
        query_id: spec.id.clone(),
        query: spec.query.clone(),
        relevant_ids: spec.relevant.clone(),
        retrieved_ids: retrieved,
        forbidden_hits,
        expected_empty: spec.expected_empty,
        empty_query_pass,
        recall_at_k: if relevant.is_empty() {
            0.0
        } else {
            hits as f64 / relevant.len() as f64
        },
        precision_at_k: hits as f64 / top_k as f64,
        reciprocal_rank,
        ndcg_at_k: if idcg > 0.0 { dcg / idcg } else { 0.0 },
        latency_p50_ms: quantile(latencies, 0.50).unwrap_or_default(),
        latency_p95_ms: quantile(latencies, 0.95).unwrap_or_default(),
    }
}

fn aggregate_metrics(
    queries: &[RagQueryResult],
    latencies: &[f64],
    cold_index_ms: u64,
) -> RagAggregateMetrics {
    let positive = queries
        .iter()
        .filter(|query| !query.expected_empty)
        .collect::<Vec<_>>();
    let positive_count = positive.len().max(1) as f64;
    let negative = queries
        .iter()
        .filter(|query| query.expected_empty)
        .collect::<Vec<_>>();
    let negative_query_accuracy = if negative.is_empty() {
        1.0
    } else {
        negative
            .iter()
            .filter(|query| query.empty_query_pass)
            .count() as f64
            / negative.len() as f64
    };
    RagAggregateMetrics {
        recall_at_k: positive.iter().map(|query| query.recall_at_k).sum::<f64>() / positive_count,
        precision_at_k: positive
            .iter()
            .map(|query| query.precision_at_k)
            .sum::<f64>()
            / positive_count,
        mrr: positive
            .iter()
            .map(|query| query.reciprocal_rank)
            .sum::<f64>()
            / positive_count,
        ndcg_at_k: positive.iter().map(|query| query.ndcg_at_k).sum::<f64>() / positive_count,
        forbidden_hits: queries.iter().map(|query| query.forbidden_hits.len()).sum(),
        negative_query_accuracy,
        latency_p50_ms: quantile(latencies, 0.50).unwrap_or_default(),
        latency_p95_ms: quantile(latencies, 0.95).unwrap_or_default(),
        cold_index_ms,
    }
}

fn evaluate_gates(metrics: &RagAggregateMetrics, thresholds: &RagThresholds) -> Vec<RagGateResult> {
    vec![
        gate(
            "recall_at_k",
            metrics.recall_at_k >= thresholds.min_recall_at_k,
            format!(
                "{:.3} >= {:.3}",
                metrics.recall_at_k, thresholds.min_recall_at_k
            ),
        ),
        gate(
            "mrr",
            metrics.mrr >= thresholds.min_mrr,
            format!("{:.3} >= {:.3}", metrics.mrr, thresholds.min_mrr),
        ),
        gate(
            "ndcg_at_k",
            metrics.ndcg_at_k >= thresholds.min_ndcg_at_k,
            format!(
                "{:.3} >= {:.3}",
                metrics.ndcg_at_k, thresholds.min_ndcg_at_k
            ),
        ),
        gate(
            "latency_p95_ms",
            metrics.latency_p95_ms <= thresholds.max_p95_ms as f64,
            format!(
                "{:.1} <= {} ms",
                metrics.latency_p95_ms, thresholds.max_p95_ms
            ),
        ),
        gate(
            "cold_index_ms",
            metrics.cold_index_ms <= thresholds.max_cold_index_ms,
            format!(
                "{} <= {} ms",
                metrics.cold_index_ms, thresholds.max_cold_index_ms
            ),
        ),
        gate(
            "zero_forbidden_hits",
            !thresholds.require_zero_forbidden_hits || metrics.forbidden_hits == 0,
            format!("forbidden_hits={}", metrics.forbidden_hits),
        ),
        gate(
            "negative_query_accuracy",
            metrics.negative_query_accuracy >= thresholds.min_negative_query_accuracy,
            format!(
                "{:.3} >= {:.3}",
                metrics.negative_query_accuracy, thresholds.min_negative_query_accuracy
            ),
        ),
    ]
}

fn gate(name: &str, pass: bool, detail: String) -> RagGateResult {
    RagGateResult {
        name: name.to_string(),
        pass,
        detail,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagComparisonReport {
    pub decision: ComparisonDecision,
    pub reasons: Vec<String>,
    pub dataset_id: String,
    pub recall_delta: f64,
    pub mrr_delta: f64,
    pub ndcg_delta: f64,
    pub forbidden_hit_delta: i64,
    pub negative_query_accuracy_delta: f64,
    pub latency_p95_change: Option<f64>,
    pub cold_index_change: Option<f64>,
}

pub fn compare_rag_reports(
    baseline: &RagBenchmarkReport,
    candidate: &RagBenchmarkReport,
) -> RagComparisonReport {
    let mut reasons = Vec::new();
    if baseline.dataset_id != candidate.dataset_id
        || baseline.dataset_hash.is_empty()
        || baseline.dataset_hash != candidate.dataset_hash
        || baseline.top_k != candidate.top_k
    {
        reasons.push("基线与候选必须使用内容哈希相同的数据集、Evidence 语料和 top_k".to_string());
        return RagComparisonReport {
            decision: ComparisonDecision::InsufficientData,
            reasons,
            dataset_id: candidate.dataset_id.clone(),
            recall_delta: 0.0,
            mrr_delta: 0.0,
            ndcg_delta: 0.0,
            forbidden_hit_delta: 0,
            negative_query_accuracy_delta: 0.0,
            latency_p95_change: None,
            cold_index_change: None,
        };
    }

    let recall_delta = candidate.aggregate.recall_at_k - baseline.aggregate.recall_at_k;
    let mrr_delta = candidate.aggregate.mrr - baseline.aggregate.mrr;
    let ndcg_delta = candidate.aggregate.ndcg_at_k - baseline.aggregate.ndcg_at_k;
    let forbidden_hit_delta =
        candidate.aggregate.forbidden_hits as i64 - baseline.aggregate.forbidden_hits as i64;
    let negative_query_accuracy_delta =
        candidate.aggregate.negative_query_accuracy - baseline.aggregate.negative_query_accuracy;
    let latency_p95_change = relative_change(
        baseline.aggregate.latency_p95_ms,
        candidate.aggregate.latency_p95_ms,
    );
    let cold_index_change = relative_change(
        baseline.aggregate.cold_index_ms as f64,
        candidate.aggregate.cold_index_ms as f64,
    );

    if !candidate.hard_gates_passed {
        reasons.push("候选版本未通过 RAG 硬门禁".to_string());
    }
    if recall_delta < -f64::EPSILON {
        reasons.push(format!("Recall@K 下降 {:.3}", recall_delta.abs()));
    }
    if mrr_delta < -0.02 {
        reasons.push(format!("MRR 下降超过 0.02：{mrr_delta:.3}"));
    }
    if ndcg_delta < -0.02 {
        reasons.push(format!("nDCG@K 下降超过 0.02：{ndcg_delta:.3}"));
    }
    if forbidden_hit_delta > 0 {
        reasons.push("候选版本新增了干扰证据命中".to_string());
    }
    if negative_query_accuracy_delta < -f64::EPSILON {
        reasons.push("候选版本的域外空结果准确率下降".to_string());
    }
    if latency_p95_change.is_some_and(|change| change > 0.10) {
        reasons.push("检索 P95 延迟恶化超过 10%".to_string());
    }

    let meaningful_gain = recall_delta >= 0.05
        || mrr_delta >= 0.05
        || ndcg_delta >= 0.05
        || latency_p95_change.is_some_and(|change| change <= -0.15)
        || cold_index_change.is_some_and(|change| change <= -0.15);
    let decision = if !reasons.is_empty() {
        ComparisonDecision::Reject
    } else if meaningful_gain {
        reasons.push("通过非劣效门，且至少一个 RAG 主指标显著改善".to_string());
        ComparisonDecision::Accept
    } else {
        reasons.push("通过硬门，但收益未超过自然波动阈值".to_string());
        ComparisonDecision::InsufficientData
    };
    RagComparisonReport {
        decision,
        reasons,
        dataset_id: candidate.dataset_id.clone(),
        recall_delta,
        mrr_delta,
        ndcg_delta,
        forbidden_hit_delta,
        negative_query_accuracy_delta,
        latency_p95_change,
        cold_index_change,
    }
}

pub fn write_report(path: &Path, report: &impl Serialize) -> Result<(), String> {
    if path.exists() {
        return Err(format!("拒绝覆盖已有评测报告: {}", path.display()));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("创建评测报告目录失败: {error}"))?;
    }
    let content = serde_json::to_string_pretty(report)
        .map_err(|error| format!("序列化 RAG 评测报告失败: {error}"))?;
    std::fs::write(path, content).map_err(|error| format!("写入 RAG 评测报告失败: {error}"))
}

pub fn load_report(path: &Path) -> Result<RagBenchmarkReport, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("读取 RAG 评测报告 {} 失败: {error}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|error| format!("解析 RAG 评测报告 {} 失败: {error}", path.display()))
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn quantile(values: &[f64], probability: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let position = probability.clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let weight = position - lower as f64;
    Some(sorted[lower] * (1.0 - weight) + sorted[upper] * weight)
}

fn relative_change(baseline: f64, candidate: f64) -> Option<f64> {
    if baseline > 0.0 {
        Some((candidate - baseline) / baseline)
    } else {
        None
    }
}

fn dataset_hash(spec: &RagBenchmarkSpec, workspace: &Path) -> Result<String, String> {
    let evidence = crate::evidence::load_evidence(workspace)?;
    let encoded = serde_json::to_vec(&(spec, evidence))
        .map_err(|error| format!("计算 RAG 数据集哈希失败: {error}"))?;
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in encoded {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Ok(format!("{hash:016x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranking_metrics_reward_early_relevant_hits() {
        let query = RagQuerySpec {
            id: "q1".to_string(),
            query: "施罗斯".to_string(),
            relevant: vec!["e1".to_string(), "e2".to_string()],
            forbidden: vec!["bad".to_string()],
            expected_empty: false,
        };
        let result = score_query(
            &query,
            &["e1".to_string(), "noise".to_string(), "e2".to_string()],
            3,
            &[1.0, 2.0, 3.0],
        );
        assert_eq!(result.recall_at_k, 1.0);
        assert!((result.precision_at_k - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(result.reciprocal_rank, 1.0);
        assert!(result.ndcg_at_k > 0.9);
        assert!(result.forbidden_hits.is_empty());
    }

    #[test]
    fn forbidden_hit_is_observable() {
        let query = RagQuerySpec {
            id: "q1".to_string(),
            query: "脊柱侧弯".to_string(),
            relevant: vec!["e1".to_string()],
            forbidden: vec!["stroke".to_string()],
            expected_empty: false,
        };
        let result = score_query(&query, &["stroke".to_string(), "e1".to_string()], 2, &[1.0]);
        assert_eq!(result.forbidden_hits, vec!["stroke"]);
        assert_eq!(result.reciprocal_rank, 0.5);
    }

    #[test]
    fn negative_query_passes_only_with_empty_results() {
        let query = RagQuerySpec {
            id: "ood".to_string(),
            query: "糖尿病足溃疡".to_string(),
            relevant: Vec::new(),
            forbidden: Vec::new(),
            expected_empty: true,
        };
        let empty = score_query(&query, &[], 3, &[1.0]);
        assert!(empty.empty_query_pass);
        let polluted = score_query(&query, &["ev-ais".to_string()], 3, &[1.0]);
        assert!(!polluted.empty_query_pass);
    }

    #[test]
    fn comparator_rejects_recall_regression() {
        let report = |recall: f64, p95: f64| RagBenchmarkReport {
            schema_version: 1,
            dataset_id: "ais".to_string(),
            dataset_name: "AIS".to_string(),
            dataset_hash: "same".to_string(),
            engine: "tantivy-bm25".to_string(),
            tokenizer: "jieba".to_string(),
            commit: "c".to_string(),
            started_at_ms: 0,
            repeat: 5,
            top_k: 5,
            hard_gates_passed: true,
            aggregate: RagAggregateMetrics {
                recall_at_k: recall,
                mrr: 1.0,
                ndcg_at_k: 1.0,
                latency_p95_ms: p95,
                ..RagAggregateMetrics::default()
            },
            gates: Vec::new(),
            queries: Vec::new(),
        };
        let comparison = compare_rag_reports(&report(1.0, 10.0), &report(0.8, 5.0));
        assert!(matches!(comparison.decision, ComparisonDecision::Reject));
        assert!(comparison
            .reasons
            .iter()
            .any(|reason| reason.contains("Recall")));
    }
}
