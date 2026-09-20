use super::{lock_mutex, AppState, ResearchNode, ResearchTask};
use medical_core::types::CitationStyle;
use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

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
    let task = crate::research_task::create_task(&root, title, goal, nodes)?;
    crate::pi_kernel::record_project_created(&root, task).map(|snapshot| snapshot.task)
}

/// Merge new scope/constraint terms into the active project without dropping
/// earlier conversational decisions. The host keeps the canonical revision so
/// two open views cannot silently overwrite each other.
#[tauri::command]
pub fn update_research_context(
    state: State<AppState>,
    task_id: String,
    expected_revision: u64,
    patch: crate::research_task::ActiveResearchContext,
) -> Result<ResearchTask, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend
        .get_workspace_root()
        .ok_or("workspace is not selected")?;
    let task =
        crate::research_task::update_active_context(&root, &task_id, expected_revision, patch)?;
    crate::pi_event::append_event(
        &root,
        &task.task_id,
        None,
        crate::pi_event::PiEventKind::ContextUpdated,
        &format!("context-updated:{}", task.revision),
        serde_json::json!({
            "revision": task.revision,
            "activeContext": task.active_context,
        }),
    )?;
    Ok(task)
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
    crate::pi_kernel::load_active_snapshot(&root)
        .map(|snapshot| snapshot.map(|snapshot| snapshot.task))
}

/// Return the PI's host-authoritative execution view for the active project.
#[tauri::command]
pub fn get_pi_snapshot(
    state: State<AppState>,
) -> Result<Option<crate::pi_kernel::PiSnapshot>, String> {
    let backend = lock_mutex(&state.backend)?;
    let Some(root) = backend.get_workspace_root() else {
        return Ok(None);
    };
    crate::pi_kernel::load_active_snapshot(&root)
}

#[tauri::command]
pub fn get_pi_events(
    state: State<AppState>,
    task_id: String,
) -> Result<Vec<crate::pi_event::PiEvent>, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    let active =
        crate::research_task::load_active_task(&root)?.ok_or("当前工作区没有活动研究任务")?;
    if active.task_id != task_id {
        return Err("PI_TASK_CONFLICT: 只能读取当前活动研究任务的事件".to_string());
    }
    crate::pi_event::read_events(&root, &task_id)
}

#[tauri::command]
pub fn pi_start_node(
    state: State<AppState>,
    task_id: String,
    expected_revision: u64,
    node_id: String,
    idempotency_key: String,
) -> Result<crate::pi_kernel::PiSnapshot, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    crate::pi_kernel::start_node(
        &root,
        &task_id,
        expected_revision,
        &node_id,
        &idempotency_key,
    )
}

#[tauri::command]
pub fn pi_complete_node(
    state: State<AppState>,
    task_id: String,
    expected_revision: u64,
    node_id: String,
    result: String,
    evidence: Vec<String>,
    outputs: Vec<String>,
    idempotency_key: String,
) -> Result<crate::pi_kernel::PiSnapshot, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    crate::pi_kernel::complete_node(
        &root,
        &task_id,
        expected_revision,
        &node_id,
        result,
        evidence,
        outputs,
        &idempotency_key,
    )
}

#[tauri::command]
pub fn pi_block_node(
    state: State<AppState>,
    task_id: String,
    expected_revision: u64,
    node_id: String,
    reason: String,
    idempotency_key: String,
) -> Result<crate::pi_kernel::PiSnapshot, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    crate::pi_kernel::block_node(
        &root,
        &task_id,
        expected_revision,
        &node_id,
        reason,
        &idempotency_key,
    )
}

#[tauri::command]
pub fn pi_approve_node(
    state: State<AppState>,
    task_id: String,
    expected_revision: u64,
    node_id: String,
    idempotency_key: String,
) -> Result<crate::pi_kernel::PiSnapshot, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    crate::pi_kernel::approve_node(
        &root,
        &task_id,
        expected_revision,
        &node_id,
        &idempotency_key,
    )
}

