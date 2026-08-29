use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{Emitter, State, Window};

use crate::backend::{self, ChatBackend, ChatEvent, FileEntry, ModelConfig};
use crate::modes::ChatMode;
use crate::research_task::{ResearchNode, ResearchTask};
use crate::runtime_manager::{self, McpServerStatus, RuntimeStatus};
use crate::workspace::WorkspaceConfig;
use api::{InputContentBlock, InputMessage, MessageRequest};
use medical_core::clinical::ClinicalCaseInput;

pub struct AppState {
    pub backend: Mutex<ChatBackend>,
    pub ws_config: Mutex<WorkspaceConfig>,
    pub mode: Mutex<ChatMode>,
    pub persona: Mutex<crate::personas::Persona>,
}

/// Lock a std::sync::Mutex and map the poison error to a String.
fn lock_mutex<T>(m: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, String> {
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

#[tauri::command]
pub fn get_workspace_root(state: State<AppState>) -> Result<Option<String>, String> {
    Ok(lock_mutex(&state.backend)?
        .get_workspace_root()
        .map(|p| p.to_string_lossy().to_string()))
}

// ---------------------------------------------------------------------------
// Simple query commands (no async work)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn set_workspace(state: State<AppState>, path: String) -> Result<(), String> {
    let pb = std::path::PathBuf::from(&path);
    if !pb.exists() || !pb.is_dir() {
        return Err("Path does not exist or is not a directory".into());
    }
    let backend = lock_mutex(&state.backend)?;
    backend.set_workspace_root(Some(pb));
    let mut config = lock_mutex(&state.ws_config)?;
    config.set_workspace(&path);
    Ok(())
}

#[tauri::command]
pub fn list_workspace_files(
    state: State<AppState>,
    path: Option<String>,
) -> Result<Vec<FileEntry>, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend
        .get_workspace_root()
        .ok_or("No workspace selected")?;
    let target = crate::tools::workspace_path::resolve_workspace_path_from_root(
        &root,
        path.as_deref().unwrap_or(""),
    )?;

    let mut entries = Vec::new();
    let dir_iter =
        std::fs::read_dir(&target).map_err(|e| format!("Failed to read directory: {e}"))?;
    for entry in dir_iter {
        let entry = entry.map_err(|e| format!("Failed: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        let meta = entry.metadata().ok();
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let ep = entry.path();
        let rel = ep
            .strip_prefix(&target)
            .unwrap_or(&ep)
            .to_string_lossy()
            .to_string();
        entries.push(FileEntry {
            name,
            path: rel,
            is_dir,
            size,
        });
    }
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
    Ok(entries)
}

#[tauri::command]
pub fn read_workspace_file(state: State<AppState>, path: String) -> Result<String, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend
        .get_workspace_root()
        .ok_or("No workspace selected")?;
    let target = crate::tools::workspace_path::resolve_workspace_path_from_root(&root, &path)?;
    std::fs::read_to_string(&target).map_err(|e| format!("Failed to read file: {e}"))
}

// ---------------------------------------------------------------------------
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
}

// ---------------------------------------------------------------------------
// GALEN.md memory
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct MemoryStatus {
    pub exists: bool,
    pub size: u64,
    pub preview: String, // first 500 chars
}

#[tauri::command]
pub fn get_memory_status(state: State<AppState>) -> Result<MemoryStatus, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = match backend.get_workspace_root() {
        Some(r) => r,
        None => {
            return Ok(MemoryStatus {
                exists: false,
                size: 0,
                preview: String::new(),
            })
        }
    };
    let path = root.join("GALEN.md");
    match std::fs::metadata(&path) {
        Ok(meta) => {
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            let preview = content.chars().take(500).collect();
            Ok(MemoryStatus {
                exists: true,
                size: meta.len(),
                preview,
            })
        }
        Err(_) => Ok(MemoryStatus {
            exists: false,
            size: 0,
            preview: String::new(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Durable research task state
// ---------------------------------------------------------------------------

/// Create a new host-authoritative research task in
/// `<workspace>/.galen/tasks/<task-id>/task.json` and make it active.
#[tauri::command]
pub fn create_research_task(
    state: State<AppState>,
    title: String,
    goal: String,
    nodes: Vec<ResearchNode>,
) -> Result<ResearchTask, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    crate::research_task::create_task(&root, title, goal, nodes)
}

/// Restore the active research task. If this workspace only has the old
/// frontend-owned `plan.json`, migrate it once without deleting the source.
#[tauri::command]
pub fn get_active_research_task(state: State<AppState>) -> Result<Option<ResearchTask>, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = match backend.get_workspace_root() {
        Some(root) => root,
        None => return Ok(None),
    };
    let Some(_) = crate::research_task::load_or_migrate_active_task(&root)? else {
        return Ok(None);
    };
    // Complete legacy evidence migration before returning the revision that
    // the frontend will use for its first CAS write.
    crate::evidence::load_evidence(&root)?;
    crate::research_task::load_active_task(&root)
}

/// Replace the node snapshot for a task. The host derives the task-level
/// status instead of accepting it from the webview.
#[tauri::command]
pub fn save_research_task_nodes(
    state: State<AppState>,
    task_id: String,
    expected_revision: u64,
    nodes: Vec<ResearchNode>,
) -> Result<ResearchTask, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    crate::research_task::replace_nodes(&root, &task_id, expected_revision, nodes)
}

/// Append one line to `<workspace>/GALEN.md` (loop output becomes memory).
/// Entry format follows the convention: `date | source | key finding | related file`.
#[tauri::command]
pub fn append_memory(state: State<AppState>, entry: String) -> Result<(), String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    let path = root.join("GALEN.md");
    let mut content = std::fs::read_to_string(&path).unwrap_or_default();
    if content.trim().is_empty() {
        content.push_str("# GALEN 项目记忆\n\n");
    }
    if !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&format!("- {entry}\n"));
    std::fs::write(&path, content).map_err(|e| format!("写入 GALEN.md 失败: {e}"))
}

