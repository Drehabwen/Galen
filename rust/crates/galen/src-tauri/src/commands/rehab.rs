use tauri::State;

use super::{lock_mutex, AppState};

fn selected_workspace(state: &State<AppState>) -> Result<std::path::PathBuf, String> {
    lock_mutex(&state.backend)?
        .get_workspace_root()
        .ok_or_else(|| "请先选择研究工作区。".to_string())
}

#[tauri::command]
pub fn discover_research_data_source(
    source_id: String,
    case_hint: Option<String>,
) -> Result<crate::connectors::ConnectorPreview, String> {
    crate::connectors::discover_latest(&source_id, case_hint.as_deref())
}

#[tauri::command]
pub fn import_research_data_source(
    state: State<AppState>,
    request: crate::connectors::ConnectorImportRequest,
) -> Result<crate::rehab_context::GovernedTimelineImportOutput, String> {
    let root = selected_workspace(&state)?;
    crate::connectors::import_from_export(&root, request)
}

#[tauri::command]
pub fn import_rehab_case(
    state: State<AppState>,
    source_path: String,
    case_id: String,
) -> Result<crate::rehab_context::RehabCaseBundle, String> {
    let root = selected_workspace(&state)?;
    crate::rehab_context::import_ais_case(&root, &source_path, &case_id)
}

#[tauri::command]
pub fn import_governed_dataset_to_rehab_timeline(
    state: State<AppState>,
    input: crate::rehab_context::GovernedTimelineImportInput,
) -> Result<crate::rehab_context::GovernedTimelineImportOutput, String> {
    let root = selected_workspace(&state)?;
    crate::rehab_context::import_governed_timeline(&root, input)
}

#[tauri::command]
pub fn get_rehab_case(
    state: State<AppState>,
    case_id: String,
) -> Result<crate::rehab_context::RehabCaseBundle, String> {
    let root = selected_workspace(&state)?;
    crate::rehab_context::load_case_bundle(&root, &case_id)
}

#[tauri::command]
pub fn list_rehab_cases(
    state: State<AppState>,
) -> Result<Vec<crate::rehab_context::RehabCaseSummary>, String> {
    let Ok(root) = selected_workspace(&state) else {
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
    reviewer: Option<String>,
) -> Result<crate::rehab_context::RehabCaseBundle, String> {
    let root = selected_workspace(&state)?;
    let reviewer = reviewer.as_deref().unwrap_or("human-reviewer");
    crate::rehab_context::resolve_review(&root, &case_id, &decision_id, &option_id, reviewer)
}

#[tauri::command]
pub fn run_rehab_golden_journeys(
    state: State<AppState>,
    source_path: String,
) -> Result<crate::rehab_eval::RehabGoldenEvalReport, String> {
    let root = selected_workspace(&state)?;
    crate::rehab_eval::run_golden_journeys(&root, &source_path)
}

#[tauri::command]
pub fn get_agent_benchmark_report(
    state: State<AppState>,
) -> Result<crate::agent_benchmark::AgentBenchmarkReport, String> {
    let root = selected_workspace(&state)?;
    crate::agent_benchmark::load_latest(&root)
}