#[tauri::command]
pub fn pi_assign_node(
    state: State<AppState>,
    task_id: String,
    expected_revision: u64,
    node_id: String,
    owner: Option<String>,
    idempotency_key: String,
) -> Result<crate::pi_kernel::PiSnapshot, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    crate::pi_kernel::assign_node(
        &root,
        &task_id,
        expected_revision,
        &node_id,
        owner,
        &idempotency_key,
    )
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
    let evidence_id = evidence.id.clone();
    let node_id = evidence.node_id.clone();
    let task = crate::evidence::append_evidence_file(&root, evidence)?;
    crate::pi_event::append_event(
        &root,
        &task.task_id,
        Some(&node_id),
        crate::pi_event::PiEventKind::EvidenceAttached,
        &format!("evidence-attached:{evidence_id}"),
        serde_json::json!({"evidenceId": evidence_id, "revision": task.revision}),
    )?;
    Ok(task)
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
pub fn get_review_flow(state: State<AppState>) -> Result<crate::review_flow::ReviewFlow, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = match backend.get_workspace_root() {
        Some(root) => root,
        None => {
            return Ok(crate::review_flow::ReviewFlow {
                task_id: None,
                identified: 0,
                duplicates_removed: None,
                screened: None,
                excluded: None,
                full_text_assessed: None,
                included: None,
                updated_at: None,
            })
        }
    };
    crate::review_flow::load_review_flow(&root)
}

#[tauri::command]
pub fn save_review_flow(
    state: State<AppState>,
    flow: crate::review_flow::ReviewFlowInput,
) -> Result<crate::review_flow::ReviewFlow, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    crate::review_flow::save_review_flow(&root, flow)
}

/// Export the PubMed records explicitly cited in the active evidence ledger.
/// The export is deliberately derived from PMID-labelled evidence only: plain
/// numbers in prose never become references, and every emitted record is
/// fetched afresh from PubMed before it reaches a citation file.
#[tauri::command]
pub async fn export_evidence_citations(
    state: State<'_, AppState>,
    style: String,
) -> Result<crate::artifact::ArtifactRecord, String> {
    let (root, medical) = {
        let backend = lock_mutex(&state.backend)?;
        (
            backend.get_workspace_root().ok_or("请先选择工作区")?,
            backend.medical.clone(),
        )
    };
    let pmids = cited_pmids(&crate::evidence::load_evidence(&root)?);
    if pmids.is_empty() {
        return Err("当前证据账本中没有 PMID 标注，无法导出可核验参考文献。".into());
    }
    let citation_style =
        CitationStyle::from_str(&style).ok_or("仅支持 vancouver、bibtex、ris、apa 或 mla 格式")?;
    let papers = medical
        .pubmed
        .fetch_articles(&pmids)
        .await
        .map_err(|error| format!("导出前回查 PubMed 失败: {error}"))?;
    if papers.is_empty() {
        return Err("PubMed 未返回可导出的题录。".into());
    }
    let content = medical.format_citations(&papers, citation_style);
    let extension = match citation_style {
        CitationStyle::BibTeX => "bib",
        CitationStyle::RIS => "ris",
        _ => "txt",
    };
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default();
    let relative_path = format!("output/galen-verified-citations-{stamp}.{extension}");
    let output = root.join(&relative_path);
    let parent = output.parent().ok_or("引用导出路径无效")?;
    std::fs::create_dir_all(parent).map_err(|error| format!("创建引用导出目录失败: {error}"))?;
    std::fs::write(&output, content).map_err(|error| format!("写入引用文件失败: {error}"))?;
    let task_id = crate::research_task::load_active_task(&root)?.map(|task| task.task_id);
    crate::artifact::register_file(&root, &relative_path, task_id, None)
}

pub(super) fn cited_pmids(evidence: &[crate::evidence::Evidence]) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for item in evidence {
        let text = format!(
            "{}\n{}",
            item.claim,
            item.detail.as_deref().unwrap_or_default()
        );
        let lower = text.to_ascii_lowercase();
        let mut offset = 0;
        while let Some(relative) = lower[offset..].find("pmid") {
            let start = offset + relative + 4;
            let digits: String = text[start..]
                .chars()
                .skip_while(|ch| !ch.is_ascii_digit())
                .take_while(|ch| ch.is_ascii_digit())
                .take(10)
                .collect();
            if (5..=9).contains(&digits.len()) {
                ids.insert(digits);
            }
            offset = start;
        }
    }
    ids.into_iter().collect()
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