/// Append one structured evidence record to the active task's evidence ledger.
#[tauri::command]
pub fn append_evidence(
    state: State<AppState>,
    evidence: crate::evidence::Evidence,
) -> Result<ResearchTask, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    crate::evidence::append_evidence_file(&root, evidence)
}

/// Read the active task's full evidence chain.
#[tauri::command]
pub fn get_evidence(state: State<AppState>) -> Result<Vec<crate::evidence::Evidence>, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = match backend.get_workspace_root() {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };
    crate::evidence::load_evidence(&root)
}

#[tauri::command]
pub fn get_artifacts(
    state: State<AppState>,
) -> Result<Vec<crate::artifact::ArtifactRecord>, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = match backend.get_workspace_root() {
        Some(root) => root,
        None => return Ok(Vec::new()),
    };
    crate::artifact::list_artifacts(&root)
}

// ---------------------------------------------------------------------------
// Rehabilitation context — host-authoritative case evidence and review state
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn import_rehab_case(
    state: State<AppState>,
    source_path: String,
    case_id: String,
) -> Result<crate::rehab_context::RehabCaseBundle, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    crate::rehab_context::import_ais_case(&root, &source_path, &case_id)
}

#[tauri::command]
pub fn get_rehab_case(
    state: State<AppState>,
    case_id: String,
) -> Result<crate::rehab_context::RehabCaseBundle, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    crate::rehab_context::load_case_bundle(&root, &case_id)
}

#[tauri::command]
pub fn list_rehab_cases(
    state: State<AppState>,
) -> Result<Vec<crate::rehab_context::RehabCaseSummary>, String> {
    let backend = lock_mutex(&state.backend)?;
    let Some(root) = backend.get_workspace_root() else {
        return Ok(Vec::new());
    };
    crate::rehab_context::list_case_summaries(&root)
}

#[tauri::command]
pub fn resolve_rehab_review(
    state: State<AppState>,
    case_id: String,
    decision_id: String,
    option_id: String,
    reviewer: String,
) -> Result<crate::rehab_context::RehabCaseBundle, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    crate::rehab_context::resolve_review(&root, &case_id, &decision_id, &option_id, &reviewer)
}

#[tauri::command]
pub fn run_rehab_golden_journeys(
    state: State<AppState>,
    source_path: String,
) -> Result<crate::rehab_eval::RehabGoldenEvalReport, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    crate::rehab_eval::run_golden_journeys(&root, &source_path)
}

#[tauri::command]
pub fn get_agent_benchmark_report(
    state: State<AppState>,
) -> Result<crate::agent_benchmark::AgentBenchmarkReport, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    crate::agent_benchmark::load_latest(&root)
}

// ---------------------------------------------------------------------------
// Model / API key status
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct ModelStatus {
    pub name: String,
    pub model_id: String,
    pub description: Option<String>,
    pub api_key_present: bool,
    pub api_key_masked: Option<String>,
    pub base_url: Option<String>,
    pub max_tokens: Option<u32>,
    pub is_default: bool,
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "••••".to_string()
    } else {
        format!("{}…{}", &key[..4], &key[key.len() - 4..])
    }
}

/// Report configured models and whether each has an API key (masked only).
/// Lets the user verify at a glance that credentials are in place.
#[tauri::command]
pub fn get_model_status(state: State<AppState>) -> Result<Vec<ModelStatus>, String> {
    let backend = lock_mutex(&state.backend)?;
    let default_alias = backend.router.default_alias().to_string();
    let mut statuses: Vec<ModelStatus> = backend
        .router
        .all_models()
        .iter()
        .map(|(alias, entry)| {
            let masked = entry
                .api_key
                .as_deref()
                .filter(|k| !k.is_empty())
                .map(mask_key);
            ModelStatus {
                name: alias.clone(),
                model_id: entry.model_id.clone(),
                description: entry.description.clone(),
                api_key_present: masked.is_some(),
                api_key_masked: masked,
                base_url: entry.base_url.clone(),
                max_tokens: entry.max_tokens,
                is_default: *alias == default_alias,
            }
        })
        .collect();
    statuses.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(statuses)
}

/// Test the default model connection with a minimal "ping" request.
/// Returns the responding model id on success, or a descriptive error.
#[tauri::command]
pub async fn test_model_connection(state: State<'_, AppState>) -> Result<String, String> {
    let (_alias, model_id, client) = {
        let backend = lock_mutex(&state.backend)?;
        let alias = backend.router.default_alias().to_string();
        let model_id = backend.router.resolve_model_id(&alias);
        let router = backend.router.clone();
        let client = backend::make_client(&alias, &router)?;
        (alias, model_id, client)
    };

    let request = MessageRequest {
        model: model_id.clone(),
        max_tokens: 8,
        messages: vec![InputMessage {
            role: "user".to_string(),
            content: vec![InputContentBlock::Text {
                text: "ping".to_string(),
            }],
        }],
        system: None,
        tools: None,
        tool_choice: None,
        stream: false,
        temperature: None,
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: None,
        reasoning_effort: None,
        thinking: None,
    };

    match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        client.send_message(&request),
    )
    .await
    {
        Ok(Ok(_)) => Ok(format!("连接成功：{model_id} 响应正常")),
        Ok(Err(e)) => Err(format!("连接失败：{e}")),
        Err(_) => Err("连接超时（30 秒），请检查网络或 API Key".to_string()),
    }
}
