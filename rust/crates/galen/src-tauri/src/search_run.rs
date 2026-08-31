//! Durable, task-scoped provenance for literature search attempts.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_ERROR_LENGTH: usize = 1_000;

static NEXT_RUN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchRunStatus {
    Succeeded,
    Failed,
    Partial,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchRun {
    pub id: String,
    pub task_id: String,
    pub provider_id: String,
    pub tool_name: String,
    pub query: String,
    #[serde(default)]
    pub arguments: Value,
    pub started_at: String,
    pub finished_at: String,
    pub status: SearchRunStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub raw_result_hash: String,
}

impl SearchRun {
    pub fn succeeded(
        task_id: impl Into<String>,
        provider_id: impl Into<String>,
        tool_name: impl Into<String>,
        query: impl AsRef<str>,
        result_count: usize,
        raw_result_hash: impl Into<String>,
    ) -> Self {
        Self::terminal(
            task_id,
            provider_id,
            tool_name,
            query.as_ref(),
            SearchRunStatus::Succeeded,
            Some(result_count),
            None,
            raw_result_hash,
        )
    }

    pub fn failed(
        task_id: impl Into<String>,
        provider_id: impl Into<String>,
        tool_name: impl Into<String>,
        query: impl AsRef<str>,
        error: impl AsRef<str>,
        raw_result_hash: impl Into<String>,
    ) -> Self {
        Self::terminal(
            task_id,
            provider_id,
            tool_name,
            query.as_ref(),
            SearchRunStatus::Failed,
            None,
            Some(bound_error(error.as_ref())),
            raw_result_hash,
        )
    }

    pub fn partial(
        task_id: impl Into<String>,
        provider_id: impl Into<String>,
        tool_name: impl Into<String>,
        query: impl AsRef<str>,
        result_count: Option<usize>,
        error: impl AsRef<str>,
        raw_result_hash: impl Into<String>,
    ) -> Self {
        Self::terminal(
            task_id,
            provider_id,
            tool_name,
            query.as_ref(),
            SearchRunStatus::Partial,
            result_count,
            Some(bound_error(error.as_ref())),
            raw_result_hash,
        )
    }

    pub fn with_arguments(mut self, arguments: Value) -> Self {
        self.arguments = arguments;
        self
    }

    fn terminal(
        task_id: impl Into<String>,
        provider_id: impl Into<String>,
        tool_name: impl Into<String>,
        query: &str,
        status: SearchRunStatus,
        result_count: Option<usize>,
        error: Option<String>,
        raw_result_hash: impl Into<String>,
    ) -> Self {
        let started_at = now_timestamp();
        Self {
            id: next_run_id(),
            task_id: task_id.into(),
            provider_id: provider_id.into(),
            tool_name: tool_name.into(),
            query: normalize_query(query),
            arguments: Value::Null,
            finished_at: now_timestamp(),
            started_at,
            status,
            result_count,
            error,
            raw_result_hash: raw_result_hash.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CoverageState {
    Searched,
    Failed,
    ConnectedNotSearched,
    ConfiguredDisabled,
    Unavailable,
    NotConfigured,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDescriptor {
    pub provider_id: String,
    pub configured: bool,
    pub enabled: bool,
    pub connected: bool,
}

impl ProviderDescriptor {
    pub fn configured(provider_id: impl Into<String>, enabled: bool, connected: bool) -> Self {
        Self {
            provider_id: provider_id.into(),
            configured: true,
            enabled,
            connected,
        }
    }

    pub fn not_configured(provider_id: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            configured: false,
            enabled: false,
            connected: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCoverage {
    pub provider_id: String,
    pub state: CoverageState,
    pub has_successful_history: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_run: Option<SearchRun>,
}

/// Append one terminal search run to the active task's JSONL ledger.
pub fn append_search_run(workspace: &Path, run: &SearchRun) -> Result<(), String> {
    validate_task_id(&run.task_id)?;
    let path = search_runs_path(workspace, &run.task_id);
    let parent = path
        .parent()
        .ok_or("search run path does not have a parent directory")?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create search run directory failed: {error}"))?;
    let line = serde_json::to_string(run)
        .map_err(|error| format!("serialize search run failed: {error}"))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("open search run ledger failed: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("append search run failed: {error}"))
}

/// Load every prior terminal search run for one task in append order.
pub fn load_search_runs(workspace: &Path, task_id: &str) -> Result<Vec<SearchRun>, String> {
    validate_task_id(task_id)?;
    let path = search_runs_path(workspace, task_id);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("read search run ledger failed: {error}"))?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .map(|(index, line)| {
            serde_json::from_str(line)
                .map_err(|error| format!("invalid search run at line {}: {error}", index + 1))
        })
        .collect()
}

/// Derive current coverage from current provider availability and durable history.
pub fn derive_coverage(
    providers: &[ProviderDescriptor],
    runs: &[SearchRun],
) -> BTreeMap<String, ProviderCoverage> {
    providers
        .iter()
        .map(|provider| {
            let provider_runs: Vec<&SearchRun> = runs
                .iter()
                .filter(|run| run.provider_id == provider.provider_id)
                .collect();
            let latest_run = provider_runs
                .iter()
                .enumerate()
                .max_by_key(|(index, run)| (timestamp_key(&run.finished_at), *index))
                .map(|(_, run)| (*run).clone());
            let has_successful_history = provider_runs
                .iter()
                .any(|run| run.status == SearchRunStatus::Succeeded);
            let state = coverage_state(provider, latest_run.as_ref());
            (
                provider.provider_id.clone(),
                ProviderCoverage {
                    provider_id: provider.provider_id.clone(),
                    state,
                    has_successful_history,
                    latest_run,
                },
            )
        })
        .collect()
}

fn coverage_state(provider: &ProviderDescriptor, latest_run: Option<&SearchRun>) -> CoverageState {
    if !provider.configured {
        CoverageState::NotConfigured
    } else if !provider.enabled {
        CoverageState::ConfiguredDisabled
    } else if !provider.connected {
        CoverageState::Unavailable
    } else if matches!(
        latest_run.map(|run| &run.status),
        Some(SearchRunStatus::Succeeded)
    ) {
        CoverageState::Searched
    } else if latest_run.is_some() {
        CoverageState::Failed
    } else {
        CoverageState::ConnectedNotSearched
    }
}

fn search_runs_path(workspace: &Path, task_id: &str) -> PathBuf {
    workspace
        .join(".galen")
        .join("tasks")
        .join(task_id)
        .join("search-runs.jsonl")
}

fn validate_task_id(task_id: &str) -> Result<(), String> {
    if task_id.is_empty()
        || !task_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err("invalid task ID".to_string());
    }
    Ok(())
}

fn normalize_query(query: &str) -> String {
    query.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn bound_error(error: &str) -> String {
    error.chars().take(MAX_ERROR_LENGTH).collect()
}

fn now_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

fn next_run_id() -> String {
    let sequence = NEXT_RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("run-{}-{}-{sequence}", now_timestamp(), std::process::id())
}

fn timestamp_key(value: &str) -> u128 {
    value.parse().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn temp_workspace(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "galen-search-run-test-{tag}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn append_and_load_preserves_zero_result_success() {
        // Removing result_count or serializing zero as absent must fail this test.
        let root = temp_workspace("zero-result");
        let run = SearchRun::succeeded("task-1", "pubmed", "search_pubmed", "stroke", 0, "abc");

        append_search_run(&root, &run).unwrap();
        let loaded = load_search_runs(&root, "task-1").unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, run.id);
        assert_eq!(loaded[0].status, SearchRunStatus::Succeeded);
        assert_eq!(loaded[0].result_count, Some(0));
        assert_eq!(loaded[0].raw_result_hash, "abc");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn append_rejects_task_ids_that_escape_the_task_store() {
        // Removing task-ID validation could write a ledger outside .galen/tasks.
        let root = temp_workspace("task-id");
        let run = SearchRun::succeeded("../escape", "pubmed", "search_pubmed", "stroke", 1, "abc");

        assert!(append_search_run(&root, &run).is_err());
        assert!(!root.join(".galen/escape/search-runs.jsonl").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn coverage_uses_latest_attempt_without_erasing_prior_search_history() {
        // Returning Searched whenever any past success exists would hide a later failure.
        let providers = vec![ProviderDescriptor::configured("pubmed", true, true)];
        let mut success =
            SearchRun::succeeded("task-1", "pubmed", "search_pubmed", "stroke", 0, "first");
        success.finished_at = "100".to_string();
        let mut later_failure = SearchRun::failed(
            "task-1",
            "pubmed",
            "search_pubmed",
            "stroke",
            "timeout",
            "second",
        );
        later_failure.finished_at = "200".to_string();

        let coverage = derive_coverage(&providers, &[success, later_failure]);

        assert_eq!(coverage["pubmed"].state, CoverageState::Failed);
        assert!(coverage["pubmed"].has_successful_history);
    }

    #[test]
    fn coverage_distinguishes_every_provider_state() {
        // Collapsing configured state into searched or failed would conceal coverage limits.
        let providers = vec![
            ProviderDescriptor::configured("searched", true, true),
            ProviderDescriptor::configured("failed", true, true),
            ProviderDescriptor::configured("connected", true, true),
            ProviderDescriptor::configured("disabled", false, false),
            ProviderDescriptor::configured("unavailable", true, false),
            ProviderDescriptor::not_configured("not-configured"),
        ];
        let mut searched = SearchRun::succeeded("task-1", "searched", "search", "stroke", 0, "a");
        searched.finished_at = "100".to_string();
        let mut failed = SearchRun::failed("task-1", "failed", "search", "stroke", "timeout", "b");
        failed.finished_at = "200".to_string();

        let coverage = derive_coverage(&providers, &[searched, failed]);

        assert_eq!(coverage["searched"].state, CoverageState::Searched);
        assert_eq!(coverage["failed"].state, CoverageState::Failed);
        assert_eq!(
            coverage["connected"].state,
            CoverageState::ConnectedNotSearched
        );
        assert_eq!(
            coverage["disabled"].state,
            CoverageState::ConfiguredDisabled
        );
        assert_eq!(coverage["unavailable"].state, CoverageState::Unavailable);
        assert_eq!(
            coverage["not-configured"].state,
            CoverageState::NotConfigured
        );
    }

    #[test]
    fn failed_runs_bound_error_text_and_normalize_the_query() {
        // Removing either safeguard could persist unbounded diagnostics or inconsistent queries.
        let long_error = "x".repeat(MAX_ERROR_LENGTH + 1);
        let run = SearchRun::failed(
            "task-1",
            "pubmed",
            "search_pubmed",
            "  stroke\n  rehabilitation  ",
            &long_error,
            "abc",
        );

        assert_eq!(run.query, "stroke rehabilitation");
        assert_eq!(run.error.as_deref().map(str::len), Some(MAX_ERROR_LENGTH));
    }

    #[test]
    fn provider_descriptors_can_be_indexed_by_their_id() {
        let providers = vec![ProviderDescriptor::configured("pubmed", true, true)];
        let coverage = derive_coverage(&providers, &[]);

        assert_eq!(
            coverage,
            BTreeMap::from([(
                "pubmed".to_string(),
                ProviderCoverage {
                    provider_id: "pubmed".to_string(),
                    state: CoverageState::ConnectedNotSearched,
                    has_successful_history: false,
                    latest_run: None,
                },
            )])
        );
    }
}
