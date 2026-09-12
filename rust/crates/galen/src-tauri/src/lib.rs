pub mod agent_benchmark;
pub mod architecture_variant;
pub mod artifact;
pub mod backend;
pub mod capability;
mod chat_loop;
pub mod chat_session;
mod commands;
pub mod connectors;
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
pub mod pi_event;
pub mod pi_kernel;
pub mod probe;
pub mod rag_eval;
pub mod rehab_context;
pub mod rehab_eval;
pub mod research_task;
pub mod review_flow;
pub mod runtime_manager;
pub mod search_run;
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
        .plugin(tauri_plugin_opener::init())
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
            commands::get_capabilities,
            commands::get_modes,
            commands::get_mode,
            commands::set_mode,
            commands::get_personas,
            commands::get_persona,
            commands::set_persona,
            commands::workspace::get_workspace_root,
            commands::workspace::set_workspace,
            commands::workspace::list_workspace_files,
            commands::workspace::read_workspace_file,
            commands::workspace::read_artifact_bytes,
            commands::workspace::write_cleaned_dataset,
            commands::analyze_clinical_case,
            commands::send_message,
            commands::get_chat_session,
            commands::get_conversation_decisions,
            commands::revise_conversation_decision,
            commands::dismiss_conversation_decision,
            commands::clear_chat_session,
            commands::start_new_research_topic,
            commands::get_runtime_status,
            commands::get_mcp_status,
            commands::get_memory_status,
            commands::append_memory,
            commands::get_model_status,
            commands::save_api_key,
            commands::test_model_connection,
            commands::append_evidence,
            commands::get_evidence,
            commands::export_evidence_citations,
            commands::get_review_flow,
            commands::save_review_flow,
            commands::get_literature_coverage,
            commands::get_artifacts,
            commands::create_research_task,
            commands::get_active_research_task,
            commands::get_pi_snapshot,
            commands::get_pi_events,
            commands::pi_start_node,
            commands::pi_complete_node,
            commands::pi_block_node,
            commands::pi_approve_node,
            commands::pi_assign_node,
            commands::rehab::import_rehab_case,
            commands::rehab::import_governed_dataset_to_rehab_timeline,
            commands::rehab::discover_research_data_source,
            commands::rehab::import_research_data_source,
            commands::rehab::get_rehab_case,
            commands::rehab::list_rehab_cases,
            commands::rehab::resolve_rehab_review,
            commands::rehab::run_rehab_golden_journeys,
            commands::rehab::get_agent_benchmark_report,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Galen");
}
