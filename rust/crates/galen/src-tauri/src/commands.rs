use std::sync::Mutex;

use tauri::{Emitter, State, Window};

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
) -> Result<(), String> {
    // Phase 1: extract all needed data from locked state (before any .await)
    let (model_id, medical, router, workspace_root, persona) = {
        let backend = lock_mutex(&state.backend)?;
        let model_id = backend.resolve_model(&model_alias);
        let medical = backend.medical.clone();
        let router = backend.router.clone();
        let ws = Mutex::new(backend.workspace_root.lock().map_err(|e| format!("{e}"))?.clone());
        let persona = crate::personas::find_persona(&persona_id);
        (model_id, medical, router, ws, persona)
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
            model_alias, model_id, message, input_messages, mode, persona,
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
default = "default"

[models.default]
provider = "openai_compat"
api_key = "{api_key}"
model_id = "deepseek-v4-pro"
base_url = "https://api.deepseek.com/v1"
description = "Default model — edit this file to change provider/model"
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
        // Inject api_key into the first [models.*] block that lacks one
        if existing.contains("[models.deepseek]") && !existing.contains("api_key =") {
            existing.replace("[models.deepseek]", &format!("[models.deepseek]\napi_key = \"{api_key}\""))
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
