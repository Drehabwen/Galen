use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{Emitter, State, Window};

use crate::backend::{self, ChatBackend, ChatEvent, ModelConfig};
use crate::modes::ChatMode;
use crate::research_task::{ResearchNode, ResearchTask};
use crate::runtime_manager::{self, McpServerStatus, RuntimeStatus};
use crate::workspace::WorkspaceConfig;
use medical_core::clinical::ClinicalCaseInput;

pub mod rehab;
pub mod memory;
pub mod models;
pub mod research;
pub mod workspace;

pub use memory::*;
pub use models::*;
pub use research::*;

pub struct AppState {
    pub backend: Mutex<ChatBackend>,
    pub ws_config: Mutex<WorkspaceConfig>,
    pub mode: Mutex<ChatMode>,
    pub persona: Mutex<crate::personas::Persona>,
}

/// A deliberately narrow, presentation-ready projection of one literature
/// provider. Full SearchRun records and MCP configuration never cross the
/// WebView boundary.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiteratureProviderCoverage {
    pub provider_id: String,
    pub display_name: String,
    pub state: crate::search_run::CoverageState,
    pub has_successful_history: bool,
    pub latest_query: Option<String>,
    pub latest_finished_at: Option<String>,
    pub result_count: Option<usize>,
    pub error_class: Option<crate::search_run::SearchErrorClass>,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiteratureCoverageResponse {
    pub task_id: Option<String>,
    pub providers: Vec<LiteratureProviderCoverage>,
    pub has_limitations: bool,
    pub limitation: Option<String>,
}

/// Internal provider input that preserves the distinction between a missing
/// configuration (catalog defaults may be used) and an unreadable one.
#[derive(Debug, Clone)]
pub(crate) struct LiteratureProviderSource {
    pub(crate) providers: Vec<crate::search_run::ProviderDescriptor>,
    pub(crate) configuration_unavailable: bool,
}

const LITERATURE_PROVIDER_IDS: &[&str] = &["pubmed", "crossref", "semantic-scholar", "cnki"];
const MAX_COVERAGE_QUERY_CHARS: usize = 240;

fn literature_provider_name(provider_id: &str) -> String {
    match provider_id {
        "pubmed" => "PubMed".to_string(),
        "crossref" => "Crossref".to_string(),
        "semantic-scholar" => "Semantic Scholar".to_string(),
        "cnki" => "CNKI".to_string(),
        other => other.to_string(),
    }
}

fn bounded_query_summary(query: Option<&str>) -> Option<String> {
    query.map(|query| {
        if query.chars().count() <= MAX_COVERAGE_QUERY_CHARS {
            query.to_string()
        } else {
            let mut summary: String = query.chars().take(MAX_COVERAGE_QUERY_CHARS).collect();
            summary.push('…');
            summary
        }
    })
}

pub(crate) fn read_literature_mcp_config(
    path: &std::path::Path,
) -> Result<Option<crate::mcp_client::McpConfig>, ()> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map(Some).map_err(|_| ()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(()),
    }
}

pub(crate) fn literature_provider_source_from_config(
    config: Result<Option<crate::mcp_client::McpConfig>, ()>,
    connected_server_names: &[String],
) -> LiteratureProviderSource {
    let config = match config {
        Ok(config) => {
            crate::mcp_client::McpConfig::with_builtin_catalog(config.unwrap_or_default())
        }
        Err(()) => {
            return LiteratureProviderSource {
                providers: LITERATURE_PROVIDER_IDS
                    .iter()
                    .map(|provider_id| {
                        if *provider_id == "pubmed" {
                            crate::search_run::ProviderDescriptor::configured(
                                *provider_id,
                                true,
                                true,
                            )
                        } else {
                            // The provider is part of the known catalog, but
                            // its user configuration is unavailable. Do not
                            // treat it as a healthy catalog default.
                            crate::search_run::ProviderDescriptor::configured(
                                *provider_id,
                                true,
                                false,
                            )
                        }
                    })
                    .collect(),
                configuration_unavailable: true,
            };
        }
    };
    let providers = LITERATURE_PROVIDER_IDS
        .iter()
        .map(|provider_id| {
            if *provider_id == "pubmed" {
                return crate::search_run::ProviderDescriptor::configured(*provider_id, true, true);
            }
            match config.mcp_servers.get(*provider_id) {
                Some(server) => crate::search_run::ProviderDescriptor::configured(
                    *provider_id,
                    server.enabled,
                    server.enabled
                        && connected_server_names
                            .iter()
                            .any(|name| name == provider_id),
                ),
                None => crate::search_run::ProviderDescriptor::not_configured(*provider_id),
            }
        })
        .collect();
    LiteratureProviderSource {
        providers,
        configuration_unavailable: false,
    }
}

