//! Human-readable evaluation report rendering.

use std::path::Path;

use crate::eval::{reliability_report, RunRecord};
use crate::rag_eval::RagBenchmarkReport;

#[must_use]
pub fn render_markdown_report(
    title: &str,
    agent_records: &[RunRecord],
    rag: Option<&RagBenchmarkReport>,
) -> String {
    let mut out = format!("# {title}\n\n");
    out.push_str("> 由 Galen Eval 自动生成。原始 JSON/JSONL 是事实源，本报告用于审阅与演示。\n\n");

    if !agent_records.is_empty() {
        render_agent_section(&mut out, agent_records);
    }
    if let Some(rag) = rag {
        render_rag_section(&mut out, rag);
    }
    if agent_records.is_empty() && rag.is_none() {
        out.push_str("没有提供可汇总的 Agent 或 RAG 运行记录。\n");
    }
    out
}

fn render_agent_section(out: &mut String, records: &[RunRecord]) {
    let reliability = reliability_report(records, 5);
    let total_values = records
        .iter()
        .map(|record| record.latency.total_ms as f64)
        .collect::<Vec<_>>();
    let ttfr_values = records
        .iter()
        .filter_map(|record| record.latency.ttfr_ms.map(|value| value as f64))
        .collect::<Vec<_>>();
    let tokens = records
        .iter()
        .map(|record| record.usage.total() as f64)
        .collect::<Vec<_>>();
    out.push_str("## Agent 端到端可靠性\n\n");
    out.push_str("| 指标 | 结果 |\n|---|---:|\n");
    out.push_str(&format!("| 运行数 | {} |\n", reliability.runs));
    out.push_str(&format!(
        "| 硬门通过 | {}/{} |\n",
        reliability.successes, reliability.runs
    ));
    out.push_str(&format!(
        "| 成功率 | {:.1}% |\n",
        reliability.success_rate * 100.0
    ));
    out.push_str(&format!(
        "| Wilson 95% 下界 | {:.1}% |\n",
        reliability.wilson_lower_95 * 100.0
    ));
    out.push_str(&format!(
        "| pass^5 | {} |\n",
        format_optional(reliability.pass_k, true)
    ));
    out.push_str(&format!(
        "| Galen Agent Index | {:.1} |\n",
        reliability.galen_agent_index
    ));
    out.push_str(&format!(
        "| 总耗时 P50 / P95 | {:.0} / {:.0} ms |\n",
        quantile(&total_values, 0.50),
        quantile(&total_values, 0.95)
    ));
    out.push_str(&format!(
        "| TTFR P50 / P95 | {:.0} / {:.0} ms |\n",
        quantile(&ttfr_values, 0.50),
        quantile(&ttfr_values, 0.95)
    ));
    out.push_str(&format!("| Token 平均值 | {:.0} |\n", mean(&tokens)));
    out.push_str(&format!(
        "| 证据检索覆盖率 | {} |\n",
        format_optional(reliability.grounding.retrieval_coverage, true)
    ));
    out.push_str(&format!(
        "| 证据引用覆盖率 | {} |\n",
        format_optional(reliability.grounding.citation_coverage, true)
    ));
    out.push_str(&format!(
        "| 禁止证据命中 | {} |\n",
        reliability.grounding.forbidden_hits
    ));
    out.push_str(&format!(
        "| 本地 / 外部检索调用 | {} / {} |\n\n",
        reliability.grounding.local_search_calls, reliability.grounding.external_search_calls
    ));

    out.push_str("### 分案例结果\n\n");
    out.push_str("| Case | Model | 通过 | 成功率 | Lower95 | 质量 | 引用覆盖 |\n|---|---|---:|---:|---:|---:|---:|\n");
    for case in &reliability.cases {
        out.push_str(&format!(
            "| {} | {} | {}/{} | {:.1}% | {:.1}% | {:.3} | {} |\n",
            case.case_id,
            case.model,
            case.successes,
            case.runs,
            case.success_rate * 100.0,
            case.wilson_lower_95 * 100.0,
            case.quality_mean,
            format_optional(case.grounding.citation_coverage, true)
        ));
    }

    let failures = records
        .iter()
        .flat_map(|record| {
            record
                .assertions
                .iter()
                .filter(|assertion| !assertion.pass)
                .map(move |assertion| (record, assertion))
        })
        .collect::<Vec<_>>();
    out.push_str("\n### 硬门失败\n\n");
    if failures.is_empty() {
        out.push_str("无。\n\n");
    } else {
        for (record, assertion) in failures {
            out.push_str(&format!(
                "- `{}` run {} — `{}`：{}\n",
                record.case_id, record.run_index, assertion.name, assertion.detail
            ));
        }
        out.push('\n');
    }
}

