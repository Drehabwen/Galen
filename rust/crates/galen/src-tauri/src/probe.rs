//! Headless end-to-end probes for Galen's delivery loop.

use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::artifact::ArtifactRecord;
use crate::backend::{ChatRunSummary, ToolTrace};
use crate::research_task::ResearchTask;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeEventCounts {
    pub artifact_created: usize,
    pub research_task_updated: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeAssertion {
    pub name: String,
    pub pass: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeMetrics {
    pub context_ms: u64,
    pub mcp_ms: u64,
    pub ttft_ms: Option<u64>,
    pub ttfr_ms: Option<u64>,
    pub total_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub model_requests: u32,
    pub tool_calls: usize,
    pub compactions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeTaskSnapshot {
    pub task_id: String,
    pub status: String,
    pub node_count: usize,
    pub completed_nodes: usize,
    pub artifact_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosedLoopProbeReport {
    pub schema_version: u32,
    pub scenario: String,
    pub generated_at_ms: u128,
    pub passed: bool,
    pub model: String,
    pub workspace: String,
    pub expected_artifact: String,
    pub run_error: Option<String>,
    pub response: String,
    pub metrics: ProbeMetrics,
    pub events: ProbeEventCounts,
    pub tool_names: Vec<String>,
    pub max_repeated_call: usize,
    pub task: Option<ProbeTaskSnapshot>,
    pub artifacts: Vec<ArtifactRecord>,
    pub assertions: Vec<ProbeAssertion>,
}

pub struct ClosedLoopObservation<'a> {
    pub workspace: &'a Path,
    pub model: &'a str,
    pub expected_artifact: &'a str,
    pub response: &'a str,
    pub run_error: Option<String>,
    pub summary: &'a ChatRunSummary,
    pub traces: &'a [ToolTrace],
    pub events: ProbeEventCounts,
}

pub fn evaluate_closed_loop(observation: ClosedLoopObservation<'_>) -> ClosedLoopProbeReport {
    let task = crate::research_task::load_active_task(observation.workspace)
        .ok()
        .flatten();
    let artifacts = crate::artifact::list_artifacts(observation.workspace).unwrap_or_default();
    let ordinary_traces = observation
        .traces
        .iter()
        .filter(|trace| trace.tool != "__convergence__")
        .collect::<Vec<_>>();
    let mut tool_names = Vec::new();
    let mut repeats: HashMap<(&str, &str), usize> = HashMap::new();
    for trace in &ordinary_traces {
        if !tool_names.contains(&trace.tool) {
            tool_names.push(trace.tool.clone());
        }
        *repeats
            .entry((trace.tool.as_str(), trace.input.as_str()))
            .or_default() += 1;
    }
    let max_repeated_call = repeats.values().copied().max().unwrap_or_default();
    let expected_file = observation.workspace.join(observation.expected_artifact);
    let file_valid = expected_file
        .metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0);
    let registered = artifacts
        .iter()
        .find(|artifact| artifact.path == observation.expected_artifact);
    let bound = registered.is_some_and(|artifact| {
        artifact
            .task_id
            .as_deref()
            .is_some_and(|value| !value.is_empty())
            && artifact
                .node_id
                .as_deref()
                .is_some_and(|value| !value.is_empty())
    });
    let task_has_artifact = match (&task, registered) {
        (Some(task), Some(artifact)) => {
            task.artifact_ids.contains(&artifact.id)
                && task.nodes.iter().any(|node| {
                    node.outputs.contains(&artifact.path)
                        && artifact.node_id.as_deref() == Some(node.id.as_str())
                })
        }
        _ => false,
    };
    let previewable = registered.is_some_and(|artifact| {
        matches!(
            artifact.mime_type.as_str(),
            "text/markdown" | "text/plain" | "text/csv"
        )
    });

    let mut assertions = Vec::new();
    let mut add = |name: &str, pass: bool, detail: String| {
        assertions.push(ProbeAssertion {
            name: name.to_string(),
            pass,
            detail,
        });
    };
    add(
        "run_completed",
        observation.run_error.is_none(),
        observation
            .run_error
            .clone()
            .unwrap_or_else(|| "Ok".to_string()),
    );
    add(
        "model_request_budget",
        observation.summary.model_request_count <= 6,
        format!("{}/6", observation.summary.model_request_count),
    );
    add(
        "tool_call_budget",
        ordinary_traces.len() <= 8,
        format!("{}/8", ordinary_traces.len()),
    );
    add(
        "no_tool_errors",
        ordinary_traces.iter().all(|trace| !trace.is_error),
        format!(
            "errors={}",
            ordinary_traces
                .iter()
                .filter(|trace| trace.is_error)
                .count()
        ),
    );
    add(
        "no_chat_error_events",
        observation.events.errors == 0,
        format!("errors={}", observation.events.errors),
    );
    add(
        "non_empty_final_response",
        !observation.response.trim().is_empty(),
        format!("chars={}", observation.response.chars().count()),
    );
    add(
        "no_repeated_call_loop",
        max_repeated_call <= 2,
        format!("max_repeat={max_repeated_call}"),
    );
    add(
        "create_research_plan_called",
        tool_names.iter().any(|name| name == "create_research_plan"),
        format!("observed={}", tool_names.join(",")),
    );
    add(
        "write_file_called",
        tool_names.iter().any(|name| name == "write_file"),
        format!("observed={}", tool_names.join(",")),
    );
    add(
        "artifact_file_valid",
        file_valid,
        expected_file.display().to_string(),
    );
    add(
        "artifact_registered",
        registered.is_some(),
        format!("registry_entries={}", artifacts.len()),
    );
    add(
        "artifact_bound_to_task_and_node",
        bound,
        format!("bound={bound}"),
    );
    add(
        "research_task_has_three_nodes",
        task.as_ref().is_some_and(|value| value.nodes.len() >= 3),
        format!(
            "nodes={}",
            task.as_ref().map_or(0, |value| value.nodes.len())
        ),
    );
    add(
        "at_least_one_node_completed",
        task.as_ref()
            .is_some_and(|value| value.nodes.iter().any(|node| node.status == "completed")),
        format!(
            "completed={}",
            task.as_ref().map_or(0, |value| value
                .nodes
                .iter()
                .filter(|node| node.status == "completed")
                .count())
        ),
    );
    add(
        "task_artifact_bidirectional_link",
        task_has_artifact,
        format!("linked={task_has_artifact}"),
    );
    add(
        "research_task_event_emitted",
        observation.events.research_task_updated > 0,
        format!("count={}", observation.events.research_task_updated),
    );
    add(
        "artifact_event_emitted",
        observation.events.artifact_created > 0,
        format!("count={}", observation.events.artifact_created),
    );
    add(
        "galen_preview_contract",
        previewable && file_valid,
        registered
            .map(|artifact| artifact.mime_type.clone())
            .unwrap_or_else(|| "missing".to_string()),
    );

    let task_snapshot = task.as_ref().map(task_snapshot);
    let passed = assertions.iter().all(|assertion| assertion.pass);
    ClosedLoopProbeReport {
        schema_version: 1,
        scenario: "closed-loop".to_string(),
        generated_at_ms: now_millis(),
        passed,
        model: observation.model.to_string(),
        workspace: observation.workspace.display().to_string(),
        expected_artifact: observation.expected_artifact.to_string(),
        run_error: observation.run_error,
        response: observation.response.to_string(),
        metrics: metrics(observation.summary),
        events: observation.events,
        tool_names,
        max_repeated_call,
        task: task_snapshot,
        artifacts,
        assertions,
    }
}

fn task_snapshot(task: &ResearchTask) -> ProbeTaskSnapshot {
    ProbeTaskSnapshot {
        task_id: task.task_id.clone(),
        status: serde_json::to_value(&task.status)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| "unknown".to_string()),
        node_count: task.nodes.len(),
        completed_nodes: task
            .nodes
            .iter()
            .filter(|node| node.status == "completed")
            .count(),
        artifact_ids: task.artifact_ids.clone(),
    }
}

fn metrics(summary: &ChatRunSummary) -> ProbeMetrics {
    ProbeMetrics {
        context_ms: summary.context_assembly_ms,
        mcp_ms: summary.mcp_setup_ms,
        ttft_ms: summary.ttft_ms,
        ttfr_ms: summary.ttfr_ms,
        total_ms: summary.total_ms,
        input_tokens: summary.input_tokens,
        output_tokens: summary.output_tokens,
        cache_creation_tokens: summary.cache_creation_input_tokens,
        cache_read_tokens: summary.cache_read_input_tokens,
        model_requests: summary.model_request_count,
        tool_calls: summary.tool_call_count,
        compactions: summary.compaction_count,
    }
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact;
    use crate::research_task::{self, ResearchNode};
    use std::collections::BTreeMap;

    fn temp_workspace() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("galen-probe-test-{}", now_millis()));
        std::fs::create_dir_all(path.join("output")).unwrap();
        path
    }

    fn node(id: &str, index: &str) -> ResearchNode {
        ResearchNode {
            id: id.to_string(),
            index: index.to_string(),
            title: format!("node {index}"),
            description: None,
            node_type: "analysis".to_string(),
            status: "pending".to_string(),
            owner: None,
            inputs: Vec::new(),
            outputs: Vec::new(),
            depends_on: Vec::new(),
            tags: Vec::new(),
            risk_level: None,
            approval_required: false,
            sub_sessions: Vec::new(),
            result: None,
            evidence: Vec::new(),
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn passes_when_file_registry_task_and_events_form_a_closed_loop() {
        let workspace = temp_workspace();
        let nodes = vec![node("n1", "01"), node("n2", "02"), node("n3", "03")];
        research_task::create_task(&workspace, "task".to_string(), "goal".to_string(), nodes)
            .unwrap();
        std::fs::write(workspace.join("output/result.md"), "# result").unwrap();
        let artifact = artifact::register_file(&workspace, "output/result.md", None, None).unwrap();
        let task =
            research_task::attach_artifact(&workspace, &artifact.id, &artifact.path, Some("n1"))
                .unwrap();
        artifact::link_artifact(&workspace, &artifact.id, &task.task_id, "n1").unwrap();
        let traces = vec![
            ToolTrace {
                turn: 1,
                tool: "create_research_plan".to_string(),
                input: "plan".to_string(),
                output: "ok".to_string(),
                is_error: false,
            },
            ToolTrace {
                turn: 2,
                tool: "write_file".to_string(),
                input: "file".to_string(),
                output: "ok".to_string(),
                is_error: false,
            },
        ];
        let summary = ChatRunSummary {
            model_request_count: 2,
            tool_call_count: 2,
            ..ChatRunSummary::default()
        };
        let report = evaluate_closed_loop(ClosedLoopObservation {
            workspace: &workspace,
            model: "test",
            expected_artifact: "output/result.md",
            response: "done",
            run_error: None,
            summary: &summary,
            traces: &traces,
            events: ProbeEventCounts {
                artifact_created: 1,
                research_task_updated: 2,
                errors: 0,
            },
        });
        assert!(report.passed, "{:#?}", report.assertions);
    }
}
