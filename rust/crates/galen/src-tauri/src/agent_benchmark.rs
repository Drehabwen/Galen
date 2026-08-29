use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct AgentBenchmarkReport {
    pub case_id: String,
    pub runs: Vec<AgentBenchmarkRun>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentBenchmarkRun {
    pub profile: String,
    pub model: String,
    pub samples: usize,
    pub pass_rate: f64,
    pub mean_ttfr_ms: u64,
    pub p95_ttfr_ms: u64,
    pub mean_total_ms: u64,
    pub p95_total_ms: u64,
    pub mean_input_tokens: u64,
    pub mean_output_tokens: u64,
}

#[derive(Deserialize)]
struct NativeRecord {
    case_id: String,
    model: String,
    hard_gates_passed: bool,
    latency: NativeLatency,
    usage: NativeUsage,
}

#[derive(Deserialize)]
struct NativeLatency {
    ttfr_ms: Option<u64>,
    total_ms: u64,
}

#[derive(Deserialize)]
struct NativeUsage {
    input: u64,
    output: u64,
}

pub fn load_latest(root: &Path) -> Result<AgentBenchmarkReport, String> {
    let specs = [
        ("自动路由", "agent-e01-auto-k5.jsonl"),
        ("Flash", "agent-e01-flash-k5.jsonl"),
        ("Pro", "agent-e01-pro-k5.jsonl"),
    ];
    let mut runs = Vec::new();
    let mut case_id = "E01".to_string();
    for (profile, filename) in specs {
        let path = root.join("evals").join("runs").join(filename);
        if !path.is_file() {
            continue;
        }
        let records = fs::read_to_string(&path)
            .map_err(|error| format!("读取 {} 失败: {error}", path.display()))?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str::<NativeRecord>(line).map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        if records.is_empty() {
            continue;
        }
        case_id = records[0].case_id.clone();
        runs.push(summarize(profile, &records));
    }
    Ok(AgentBenchmarkReport { case_id, runs })
}

fn summarize(profile: &str, records: &[NativeRecord]) -> AgentBenchmarkRun {
    let samples = records.len();
    let mut ttfr = records
        .iter()
        .filter_map(|item| item.latency.ttfr_ms)
        .collect::<Vec<_>>();
    let mut total = records
        .iter()
        .map(|item| item.latency.total_ms)
        .collect::<Vec<_>>();
    ttfr.sort_unstable();
    total.sort_unstable();
    AgentBenchmarkRun {
        profile: profile.to_string(),
        model: records[0].model.clone(),
        samples,
        pass_rate: records.iter().filter(|item| item.hard_gates_passed).count() as f64
            / samples as f64,
        mean_ttfr_ms: mean(&ttfr),
        p95_ttfr_ms: p95(&ttfr),
        mean_total_ms: mean(&total),
        p95_total_ms: p95(&total),
        mean_input_tokens: mean(
            &records
                .iter()
                .map(|item| item.usage.input)
                .collect::<Vec<_>>(),
        ),
        mean_output_tokens: mean(
            &records
                .iter()
                .map(|item| item.usage.output)
                .collect::<Vec<_>>(),
        ),
    }
}

fn mean(values: &[u64]) -> u64 {
    if values.is_empty() {
        0
    } else {
        values.iter().sum::<u64>() / values.len() as u64
    }
}

fn p95(values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let index = ((values.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
    values[index.min(values.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_p95_uses_slowest_of_five() {
        assert_eq!(p95(&[1, 2, 3, 4, 9]), 9);
    }
}
