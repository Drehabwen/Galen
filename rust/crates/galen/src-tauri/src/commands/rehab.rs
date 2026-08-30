use tauri::State;

use super::{lock_mutex, AppState};

#[tauri::command]
pub fn import_rehab_case(
    state: State<AppState>,
    source_path: String,
    case_id: String,
) -> Result<crate::rehab_context::RehabCaseBundle, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("Please select a workspace first")?;
    crate::rehab_context::import_ais_case(&root, &source_path, &case_id)
}

#[tauri::command]
pub fn get_rehab_case(
    state: State<AppState>,
    case_id: String,
) -> Result<crate::rehab_context::RehabCaseBundle, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("Please select a workspace first")?;
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
    let root = backend.get_workspace_root().ok_or("Please select a workspace first")?;
    crate::rehab_context::resolve_review(&root, &case_id, &decision_id, &option_id, &reviewer)
}

#[tauri::command]
pub fn run_rehab_golden_journeys(
    state: State<AppState>,
    source_path: String,
) -> Result<crate::rehab_eval::RehabGoldenEvalReport, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("Please select a workspace first")?;
    crate::rehab_eval::run_golden_journeys(&root, &source_path)
}

#[tauri::command]
pub fn get_agent_benchmark_report(
    state: State<AppState>,
) -> Result<crate::agent_benchmark::AgentBenchmarkReport, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("Please select a workspace first")?;
    crate::agent_benchmark::load_latest(&root)
}