pub(crate) fn configured_literature_providers() -> LiteratureProviderSource {
    let config = dirs::config_dir()
        .map(|dir| read_literature_mcp_config(&dir.join("galen").join("mcp_servers.json")))
        .unwrap_or(Err(()));
    let connected_server_names = crate::chat_loop::cached_connected_mcp_server_names();
    literature_provider_source_from_config(config, &connected_server_names)
}

pub(crate) fn literature_coverage_from_provider_source(
    task_id: Option<&str>,
    provider_source: &LiteratureProviderSource,
    runs: &[crate::search_run::SearchRun],
) -> LiteratureCoverageResponse {
    let mut response = literature_coverage_from_runs(task_id, &provider_source.providers, runs);
    if provider_source.configuration_unavailable {
        response.has_limitations = true;
        response.limitation = Some(
            "Literature provider configuration is unavailable. Final claims must say \"based on searched providers\" and must not imply comprehensive coverage."
                .to_string(),
        );
    }
    response
}

pub(crate) fn literature_coverage_for_workspace_from_provider_source(
    workspace_root: &std::path::Path,
    provider_source: &LiteratureProviderSource,
) -> Result<LiteratureCoverageResponse, String> {
    let Some(task) = crate::research_task::load_active_task(workspace_root)? else {
        return Ok(literature_coverage_from_provider_source(
            None,
            provider_source,
            &[],
        ));
    };
    let runs = crate::search_run::load_search_runs(workspace_root, &task.task_id)?;
    Ok(literature_coverage_from_provider_source(
        Some(&task.task_id),
        provider_source,
        &runs,
    ))
}

pub(crate) fn literature_coverage_from_runs(
    task_id: Option<&str>,
    providers: &[crate::search_run::ProviderDescriptor],
    runs: &[crate::search_run::SearchRun],
) -> LiteratureCoverageResponse {
    let observed_providers: Vec<_> = providers
        .iter()
        .cloned()
        .map(|mut provider| {
            if provider.configured
                && provider.enabled
                && runs
                    .iter()
                    .any(|run| run.provider_id == provider.provider_id)
            {
                // A durable terminal attempt is enough to derive searched or
                // failed state without claiming a current live connection.
                provider.connected = true;
            }
            provider
        })
        .collect();
    let coverage = crate::search_run::derive_coverage(&observed_providers, runs);
    let providers: Vec<_> = observed_providers
        .iter()
        .filter_map(|descriptor| coverage.get(&descriptor.provider_id))
        .map(|provider| LiteratureProviderCoverage {
            provider_id: provider.provider_id.clone(),
            display_name: literature_provider_name(&provider.provider_id),
            state: provider.state.clone(),
            has_successful_history: provider.has_successful_history,
            latest_query: bounded_query_summary(provider.latest_query.as_deref()),
            latest_finished_at: provider.latest_finished_at.clone(),
            result_count: provider.result_count,
            error_class: provider.error_class.clone(),
        })
        .collect();
    let has_limitations = task_id.is_none()
        || providers.iter().any(|provider| {
            !matches!(
                provider.state,
                crate::search_run::CoverageState::Searched
                    | crate::search_run::CoverageState::NotConfigured
            )
        });
    LiteratureCoverageResponse {
        task_id: task_id.map(str::to_string),
        providers,
        has_limitations,
        limitation: has_limitations.then(|| {
            "One or more configured literature sources were not successfully searched. Final claims must say \"based on searched providers\" and must not imply comprehensive coverage."
                .to_string()
        }),
    }
}