fn render_rag_section(out: &mut String, rag: &RagBenchmarkReport) {
    let status = if rag.hard_gates_passed {
        "PASS"
    } else {
        "FAIL"
    };
    out.push_str("## RAG 组件基准\n\n");
    out.push_str(&format!(
        "- 数据集：`{}`（hash `{}`）\n- 引擎：`{}` / `{}`\n- Git：`{}`\n- 结论：**{}**\n\n",
        rag.dataset_id, rag.dataset_hash, rag.engine, rag.tokenizer, rag.commit, status
    ));
    out.push_str("| 指标 | 结果 |\n|---|---:|\n");
    out.push_str(&format!(
        "| Recall@{} | {:.3} |\n",
        rag.top_k, rag.aggregate.recall_at_k
    ));
    out.push_str(&format!(
        "| Precision@{} | {:.3} |\n",
        rag.top_k, rag.aggregate.precision_at_k
    ));
    out.push_str(&format!("| MRR | {:.3} |\n", rag.aggregate.mrr));
    out.push_str(&format!(
        "| nDCG@{} | {:.3} |\n",
        rag.top_k, rag.aggregate.ndcg_at_k
    ));
    out.push_str(&format!(
        "| 域外空结果准确率 | {:.3} |\n",
        rag.aggregate.negative_query_accuracy
    ));
    out.push_str(&format!(
        "| 禁止证据命中 | {} |\n",
        rag.aggregate.forbidden_hits
    ));
    out.push_str(&format!(
        "| 热检索 P50 / P95 | {:.1} / {:.1} ms |\n",
        rag.aggregate.latency_p50_ms, rag.aggregate.latency_p95_ms
    ));
    out.push_str(&format!(
        "| 冷建索引 | {} ms |\n\n",
        rag.aggregate.cold_index_ms
    ));

    out.push_str("### RAG 硬门\n\n");
    for gate in &rag.gates {
        out.push_str(&format!(
            "- [{}] `{}`：{}\n",
            if gate.pass { "x" } else { " " },
            gate.name,
            gate.detail
        ));
    }

    out.push_str("\n### 逐查询排名\n\n");
    out.push_str(
        "| Query | 类型 | Recall | RR | nDCG | 返回 Evidence |\n|---|---|---:|---:|---:|---|\n",
    );
    for query in &rag.queries {
        out.push_str(&format!(
            "| {} | {} | {:.3} | {:.3} | {:.3} | {} |\n",
            query.query_id,
            if query.expected_empty {
                "域外负查询"
            } else {
                "正查询"
            },
            query.recall_at_k,
            query.reciprocal_rank,
            query.ndcg_at_k,
            if query.retrieved_ids.is_empty() {
                "—".to_string()
            } else {
                query.retrieved_ids.join("<br>")
            }
        ));
    }
    out.push('\n');
}

pub fn write_markdown_report(path: &Path, content: &str) -> Result<(), String> {
    if path.exists() {
        return Err(format!("拒绝覆盖已有评测报告: {}", path.display()));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("创建评测报告目录失败: {error}"))?;
    }
    std::fs::write(path, content).map_err(|error| format!("写入评测报告失败: {error}"))
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn quantile(values: &[f64], probability: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let position = probability.clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let weight = position - lower as f64;
    sorted[lower] * (1.0 - weight) + sorted[upper] * weight
}

fn format_optional(value: Option<f64>, percentage: bool) -> String {
    value.map_or_else(
        || "N/A".to_string(),
        |value| {
            if percentage {
                format!("{:.1}%", value * 100.0)
            } else {
                format!("{value:.3}")
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rag_eval::{RagAggregateMetrics, RagBenchmarkReport};

    #[test]
    fn renders_rag_metrics_and_reproducibility_metadata() {
        let rag = RagBenchmarkReport {
            schema_version: 1,
            dataset_id: "ais".to_string(),
            dataset_name: "AIS".to_string(),
            dataset_hash: "abc123".to_string(),
            engine: "tantivy-bm25".to_string(),
            tokenizer: "jieba".to_string(),
            commit: "deadbeef-dirty".to_string(),
            started_at_ms: 0,
            repeat: 5,
            top_k: 3,
            hard_gates_passed: true,
            aggregate: RagAggregateMetrics {
                recall_at_k: 1.0,
                mrr: 1.0,
                ndcg_at_k: 1.0,
                negative_query_accuracy: 1.0,
                ..RagAggregateMetrics::default()
            },
            gates: Vec::new(),
            queries: Vec::new(),
        };
        let report = render_markdown_report("测试", &[], Some(&rag));
        assert!(report.contains("Recall@3"));
        assert!(report.contains("abc123"));
        assert!(report.contains("deadbeef-dirty"));
    }
}
