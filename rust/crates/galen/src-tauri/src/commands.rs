use std::sync::Mutex;

use tauri::{Emitter, State, Window};

use api::{InputContentBlock, InputMessage, MessageRequest};
use crate::backend::{self, ChatBackend, ChatEvent, FileEntry, ModelConfig};
use crate::modes::ChatMode;
use crate::runtime_manager::{self, McpServerStatus, RuntimeStatus};
use crate::workspace::WorkspaceConfig;
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
    let target = if let Some(ref sub) = path {
        root.join(sub)
    } else {
        root.clone()
    };

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
    let target = root.join(&path);
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
    let thinking_level = thinking_level.unwrap_or_else(|| "medium".to_string());
    // Phase 1: extract all needed data from locked state (before any .await)
    let (model_alias, model_id, medical, router, workspace_root, persona) = {
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
        let ws = Mutex::new(backend.workspace_root.lock().map_err(|e| format!("{e}"))?.clone());
        let persona = crate::personas::find_persona(&persona_id);
        (model_alias, model_id, medical, router, ws, persona)
    };

    // Build tagged event names for session isolation
    let suffix = tag.as_ref().map(|t| format!(":{t}")).unwrap_or_default();

    // Phase 2: spawn chat in background, emitting events to the window
    let window_clone = window.clone();
    let err_tag = suffix.clone();
    tokio::spawn(async move {
        // Parse history from frontend (simple {role, content}[] format)
        let input_messages = backend::parse_history_json(&history_json);
        let result = backend::run_chat(
            model_alias, model_id, message, input_messages, mode, persona, thinking_level,
            medical, router, workspace_root,
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
                        ChatEvent::Done(text) => emit!("chat-done", text),
                        ChatEvent::ThinkingDelta(text) => emit!("chat-thinking-delta", text),
                        ChatEvent::ThinkingDone(text) => emit!("chat-thinking-done", text),
                        ChatEvent::Error(e) => emit!("chat-error", e.as_str()),
                        ChatEvent::SearchResults(papers) => emit!("search-results", papers),
                        ChatEvent::WorkspaceRoot(path) => emit!("workspace-root", path.as_str()),
                        ChatEvent::WorkspaceFileList(files) => emit!("workspace-file-list", files),
                        ChatEvent::WorkspaceFileContent { path, content } => {
                            emit!("workspace-file-content", serde_json::json!({ "path": path, "content": content }))
                        }
                    }
                }
            },
        ).await;

        if let Err(e) = result {
            let ename = format!("chat-error{err_tag}");
            let _ = window_clone.emit(&ename, &e);
        }
    });

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
default = "deepseek-v4-pro"
fast = "deepseek-v4-flash"

[models.deepseek-v4-pro]
provider = "openai_compat"
api_key = "{api_key}"
model_id = "deepseek-v4-pro"
base_url = "https://api.deepseek.com/v1"
description = "DeepSeek V4 Pro（默认，最强推理）"
max_tokens = 32768

[models.deepseek-v4-flash]
provider = "openai_compat"
api_key = "{api_key}"
model_id = "deepseek-v4-flash"
base_url = "https://api.deepseek.com/v1"
description = "DeepSeek V4 Flash（快速）"
max_tokens = 32768
"#;

#[tauri::command]
pub fn save_api_key(api_key: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot find home directory")?;
    let galen_dir = home.join(".galen");
    std::fs::create_dir_all(&galen_dir).map_err(|e| format!("{e}"))?;

    let models_path = galen_dir.join("models.toml");

    let content = if models_path.exists() {
        let existing =
            std::fs::read_to_string(&models_path).map_err(|e| format!("读取配置失败: {e}"))?;
        // Inject api_key into the DeepSeek template blocks that lack one
        if existing.contains("[models.deepseek-v4-pro]") && !existing.contains("api_key =") {
            existing
                .replace(
                    "[models.deepseek-v4-pro]",
                    &format!("[models.deepseek-v4-pro]\napi_key = \"{api_key}\""),
                )
                .replace(
                    "[models.deepseek-v4-flash]",
                    &format!("[models.deepseek-v4-flash]\napi_key = \"{api_key}\""),
                )
        } else if existing.contains("[models.default]") && !existing.contains("api_key =") {
            existing.replace("[models.default]", &format!("[models.default]\napi_key = \"{api_key}\""))
        } else {
            format!("{}\n\n[models.imported]\nprovider = \"openai_compat\"\napi_key = \"{api_key}\"\nmodel_id = \"deepseek-v4-pro\"\nbase_url = \"https://api.deepseek.com/v1\"\n", existing)
        }
    } else {
        DEFAULT_MODEL_TEMPLATE.replace("{api_key}", &api_key)
    };

    std::fs::write(&models_path, &content).map_err(|e| format!("写入失败: {e}"))?;
    Ok(())
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
        None => return Ok(MemoryStatus {
            exists: false,
            size: 0,
            preview: String::new(),
        }),
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
// Plan persistence (task-level loop state)
// ---------------------------------------------------------------------------

/// Persist the research plan (nodes + evidence) to `<workspace>/plan.json`.
/// The loop's output is stored so it survives restarts and feeds the next
/// task's context.
#[tauri::command]
pub fn save_plan(state: State<AppState>, plan_json: String) -> Result<(), String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    let path = root.join("plan.json");
    std::fs::write(&path, plan_json).map_err(|e| format!("写入 plan.json 失败: {e}"))
}

/// Load a previously persisted plan, if any.
#[tauri::command]
pub fn load_plan(state: State<AppState>) -> Result<Option<String>, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = match backend.get_workspace_root() {
        Some(r) => r,
        None => return Ok(None),
    };
    let path = root.join("plan.json");
    match std::fs::read_to_string(&path) {
        Ok(content) => Ok(Some(content)),
        Err(_) => Ok(None),
    }
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

    match tokio::time::timeout(std::time::Duration::from_secs(30), client.send_message(&request)).await {
        Ok(Ok(_)) => Ok(format!("连接成功：{model_id} 响应正常")),
        Ok(Err(e)) => Err(format!("连接失败：{e}")),
        Err(_) => Err("连接超时（30 秒），请检查网络或 API Key".to_string()),
    }
}