/// Return coverage only for the host-selected workspace and its active task.
/// No WebView-supplied path or task identifier is accepted.
#[tauri::command]
pub fn get_literature_coverage(
    state: State<AppState>,
) -> Result<LiteratureCoverageResponse, String> {
    let workspace_root = {
        let backend = lock_mutex(&state.backend)?;
        backend.get_workspace_root()
    };
    let provider_source = configured_literature_providers();
    match workspace_root {
        Some(root) => {
            literature_coverage_for_workspace_from_provider_source(&root, &provider_source)
        }
        None => Ok(literature_coverage_from_provider_source(
            None,
            &provider_source,
            &[],
        )),
    }
}

/// Lock a std::sync::Mutex and map the poison error to a String.
pub(super) fn lock_mutex<T>(m: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, String> {
    m.lock().map_err(|e| format!("Internal state error: {e}"))
}

#[tauri::command]
pub fn get_models(state: State<AppState>) -> Result<Vec<ModelConfig>, String> {
    Ok(lock_mutex(&state.backend)?.all_models())
}

#[tauri::command]
pub fn get_mode(state: State<AppState>) -> Result<ChatMode, String> {
    Ok(*lock_mutex(&state.mode)?)
}

#[tauri::command]
pub fn set_mode(state: State<AppState>, mode: ChatMode) -> Result<ChatMode, String> {
    let mut guard = lock_mutex(&state.mode)?;
    *guard = mode;
    crate::modes::save_mode(mode);
    Ok(*guard)
}

#[tauri::command]
pub fn get_personas() -> Vec<crate::personas::Persona> {
    crate::personas::all_personas()
}

#[tauri::command]
pub fn get_persona(state: State<AppState>) -> Result<crate::personas::Persona, String> {
    Ok(lock_mutex(&state.persona)?.clone())
}

#[tauri::command]
pub fn get_modes() -> Vec<crate::modes::ModeMeta> {
    crate::modes::all_modes()
}

#[tauri::command]
pub fn set_persona(
    state: State<AppState>,
    persona_id: String,
) -> Result<crate::personas::Persona, String> {
    let persona = crate::personas::find_persona(&persona_id);
    let mut guard = lock_mutex(&state.persona)?;
    *guard = persona.clone();
    Ok(persona)
}

// Clinical reasoning command
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn analyze_clinical_case(
    case_text: String,
    age: Option<u8>,
    sex: Option<String>,
    context: Option<String>,
    output_format: Option<String>,
) -> Result<String, String> {
    // Translate user-facing error for empty input
    if case_text.trim().is_empty() {
        return Err("请输入症状或病例描述。".into());
    }
    medical_core::clinical::run(
        ClinicalCaseInput {
            case_text,
            age,
            sex: sex
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            context: context
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
        },
        output_format.as_deref().unwrap_or("markdown"),
    )
}

// ---------------------------------------------------------------------------
// Chat command (async, extracts data from mutex before spawning)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn send_message(
    state: State<'_, AppState>,
    window: Window,
    message: String,
    model_alias: String,
    history_json: String,
    mode: ChatMode,
    persona_id: String,
    tag: Option<String>, // Session tag for event isolation (empty = main chat)
    thinking_level: Option<String>,
) -> Result<(), String> {
    let thinking_level = thinking_level.unwrap_or_else(|| "low".to_string());
    // Phase 1: extract all needed data from locked state (before any .await)
    let (model_alias, model_id, medical, router, workspace_path, persona) = {
        let backend = lock_mutex(&state.backend)?;
        // The UI may send an empty/unknown alias before a model is selected;
        // normalize to the configured default so we never fall back to a
        // hardcoded provider (previously Anthropic).
        let model_alias = if backend.router.get_model(&model_alias).is_some() {
            model_alias.clone()
        } else {
            backend.router.default_alias().to_string()
        };
        let model_id = backend.resolve_model(&model_alias);
        let medical = backend.medical.clone();
        let router = backend.router.clone();
        let workspace_path = backend
            .workspace_root
            .lock()
            .map_err(|e| format!("{e}"))?
            .clone();
        let persona = crate::personas::find_persona(&persona_id);
        (
            model_alias,
            model_id,
            medical,
            router,
            workspace_path,
            persona,
        )
    };

    // Build tagged event names for session isolation
    let suffix = tag.as_ref().map(|t| format!(":{t}")).unwrap_or_default();

    // Phase 2: spawn chat in background, emitting events to the window
    let window_clone = window.clone();
    let err_tag = suffix.clone();
    let session_tag = tag.clone();
    tokio::spawn(async move {
        // The durable runtime session is authoritative. Frontend history is
        // imported only when creating a session for the first time.
        let fallback = backend::parse_history_json(&history_json);
        let input_messages = match workspace_path.as_deref() {
            Some(root) => match crate::chat_session::prepare_model_history(
                root,
                session_tag.as_deref(),
                &model_id,
                fallback,
            ) {
                Ok(history) => history,
                Err(error) => {
                    let ename = format!("chat-error{err_tag}");
                    let _ = window_clone.emit(&ename, &error);
                    return;
                }
            },
            None => fallback,
        };
        let final_text = Arc::new(Mutex::new(None::<String>));
        let captured_final = final_text.clone();
        let tool_traces = Arc::new(Mutex::new(Vec::<crate::backend::ToolTrace>::new()));
        let persisted_model = model_alias.clone();
        let persisted_user = message.clone();
        let started_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_default();
        let result = backend::run_chat(
            model_alias,
            model_id,
            message,
            input_messages,
            mode,
            persona,
            thinking_level,
            medical,
            router,
            Mutex::new(workspace_path.clone()),
            Some(tool_traces.clone()),
            {
                let suffix = suffix.clone();
                move |event| {
                    macro_rules! emit {
                        ($name:expr, $payload:expr) => {{
                            let ename = format!("{}{suffix}", $name);
                            if let Err(e) = window.emit(&ename, $payload) {
                                eprintln!("[galen] emit '{}' failed: {e}", ename);
                            }
                        }};
                    }
                    match &event {
                        ChatEvent::Delta(text) => emit!("chat-delta", text),
                        // Do not expose completion until the exchange has been
                        // durably appended below. Once the UI receives Done it
                        // unlocks the input, so emitting here allowed an
                        // immediate follow-up to race ahead of persistence and
                        // reach the model without the preceding turn.
                        ChatEvent::Done(_) => {}
                        ChatEvent::ThinkingDelta(text) => emit!("chat-thinking-delta", text),
                        ChatEvent::ThinkingDone(text) => emit!("chat-thinking-done", text),
                        ChatEvent::Error(e) => emit!("chat-error", e.as_str()),
                        ChatEvent::ToolProgress { turn, max_turns, tool, phase } => emit!(
                            "chat-tool-progress",
                            serde_json::json!({ "turn": turn, "maxTurns": max_turns, "tool": tool, "phase": phase })
                        ),
                        ChatEvent::SearchResults(papers) => emit!("search-results", papers),
                        ChatEvent::WorkspaceRoot(path) => emit!("workspace-root", path.as_str()),
                        ChatEvent::WorkspaceFileList(files) => emit!("workspace-file-list", files),
                        ChatEvent::WorkspaceFileContent { path, content } => {
                            emit!(
                                "workspace-file-content",
                                serde_json::json!({ "path": path, "content": content })
                            )
                        }
                        ChatEvent::ArtifactCreated(artifact) => {
                            emit!("artifact-created", artifact)
                        }
                        ChatEvent::ResearchTaskUpdated(task) => {
                            emit!("research-task-updated", task)
                        }
                    }
                    if let ChatEvent::Done(text) = &event {
                        if let Ok(mut captured) = captured_final.lock() {
                            *captured = Some(text.clone());
                        }
                    }
                }
            },
        )
        .await;

        let metrics = match result {
            Ok(metrics) => metrics,
            Err(error) => {
                let ename = format!("chat-error{err_tag}");
                let _ = window_clone.emit(&ename, &error);
                return;
            }
        };

        let completed = final_text.lock().ok().and_then(|value| value.clone());
        if let Some(assistant_text) = completed {
            if let Some(root) = workspace_path.as_deref() {
                let completed_traces = tool_traces
                    .lock()
                    .map(|value| value.clone())
                    .unwrap_or_default();
                if let Err(error) = crate::chat_session::append_exchange(
                    root,
                    session_tag.as_deref(),
                    &persisted_model,
                    &persisted_user,
                    &assistant_text,
                    &completed_traces,
                    started_at_ms,
                    &metrics,
                ) {
                    let ename = format!("chat-error{err_tag}");
                    let _ = window_clone.emit(&ename, &error);
                    return;
                }
            }

            // Completion is the UI's hand-off point. Emitting it only after
            // persistence guarantees that the very next turn can load this
            // exchange from the authoritative session.
            let metrics_name = format!("chat-run-metrics{suffix}");
            let _ = window_clone.emit(&metrics_name, &metrics);
            let done_name = format!("chat-done{suffix}");
            let _ = window_clone.emit(&done_name, &assistant_text);
        }
    });

    Ok(())
}

