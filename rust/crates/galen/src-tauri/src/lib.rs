pub mod backend;
mod commands;
pub mod mcp_client;
pub mod modes;
pub mod personas;
pub mod runtime_manager;
pub mod skills;
pub mod tools;
mod workspace;

pub use modes::ChatMode;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let backend = backend::ChatBackend::new();
    let ws_config = workspace::WorkspaceConfig::load();

    // Write default MCP config if it doesn't exist yet
    crate::mcp_client::McpConfig::write_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            backend: std::sync::Mutex::new(backend),
            ws_config: std::sync::Mutex::new(ws_config),
            mode: std::sync::Mutex::new(ChatMode::default()),
            persona: std::sync::Mutex::new(personas::medical_persona()),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_models,
            commands::get_modes,
            commands::get_mode,
            commands::set_mode,
            commands::get_personas,
            commands::get_persona,
            commands::set_persona,
            commands::get_workspace_root,
            commands::set_workspace,
            commands::list_workspace_files,
            commands::read_workspace_file,
            commands::analyze_clinical_case,
            commands::send_message,
            commands::get_runtime_status,
            commands::get_mcp_status,
            commands::get_memory_status,
            commands::save_plan,
            commands::load_plan,
            commands::append_memory,
            commands::get_model_status,
            commands::save_api_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Galen");
}
