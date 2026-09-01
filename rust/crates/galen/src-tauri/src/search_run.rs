//! Durable, task-scoped provenance for literature search attempts.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

static SEARCH_RUN_STORE_LOCK: Mutex<()> = Mutex::new(());

/// Maximum nested containers retained from one provider argument payload.
pub const MAX_ARGUMENT_DEPTH: usize = 12;
/// JSON byte budget retained from one argument payload before explicit markers.
pub const MAX_ARGUMENT_BYTES: usize = 32 * 1024;
/// Maximum serialized overhead reserved for explicit depth/size markers.
pub const MAX_ARGUMENT_OVERHEAD_BYTES: usize = 1_024;
const REDACTED: &str = "[redacted]";
const TRUNCATED: &str = "[truncated]";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchRunStatus {
    Succeeded,
    Failed,
    Partial,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct Sha256Hash(String);

impl Sha256Hash {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("raw result hash must be exactly 64 hexadecimal SHA-256 characters".into());
        }
        Ok(Self(value.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Sha256Hash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchErrorClass {
    Timeout,
    Authentication,
    RateLimited,
    Unavailable,
    Protocol,
    InvalidResponse,
    Other,
}

impl SearchErrorClass {
    /// Classify a provider error without retaining its potentially sensitive text.
    pub fn classify(error: &str) -> Self {
        let error = error.to_ascii_lowercase();
        if error.contains("timeout") || error.contains("timed out") {
            Self::Timeout
        } else if [
            "auth",
            "unauthor",
            "forbidden",
            "credential",
            "login",
            "api key",
            "api_key",
        ]
        .iter()
        .any(|marker| error.contains(marker))
        {
            Self::Authentication
        } else if ["rate limit", "429", "quota"]
            .iter()
            .any(|marker| error.contains(marker))
        {
            Self::RateLimited
        } else if [
            "unavailable",
            "not found",
            "spawn",
            "connection refused",
            "executable",
            "runtime",
        ]
        .iter()
        .any(|marker| error.contains(marker))
        {
            Self::Unavailable
        } else if ["parse", "deserialize", "malformed", "invalid response"]
            .iter()
            .any(|marker| error.contains(marker))
        {
            Self::InvalidResponse
        } else if error.contains("protocol") || error.contains("rpc") {
            Self::Protocol
        } else {
            Self::Other
        }
    }
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
    arguments: Value,
    pub started_at: String,
    pub finished_at: String,
    pub status: SearchRunStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_class: Option<SearchErrorClass>,
    pub raw_result_hash: Sha256Hash,
}

impl SearchRun {
    pub fn succeeded(
        task_id: impl Into<String>,
        provider_id: impl Into<String>,
        tool_name: impl Into<String>,
        query: impl AsRef<str>,
        arguments: Value,
        started_at: impl Into<String>,
        finished_at: impl Into<String>,
        result_count: usize,
        raw_result_hash: impl Into<String>,
    ) -> Result<Self, String> {
        Self::terminal(
            task_id,
            provider_id,
            tool_name,
            query.as_ref(),
            arguments,
            started_at,
            finished_at,
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
        arguments: Value,
        started_at: impl Into<String>,
        finished_at: impl Into<String>,
        error_class: SearchErrorClass,
        raw_result_hash: impl Into<String>,
    ) -> Result<Self, String> {
        Self::terminal(
            task_id,
            provider_id,
            tool_name,
            query.as_ref(),
            arguments,
            started_at,
            finished_at,
            SearchRunStatus::Failed,
            None,
            Some(error_class),
            raw_result_hash,
        )
    }

    pub fn partial(
        task_id: impl Into<String>,
        provider_id: impl Into<String>,
        tool_name: impl Into<String>,
        query: impl AsRef<str>,
        arguments: Value,
        started_at: impl Into<String>,
        finished_at: impl Into<String>,
        result_count: Option<usize>,
        error_class: SearchErrorClass,
        raw_result_hash: impl Into<String>,
    ) -> Result<Self, String> {
        Self::terminal(
            task_id,
            provider_id,
            tool_name,
            query.as_ref(),
            arguments,
            started_at,
            finished_at,
            SearchRunStatus::Partial,
            result_count,
            Some(error_class),
            raw_result_hash,
        )
    }

    pub fn arguments(&self) -> &Value {
        &self.arguments
    }

    fn terminal(
        task_id: impl Into<String>,
        provider_id: impl Into<String>,
        tool_name: impl Into<String>,
        query: &str,
        arguments: Value,
        started_at: impl Into<String>,
        finished_at: impl Into<String>,
        status: SearchRunStatus,
        result_count: Option<usize>,
        error_class: Option<SearchErrorClass>,
        raw_result_hash: impl Into<String>,
    ) -> Result<Self, String> {
        let started_at = started_at.into();
        let finished_at = finished_at.into();
        validate_timestamps(&started_at, &finished_at)?;
        let run = Self {
            id: uuid::Uuid::new_v4().to_string(),
            task_id: task_id.into(),
            provider_id: provider_id.into(),
            tool_name: tool_name.into(),
            query: sanitize_query(query),
            arguments: project_arguments(&arguments),
            finished_at,
            started_at,
            status,
            result_count,
            error_class,
            raw_result_hash: Sha256Hash::new(raw_result_hash.into())?,
        };
        run.validate()?;
        Ok(run)
    }

    fn validate(&self) -> Result<(), String> {
        validate_task_id(&self.task_id)?;
        validate_timestamps(&self.started_at, &self.finished_at)?;
        Sha256Hash::new(self.raw_result_hash.as_str())?;
        Ok(())
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
    pub latest_query: Option<String>,
    pub latest_finished_at: Option<String>,
    pub result_count: Option<usize>,
    pub error_class: Option<SearchErrorClass>,
}

/// Append one terminal search run to the active task's JSONL ledger.
pub fn append_search_run(workspace: &Path, run: &SearchRun) -> Result<(), String> {
    let _guard = lock_search_run_store()?;
    run.validate()?;
    let path = search_runs_path(workspace, &run.task_id);
    let parent = path
        .parent()
        .ok_or("search run path does not have a parent directory")?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create search run directory failed: {error}"))?;
    let mut safe_run = run.clone();
    safe_run.arguments = project_arguments(&safe_run.arguments);
    safe_run.query = sanitize_query(&safe_run.query);
    let mut line = serde_json::to_string(&safe_run)
        .map_err(|error| format!("serialize search run failed: {error}"))?;
    line.push('\n');
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("open search run ledger failed: {error}"))?;
    file.write_all(line.as_bytes())
        .map_err(|error| format!("append search run failed: {error}"))
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
            let mut run: SearchRun = serde_json::from_str(line)
                .map_err(|error| format!("invalid search run at line {}: {error}", index + 1))?;
            run.validate()
                .map_err(|error| format!("invalid search run at line {}: {error}", index + 1))?;
            run.arguments = project_arguments(&run.arguments);
            run.query = sanitize_query(&run.query);
            Ok(run)
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
                    latest_query: latest_run.as_ref().map(|run| sanitize_query(&run.query)),
                    latest_finished_at: latest_run.as_ref().map(|run| run.finished_at.clone()),
                    result_count: latest_run.as_ref().and_then(|run| run.result_count),
                    error_class: latest_run.and_then(|run| run.error_class),
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

fn sanitize_query(query: &str) -> String {
    let query = normalize_query(query);
    if contains_sensitive_value(&query) {
        "[redacted]".to_string()
    } else {
        query
    }
}

fn lock_search_run_store() -> Result<MutexGuard<'static, ()>, String> {
    SEARCH_RUN_STORE_LOCK
        .lock()
        .map_err(|error| format!("search run store lock poisoned: {error}"))
}

fn timestamp_key(value: &str) -> u128 {
    value.parse().unwrap_or_default()
}

fn validate_timestamps(started_at: &str, finished_at: &str) -> Result<(), String> {
    let started = started_at
        .parse::<u128>()
        .map_err(|_| "search start timestamp must be Unix epoch milliseconds")?;
    let finished = finished_at
        .parse::<u128>()
        .map_err(|_| "search finish timestamp must be Unix epoch milliseconds")?;
    if finished < started {
        return Err("search finish timestamp precedes start timestamp".into());
    }
    Ok(())
}

fn project_arguments(arguments: &Value) -> Value {
    let mut budget = ProjectionBudget {
        remaining: MAX_ARGUMENT_BYTES,
    };
    project_argument_value(arguments, None, 0, &mut budget)
}

struct ProjectionBudget {
    remaining: usize,
}

impl ProjectionBudget {
    fn consume(&mut self, amount: usize) -> bool {
        if amount > self.remaining {
            false
        } else {
            self.remaining -= amount;
            true
        }
    }
}

fn project_argument_value(
    value: &Value,
    key: Option<&str>,
    depth: usize,
    budget: &mut ProjectionBudget,
) -> Value {
    if key.is_some_and(is_sensitive_argument_key) {
        return marker_value(REDACTED, budget);
    }
    if depth > MAX_ARGUMENT_DEPTH {
        return marker_value(TRUNCATED, budget);
    }
    if budget.remaining == 0 {
        return Value::String(TRUNCATED.to_string());
    }
    match value {
        Value::Null => {
            if budget.consume(4) {
                Value::Null
            } else {
                marker_value(TRUNCATED, budget)
            }
        }
        Value::Bool(value) => {
            if budget.consume(if *value { 4 } else { 5 }) {
                Value::Bool(*value)
            } else {
                marker_value(TRUNCATED, budget)
            }
        }
        Value::Number(value) => {
            if budget.consume(value.to_string().len()) {
                Value::Number(value.clone())
            } else {
                marker_value(TRUNCATED, budget)
            }
        }
        Value::String(value) => {
            let value = sanitize_argument_text(value);
            if budget.consume(encoded_string_len(&value)) {
                Value::String(value)
            } else {
                marker_value(TRUNCATED, budget)
            }
        }
        Value::Array(values) => {
            if !budget.consume(2) {
                return marker_value(TRUNCATED, budget);
            }
            let mut projected = Vec::new();
            for value in values {
                if budget.remaining == 0 {
                    projected.push(Value::String(TRUNCATED.to_string()));
                    break;
                }
                if !projected.is_empty() {
                    budget.consume(1);
                }
                projected.push(project_argument_value(value, None, depth + 1, budget));
            }
            Value::Array(projected)
        }
        Value::Object(object) => {
            if !budget.consume(2) {
                return marker_value(TRUNCATED, budget);
            }
            let field_selects_sensitive_value = object.iter().any(|(key, value)| {
                matches!(
                    normalize_argument_key(key).as_str(),
                    "field" | "name" | "key" | "type"
                ) && value.as_str().is_some_and(is_sensitive_argument_key)
            });
            let mut projected = serde_json::Map::new();
            for (key, value) in object {
                if budget.remaining == 0 {
                    projected.insert("_truncated".to_string(), Value::Bool(true));
                    break;
                }
                if !projected.is_empty() {
                    budget.consume(1);
                }
                if !budget.consume(encoded_string_len(key) + 1) {
                    projected.insert("_truncated".to_string(), Value::Bool(true));
                    break;
                }
                let sensitive_value =
                    field_selects_sensitive_value && normalize_argument_key(key) == "value";
                let value = if sensitive_value {
                    marker_value(REDACTED, budget)
                } else {
                    project_argument_value(value, Some(key), depth + 1, budget)
                };
                projected.insert(key.clone(), value);
            }
            Value::Object(projected)
        }
    }
}

fn marker_value(marker: &str, budget: &mut ProjectionBudget) -> Value {
    if !budget.consume(encoded_string_len(marker)) {
        budget.remaining = 0;
    }
    Value::String(marker.to_string())
}

fn encoded_string_len(value: &str) -> usize {
    serde_json::to_string(value)
        .map(|encoded| encoded.len())
        .unwrap_or(MAX_ARGUMENT_BYTES.saturating_add(1))
}

fn sanitize_argument_text(value: &str) -> String {
    if contains_sensitive_value(value) {
        REDACTED.to_string()
    } else {
        value.to_string()
    }
}

fn normalize_argument_key(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_sensitive_argument_key(key: &str) -> bool {
    let key = normalize_argument_key(key);
    let credential_marker = [
        "credential",
        "password",
        "passwd",
        "apikey",
        "cookie",
        "authorization",
        "secret",
        "privatekey",
        "accesskey",
        "profile",
        "browserdata",
        "browserstorage",
        "localstorage",
        "sessionstorage",
    ]
    .iter()
    .any(|marker| key.contains(marker));
    let token_key = key == "token"
        || key.ends_with("token")
        || ["tokenvalue", "tokenkey"]
            .iter()
            .any(|marker| key.contains(marker));
    let environment_key = key == "env"
        || key == "environment"
        || key.contains("environmentvariable")
        || key.contains("envvar");
    let authentication_key =
        key == "auth" || key.starts_with("authentication") || key.contains("authheader");
    let session_key = [
        "sessionid",
        "sessionidentifier",
        "sessiontoken",
        "sessionkey",
        "sessioncookie",
        "sessionsecret",
    ]
    .iter()
    .any(|marker| key.contains(marker));
    credential_marker || token_key || environment_key || authentication_key || session_key
}

fn contains_sensitive_value(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let trimmed = value.trim();
    let lower_trimmed = lower.trim();
    let markers = [
        "api_key",
        "api-key",
        "apikey",
        "access_token",
        "refresh_token",
        "token=",
        "password",
        "passwd",
        "cookie=",
        "session=",
        "bearer ",
        "credential",
        "browser_profile",
        "profile_path",
    ];
    let bytes = value.as_bytes();
    let windows_absolute =
        bytes.len() >= 3 && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/');
    let cookie_like = lower.contains('=')
        && lower.contains(';')
        && ["path=/", "httponly", "samesite=", "; secure"]
            .iter()
            .any(|marker| lower.contains(marker));
    let auth_header = lower_trimmed.starts_with("basic ")
        || lower_trimmed.starts_with("bearer ")
        || lower_trimmed.starts_with("digest ");
    let private_key = [
        "-----begin private key-----",
        "-----begin rsa private key-----",
        "-----begin ec private key-----",
        "-----begin openssh private key-----",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    markers.iter().any(|marker| lower.contains(marker))
        || windows_absolute
        || cookie_like
        || auth_header
        || private_key
        || looks_like_aws_access_key(trimmed)
        || looks_like_aws_secret_key(trimmed)
        || looks_like_jwt(trimmed)
        || looks_like_prefixed_token(trimmed)
        || value.starts_with("\\\\")
        || value.starts_with('/')
}

fn looks_like_aws_access_key(value: &str) -> bool {
    value.len() == 20
        && ["AKIA", "ASIA", "AIDA", "AROA"]
            .iter()
            .any(|prefix| value.starts_with(prefix))
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
}

fn looks_like_aws_secret_key(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
        && value.bytes().any(|byte| byte.is_ascii_uppercase())
        && value.bytes().any(|byte| byte.is_ascii_lowercase())
        && value.bytes().any(|byte| byte.is_ascii_digit())
        && value.bytes().any(|byte| matches!(byte, b'+' | b'/'))
}

fn looks_like_jwt(value: &str) -> bool {
    let segments: Vec<&str> = value.split('.').collect();
    value.len() >= 32
        && segments.len() == 3
        && segments.iter().all(|segment| {
            segment.len() >= 4
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'='))
        })
}

fn looks_like_prefixed_token(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    value.len() >= 20
        && ["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_", "sk-"]
            .iter()
            .any(|prefix| lower.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn temp_workspace(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "galen-search-run-test-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn success(provider_id: &str, started_at: &str, finished_at: &str) -> SearchRun {
        SearchRun::succeeded(
            "task-1",
            provider_id,
            "search",
            "stroke",
            serde_json::json!({"query": "stroke", "limit": 10}),
            started_at,
            finished_at,
            0,
            HASH_A,
        )
        .unwrap()
    }

    fn failure(provider_id: &str, started_at: &str, finished_at: &str) -> SearchRun {
        SearchRun::failed(
            "task-1",
            provider_id,
            "search",
            "stroke",
            serde_json::json!({"query": "stroke"}),
            started_at,
            finished_at,
            SearchErrorClass::Timeout,
            HASH_B,
        )
        .unwrap()
    }

    #[test]
    fn append_and_load_preserves_zero_result_success() {
        // Removing result_count or serializing zero as absent must fail this test.
        let root = temp_workspace("zero-result");
        let run = SearchRun::succeeded(
            "task-1",
            "pubmed",
            "search_pubmed",
            "stroke",
            serde_json::json!({"query": "stroke"}),
            "100",
            "250",
            0,
            HASH_A,
        )
        .unwrap();

        append_search_run(&root, &run).unwrap();
        let loaded = load_search_runs(&root, "task-1").unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, run.id);
        assert_eq!(loaded[0].status, SearchRunStatus::Succeeded);
        assert_eq!(loaded[0].result_count, Some(0));
        assert_eq!(loaded[0].started_at, "100");
        assert_eq!(loaded[0].finished_at, "250");
        assert_eq!(loaded[0].raw_result_hash.as_str(), HASH_A);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn append_rejects_task_ids_that_escape_the_task_store() {
        // Removing task-ID validation could write a ledger outside .galen/tasks.
        let root = temp_workspace("task-id");
        let mut run = SearchRun::succeeded(
            "task-1",
            "pubmed",
            "search_pubmed",
            "stroke",
            Value::Null,
            "100",
            "200",
            1,
            HASH_A,
        )
        .unwrap();
        run.task_id = "../escape".to_string();

        assert!(append_search_run(&root, &run).is_err());
        assert!(!root.join(".galen/escape/search-runs.jsonl").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn coverage_uses_latest_attempt_without_erasing_prior_search_history() {
        // Returning Searched whenever any past success exists would hide a later failure.
        let providers = vec![ProviderDescriptor::configured("pubmed", true, true)];
        let success = success("pubmed", "10", "100");
        let later_failure = failure("pubmed", "110", "200");

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
        let searched = success("searched", "10", "100");
        let failed = failure("failed", "110", "200");

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
    fn ledger_projects_arguments_and_classifies_errors_without_persisting_secrets() {
        // Persisting arbitrary arguments or raw errors would expose credentials and local paths.
        let root = temp_workspace("secret-projection");
        let raw_error = "auth failed api_key=top-secret C:\\Users\\alice\\cnki-profile";
        let run = SearchRun::failed(
            "task-1",
            "cnki",
            "search_cnki",
            "stroke rehabilitation",
            serde_json::json!({
                "query": "stroke rehabilitation",
                "limit": 20,
                "api_key": "top-secret",
                "cookie": "session-secret",
                "env": {"TOKEN": "env-secret"},
                "browser_profile_path": "C:\\Users\\alice\\cnki-profile"
            }),
            "100",
            "200",
            SearchErrorClass::classify(raw_error),
            HASH_A,
        )
        .unwrap();

        append_search_run(&root, &run).unwrap();
        let ledger = std::fs::read_to_string(search_runs_path(&root, "task-1")).unwrap();

        assert!(ledger.contains("stroke rehabilitation"));
        assert!(ledger.contains("authentication"));
        assert!(ledger.contains("\"limit\":20"));
        for secret in [
            "top-secret",
            "session-secret",
            "env-secret",
            "cnki-profile",
            "auth failed",
        ] {
            assert!(!ledger.contains(secret), "ledger leaked {secret}");
        }
        assert!(ledger.contains("\"api_key\":\"[redacted]\""));
        assert!(ledger.contains("\"cookie\":\"[redacted]\""));
        assert!(ledger.contains("\"env\":\"[redacted]\""));
        assert!(ledger.contains("\"browser_profile_path\":\"[redacted]\""));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn recursive_projection_preserves_unknown_legitimate_provider_arguments() {
        // Returning only known keys loses provider-specific retrieval provenance.
        let arguments = serde_json::json!({
            "facets": ["year", "publicationType"],
            "providerOptions": {
                "includePreprints": true,
                "minimumCitationCount": 5,
                "openAccess": null,
                "filters": [
                    {"field": "journal", "operator": "equals", "value": "Stroke"},
                    {"field": "year", "operator": "gte", "value": 2020}
                ]
            },
            "experimentalRanking": "citation_velocity"
        });
        let run = SearchRun::succeeded(
            "task-1",
            "semantic-scholar",
            "search_papers",
            "stroke",
            arguments.clone(),
            "100",
            "200",
            3,
            HASH_A,
        )
        .unwrap();

        assert_eq!(run.arguments(), &arguments);
    }

    #[test]
    fn recursive_projection_redacts_nested_sensitive_values_without_flattening() {
        // Nested provider options must retain shape while secrets and profile paths do not.
        let run = SearchRun::succeeded(
            "task-1",
            "crossref",
            "search_works",
            "stroke",
            serde_json::json!({
                "providerOptions": {
                    "apiKey": "key-secret",
                    "headers": {
                        "Authorization": "Bearer auth-secret",
                        "Accept": "application/json",
                        "opaqueValue": "sid=abc123; Path=/; HttpOnly"
                    },
                    "filters": [
                        {"field": "journal", "value": "Stroke"},
                        {"field": "profile", "value": "browser-secret"}
                    ],
                    "cacheLocation": "C:\\Users\\alice\\provider-profile"
                },
                "environment_variables": {"CROSSREF_TOKEN": "env-secret"},
                "page": 2
            }),
            "100",
            "200",
            3,
            HASH_A,
        )
        .unwrap();

        assert_eq!(
            run.arguments(),
            &serde_json::json!({
                "providerOptions": {
                    "apiKey": "[redacted]",
                    "headers": {
                        "Authorization": "[redacted]",
                        "Accept": "application/json",
                        "opaqueValue": "[redacted]"
                    },
                    "filters": [
                        {"field": "journal", "value": "Stroke"},
                        {"field": "profile", "value": "[redacted]"}
                    ],
                    "cacheLocation": "[redacted]"
                },
                "environment_variables": "[redacted]",
                "page": 2
            })
        );
    }

    #[test]
    fn normalized_sensitive_key_families_redact_their_complete_values() {
        // Missing normalized aliases can persist credentials hidden in nested containers.
        let run = SearchRun::succeeded(
            "task-1",
            "provider",
            "search",
            "stroke",
            serde_json::json!({
                "environment": {"HOME": "C:\\Users\\alice", "SAFE": "no"},
                "environmentVariables": {"API_TOKEN": "env-secret"},
                "auth-header": "Basic dXNlcjpwYXNz",
                "authentication": {"scheme": "basic", "value": "auth-secret"},
                "private_key": "private-secret",
                "accessKey": "access-secret",
                "secretKey": "secret-secret",
                "session-id": "session-123",
                "sessionSecret": "session-secret",
                "providerSpecific": {
                    "sessionMode": "stateless",
                    "tokenizationStrategy": "bm25-window",
                    "ranking": {"model": "citation-velocity", "weight": 0.75}
                }
            }),
            "100",
            "200",
            1,
            HASH_A,
        )
        .unwrap();

        assert_eq!(
            run.arguments(),
            &serde_json::json!({
                "environment": "[redacted]",
                "environmentVariables": "[redacted]",
                "auth-header": "[redacted]",
                "authentication": "[redacted]",
                "private_key": "[redacted]",
                "accessKey": "[redacted]",
                "secretKey": "[redacted]",
                "session-id": "[redacted]",
                "sessionSecret": "[redacted]",
                "providerSpecific": {
                    "sessionMode": "stateless",
                    "tokenizationStrategy": "bm25-window",
                    "ranking": {"model": "citation-velocity", "weight": 0.75}
                }
            })
        );
    }

    #[test]
    fn credential_shaped_strings_are_redacted_under_innocuous_keys() {
        // Secret values must not rely solely on a revealing field name.
        let run = SearchRun::succeeded(
            "task-1",
            "provider",
            "search",
            "stroke",
            serde_json::json!({
                "samples": [
                    "Basic dXNlcjpwYXNz",
                    "Bearer eyJhbGciOiJIUzI1NiJ9.payload.signature",
                    "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BA\n-----END PRIVATE KEY-----",
                    "AKIAIOSFODNN7EXAMPLE",
                    "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
                    "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
                    "ghp_1234567890abcdefghijklmnopqrstuvwxyz",
                    "sk-proj-abcdefghijklmnopqrstuvwxyz0123456789"
                ],
                "ordinary": {
                    "identifier": "S2-CORPUS-12345",
                    "algorithm": "BM25",
                    "filters": ["year:2024", "open-access"]
                }
            }),
            "100",
            "200",
            1,
            HASH_A,
        )
        .unwrap();

        assert_eq!(
            run.arguments(),
            &serde_json::json!({
                "samples": [
                    "[redacted]", "[redacted]", "[redacted]", "[redacted]",
                    "[redacted]", "[redacted]", "[redacted]", "[redacted]"
                ],
                "ordinary": {
                    "identifier": "S2-CORPUS-12345",
                    "algorithm": "BM25",
                    "filters": ["year:2024", "open-access"]
                }
            })
        );
    }

    #[test]
    fn recursive_projection_marks_depth_and_size_bounds_explicitly() {
        // Bounds must be visible in provenance instead of silently dropping the argument object.
        let mut deep = serde_json::json!({"leaf": "kept-until-bound"});
        for level in 0..(MAX_ARGUMENT_DEPTH + 2) {
            let mut object = serde_json::Map::new();
            object.insert(format!("level{level}"), deep);
            deep = Value::Object(object);
        }
        let run = SearchRun::succeeded(
            "task-1",
            "provider",
            "search",
            "stroke",
            serde_json::json!({
                "normal": {"nested": [true, 7, null, "kept"]},
                "deep": deep,
                "escaped": "\"".repeat(MAX_ARGUMENT_BYTES * 3 / 4),
                "oversized": "x".repeat(MAX_ARGUMENT_BYTES * 2)
            }),
            "100",
            "200",
            0,
            HASH_A,
        )
        .unwrap();
        let serialized = serde_json::to_string(run.arguments()).unwrap();

        assert!(serialized.contains("\"normal\":{\"nested\":[true,7,null,\"kept\"]}"));
        assert!(serialized.contains("[truncated]"));
        assert!(serialized.len() <= MAX_ARGUMENT_BYTES + MAX_ARGUMENT_OVERHEAD_BYTES);
    }

    #[test]
    fn coverage_serialization_exposes_summary_fields_not_full_runs() {
        // Embedding SearchRun would leak task IDs, tool names, arguments, IDs, and hashes.
        let providers = vec![ProviderDescriptor::configured("pubmed", true, true)];
        let run = success("pubmed", "100", "250");

        let json = serde_json::to_string(&derive_coverage(&providers, &[run])).unwrap();

        assert!(json.contains("latestQuery"));
        assert!(json.contains("latestFinishedAt"));
        assert!(json.contains("resultCount"));
        for forbidden in [
            "arguments",
            "taskId",
            "toolName",
            "rawResultHash",
            "latestRun",
        ] {
            assert!(!json.contains(forbidden), "coverage leaked {forbidden}");
        }
    }

    #[test]
    fn ordinary_fields_still_redact_secret_values_and_profile_paths() {
        // An ordinary key must not become a tunnel for credentials or local profile paths.
        let root = temp_workspace("secret-values");
        let run = SearchRun::succeeded(
            "task-1",
            "cnki",
            "search_cnki",
            "stroke token=top-secret",
            serde_json::json!({
                "query": "stroke token=top-secret",
                "title": "C:\\Users\\alice\\cnki-profile",
                "author": "cookie=session-secret",
                "limit": 10
            }),
            "100",
            "200",
            0,
            HASH_A,
        )
        .unwrap();

        append_search_run(&root, &run).unwrap();
        let ledger = std::fs::read_to_string(search_runs_path(&root, "task-1")).unwrap();

        assert!(ledger.contains("[redacted]"));
        assert!(ledger.contains("\"limit\":10"));
        for secret in [
            "top-secret",
            "session-secret",
            "cnki-profile",
            "C:\\\\Users",
        ] {
            assert!(!ledger.contains(secret), "ledger leaked {secret}");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn append_and_coverage_resanitize_mutated_query_text() {
        // Public record fields must not bypass sanitization at persistence or DTO boundaries.
        let root = temp_workspace("mutated-query");
        let mut run = success("pubmed", "100", "200");
        run.query = "stroke bearer top-secret".to_string();

        append_search_run(&root, &run).unwrap();
        let ledger = std::fs::read_to_string(search_runs_path(&root, "task-1")).unwrap();
        let coverage = serde_json::to_string(&derive_coverage(
            &[ProviderDescriptor::configured("pubmed", true, true)],
            &[run],
        ))
        .unwrap();

        assert!(!ledger.contains("top-secret"));
        assert!(!coverage.contains("top-secret"));
        assert!(ledger.contains("[redacted]"));
        assert!(coverage.contains("[redacted]"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn hash_and_timestamp_boundaries_reject_invalid_provenance() {
        // Accepting malformed digests or host-generated timestamps destroys provenance integrity.
        assert!(SearchRun::succeeded(
            "task-1",
            "pubmed",
            "search",
            "stroke",
            Value::Null,
            "100",
            "200",
            1,
            "abc",
        )
        .is_err());
        assert!(SearchRun::succeeded(
            "task-1",
            "pubmed",
            "search",
            "stroke",
            Value::Null,
            "200",
            "100",
            1,
            HASH_A,
        )
        .is_err());

        let mut forged = success("pubmed", "100", "200");
        forged.raw_result_hash = Sha256Hash("abc".to_string());
        assert!(append_search_run(&temp_workspace("forged-hash"), &forged).is_err());
    }

    #[test]
    fn concurrent_appenders_preserve_every_complete_json_line() {
        // Removing the append lock may interleave JSON and newline writes.
        let root = Arc::new(temp_workspace("concurrent"));
        let barrier = Arc::new(Barrier::new(32));
        let handles: Vec<_> = (0..32)
            .map(|index| {
                let root = Arc::clone(&root);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    let run = SearchRun::succeeded(
                        "task-1",
                        "pubmed",
                        "search_pubmed",
                        format!("stroke {index}"),
                        serde_json::json!({"query": format!("stroke {index}"), "limit": 10}),
                        "100",
                        "200",
                        index,
                        HASH_A,
                    )
                    .unwrap();
                    barrier.wait();
                    append_search_run(&root, &run).unwrap();
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }

        let text = std::fs::read_to_string(search_runs_path(&root, "task-1")).unwrap();
        assert!(text.ends_with('\n'));
        assert_eq!(text.lines().count(), 32);
        assert!(text
            .lines()
            .all(|line| serde_json::from_str::<SearchRun>(line).is_ok()));
        assert_eq!(load_search_runs(&root, "task-1").unwrap().len(), 32);
        let _ = std::fs::remove_dir_all(root.as_path());
    }

    #[test]
    fn run_ids_are_uuid_v4_and_unique() {
        // Timestamp/process/counter IDs can collide across application processes.
        let first = success("pubmed", "100", "200");
        let second = success("pubmed", "100", "200");

        assert_ne!(first.id, second.id);
        assert_eq!(
            uuid::Uuid::parse_str(&first.id).unwrap().get_version_num(),
            4
        );
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
                    latest_query: None,
                    latest_finished_at: None,
                    result_count: None,
                    error_class: None,
                },
            )])
        );
    }
}