#[tauri::command]
pub fn get_chat_session(
    state: State<AppState>,
    tag: Option<String>,
) -> Result<Vec<crate::chat_session::ChatSessionMessage>, String> {
    let backend = lock_mutex(&state.backend)?;
    let Some(root) = backend.get_workspace_root() else {
        return Ok(Vec::new());
    };
    crate::chat_session::load_messages(&root, tag.as_deref())
}

#[tauri::command]
pub fn get_capabilities() -> Vec<crate::capability::CapabilityStatus> {
    crate::capability::official_statuses(&crate::capability::CapabilityConfig::load())
}

#[tauri::command]
pub fn get_conversation_decisions(
    state: State<AppState>,
) -> Result<Vec<crate::conversation_memory::DecisionRecord>, String> {
    let backend = lock_mutex(&state.backend)?;
    let Some(root) = backend.get_workspace_root() else {
        return Ok(Vec::new());
    };
    crate::conversation_memory::load_recent_decisions(&root, Some(24))
}

#[tauri::command]
pub fn revise_conversation_decision(
    state: State<AppState>,
    id: String,
    statement: String,
) -> Result<crate::conversation_memory::DecisionRecord, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend
        .get_workspace_root()
        .ok_or_else(|| "请先选择工作区".to_string())?;
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("系统时间错误: {error}"))?
        .as_millis() as u64;
    crate::conversation_memory::revise_decision(&root, &id, &statement, timestamp_ms)
}

