pub mod artifact;
pub mod backend;
mod chat_loop;
pub mod chat_session;
mod commands;
mod context_compaction;
mod context_engine;
#[cfg(test)]
mod context_engine_tests;
pub mod conversation_memory;
pub mod eval;
pub mod eval_report;
pub mod evidence;
pub mod evidence_search;
pub mod mcp_client;
pub mod modes;
pub mod personas;
pub mod probe;
pub mod rag_eval;
pub mod research_task;
pub mod runtime_manager;
pub mod skills;
mod task_contract;
pub mod tools;
mod workspace;

pub use modes::ChatMode;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let backend = backend::ChatBackend::new();
    let ws_config = workspace::WorkspaceConfig::load();
    if let Some(path) = ws_config
        .workspace_root
        .as_ref()
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_dir())
    {
        backend.set_workspace_root(Some(path));
    }

    // Write default MCP config if it doesn't exist yet
    crate::mcp_client::McpConfig::write_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            Ok(())
        })
        .manage(AppState {
            backend: std::sync::Mutex::new(backend),
            ws_config: std::sync::Mutex::new(ws_config),
            mode: std::sync::Mutex::new(crate::modes::load_mode()),
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
            commands::get_chat_session,
            commands::get_conversation_decisions,
            commands::revise_conversation_decision,
            commands::dismiss_conversation_decision,
            commands::clear_chat_session,
            commands::get_runtime_status,
            commands::get_mcp_status,
            commands::get_memory_status,
            commands::append_memory,
            commands::get_model_status,
            commands::save_api_key,
            commands::test_model_connection,
            commands::append_evidence,
            commands::get_evidence,
            commands::get_artifacts,
            commands::create_research_task,
            commands::get_active_research_task,
            commands::save_research_task_nodes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Galen");
}