#[tauri::command]
pub fn dismiss_conversation_decision(state: State<AppState>, id: String) -> Result<(), String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend
        .get_workspace_root()
        .ok_or_else(|| "请先选择工作区".to_string())?;
    crate::conversation_memory::dismiss_decision(&root, &id)
}

#[tauri::command]
pub fn clear_chat_session(state: State<AppState>, tag: Option<String>) -> Result<(), String> {
    let backend = lock_mutex(&state.backend)?;
    let Some(root) = backend.get_workspace_root() else {
        return Ok(());
    };
    crate::chat_session::archive_session(&root, tag.as_deref())
}

/// Archive the current research context and leave the workspace ready for a
/// genuinely new question. Nothing is deleted: the old session, memory,
/// decisions and active-task pointer move under `.galen/topic-archives/`.
#[tauri::command]
pub fn start_new_research_topic(state: State<AppState>) -> Result<(), String> {
    let backend = lock_mutex(&state.backend)?;
    let Some(root) = backend.get_workspace_root() else {
        return Ok(());
    };
    crate::chat_session::archive_session(&root, None)?;
    archive_topic_context(&root)
}

fn archive_topic_context(root: &std::path::Path) -> Result<(), String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("系统时间错误: {error}"))?
        .as_millis();
    let archive = root
        .join(".galen")
        .join("topic-archives")
        .join(stamp.to_string());
    std::fs::create_dir_all(&archive).map_err(|error| format!("创建课题归档目录失败: {error}"))?;
    for relative in [
        "GALEN.md",
        ".galen/conversation-decisions.jsonl",
        ".galen/active-task.json",
        "plan.json",
    ] {
        let source = root.join(relative);
        if !source.exists() {
            continue;
        }
        let target = archive.join(relative.replace(['/', '\\'], "_"));
        std::fs::rename(&source, &target)
            .map_err(|error| format!("归档旧课题上下文 {} 失败: {error}", source.display()))?;
    }
    std::fs::write(
        archive.join("README.txt"),
        "此目录保存由“新课题”操作归档的会话上下文；研究任务和产物仍保留在 .galen/tasks 与 output。\n",
    )
    .map_err(|error| format!("写入课题归档说明失败: {error}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Runtime environment status
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_runtime_status() -> RuntimeStatus {
    runtime_manager::detect_all()
}

// ---------------------------------------------------------------------------
// MCP server status
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_mcp_status() -> Vec<McpServerStatus> {
    runtime_manager::detect_mcp_servers().await
}

// ---------------------------------------------------------------------------
// API key management
// ---------------------------------------------------------------------------

/// Minimal default models.toml template — user only provides API key.
/// Provider defaults to DeepSeek (the most common choice for Chinese users),
/// but the file can be hand-edited for any OpenAI-compatible provider.
const DEFAULT_MODEL_TEMPLATE: &str = r#"[router]
default = "deepseek-v4-flash"
fast = "deepseek-v4-flash"
analysis = "deepseek-v4-pro"

[models.deepseek-v4-pro]
provider = "openai_compat"
api_key = "{api_key}"
model_id = "deepseek-v4-pro"
base_url = "https://api.deepseek.com/v1"
description = "DeepSeek V4 Pro（深度研究）"
max_tokens = 32768

[models.deepseek-v4-flash]
provider = "openai_compat"
api_key = "{api_key}"
model_id = "deepseek-v4-flash"
base_url = "https://api.deepseek.com/v1"
description = "DeepSeek V4 Flash（默认，快速）"
max_tokens = 32768
"#;

#[tauri::command]
pub fn save_api_key(
    state: State<AppState>,
    api_key: String,
    default_model: Option<String>,
) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let galen_dir = home.join(".galen");
    std::fs::create_dir_all(&galen_dir).map_err(|e| format!("{e}"))?;

    let models_path = galen_dir.join("models.toml");
    let router = persist_models_config(&models_path, &api_key, default_model.as_deref())?;

    // The backend is created before onboarding starts. Refresh its in-memory
    // router immediately so status checks, connection tests and chat can use
    // the newly saved model without requiring an application restart.
    lock_mutex(&state.backend)?.router = router;
    Ok(())
}

fn persist_models_config(
    models_path: &std::path::Path,
    api_key: &str,
    default_model: Option<&str>,
) -> Result<model_router::ModelRouter, String> {
    let content = if models_path.exists() {
        let existing =
            std::fs::read_to_string(&models_path).map_err(|e| format!("读取配置失败: {e}"))?;
        match existing.parse::<toml::Value>() {
            Ok(mut value) => {
                // 更新所有 DeepSeek 相关模型（pro/flash/default/imported）的 api_key
                let mut updated_any = false;
                if let Some(models) = value.get_mut("models").and_then(|m| m.as_table_mut()) {
                    for (name, model) in models.iter_mut() {
                        let is_deepseek =
                            name.contains("deepseek") || name == "default" || name == "imported";
                        if !is_deepseek {
                            continue;
                        }
                        if let Some(table) = model.as_table_mut() {
                            table.insert(
                                "api_key".to_string(),
                                toml::Value::String(api_key.to_string()),
                            );
                            updated_any = true;
                        }
                    }
                }
                // 设置默认模型（Pro / Flash）
                if let Some(default) = default_model {
                    if let Some(router) = value.get_mut("router").and_then(|r| r.as_table_mut()) {
                        router.insert(
                            "default".to_string(),
                            toml::Value::String(default.to_string()),
                        );
                    }
                }
                if updated_any {
                    toml::to_string_pretty(&value).map_err(|e| format!("序列化配置失败: {e}"))?
                } else {
                    format!(
                        "{existing}\n\n[models.imported]\nprovider = \"openai_compat\"\napi_key = \"{api_key}\"\nmodel_id = \"deepseek-v4-pro\"\nbase_url = \"https://api.deepseek.com/v1\"\n"
                    )
                }
            }
            Err(_) => template_with_default(api_key, default_model),
        }
    } else {
        template_with_default(api_key, default_model)
    };

    // Validate the exact content before replacing the active configuration.
    // This also gives us the router instance that will be installed in memory.
    let validation_path = models_path.with_extension("toml.pending");
    std::fs::write(&validation_path, &content).map_err(|e| format!("写入临时配置失败: {e}"))?;
    let router = model_router::ModelRouter::load_from(&validation_path)
        .map_err(|e| format!("模型配置无效: {e}"));
    let _ = std::fs::remove_file(&validation_path);
    let router = router?;

    std::fs::write(models_path, content).map_err(|e| format!("写入失败: {e}"))?;
    Ok(router)
}

fn template_with_default(api_key: &str, default_model: Option<&str>) -> String {
    let mut content = DEFAULT_MODEL_TEMPLATE.replace("{api_key}", api_key);
    if let Some(default) = default_model {
        content = content.replace(
            "default = \"deepseek-v4-flash\"",
            &format!("default = \"{default}\""),
        );
    }
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_injects_key_and_default() {
        let t = template_with_default("sk-test-123", Some("deepseek-v4-flash"));
        assert!(t.contains("default = \"deepseek-v4-flash\""));
        assert!(t.contains("api_key = \"sk-test-123\""));
        assert!(t.contains("[models.deepseek-v4-pro]"));
    }

    #[test]
    fn template_defaults_to_flash_and_routes_analysis_to_pro() {
        let t = template_with_default("sk-test", None);
        assert!(t.contains("default = \"deepseek-v4-flash\""));
        assert!(t.contains("analysis = \"deepseek-v4-pro\""));
    }

    #[test]
    fn persisted_config_is_immediately_loadable() {
        let dir = std::env::temp_dir().join(format!(
            "galen-model-config-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("models.toml");

        let router = persist_models_config(&path, "sk-probe", Some("deepseek-v4-flash")).unwrap();

        assert_eq!(router.default_alias(), "deepseek-v4-flash");
        assert_eq!(router.all_models().len(), 2);
        assert!(router
            .to_provider_config("deepseek-v4-pro")
            .and_then(|config| config.api_key().map(str::to_owned))
            .is_some());

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn citation_export_only_collects_explicit_pmids() {
        let evidence = vec![crate::evidence::Evidence {
            id: "ev-1".into(),
            node_id: "node-1".into(),
            node_title: "文献检索".into(),
            source: "research".into(),
            claim: "支持结论 [PMID: 12345678]，样本量为 987654。".into(),
            detail: Some("另见 PMID：23456789；重复 PMID: 12345678。".into()),
            confidence: "high".into(),
            created_at: "2026-09-07".into(),
        }];
        assert_eq!(
            cited_pmids(&evidence),
            vec!["12345678".to_string(), "23456789".to_string()]
        );
    }

    #[test]
    fn new_topic_archives_context_without_deleting_it() {
        let root = std::env::temp_dir().join(format!(
            "galen-topic-archive-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join(".galen")).unwrap();
        std::fs::write(root.join("GALEN.md"), "旧课题记忆").unwrap();
        std::fs::write(root.join(".galen").join("active-task.json"), "{}").unwrap();
        archive_topic_context(&root).unwrap();
        assert!(!root.join("GALEN.md").exists());
        assert!(!root.join(".galen").join("active-task.json").exists());
        let archive = std::fs::read_dir(root.join(".galen").join("topic-archives"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            std::fs::read_to_string(archive.join("GALEN.md")).unwrap(),
            "旧课题记忆"
        );
        assert!(archive.join(".galen_active-task.json").exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
