//! Host-authoritative PI orchestration kernel.
//!
//! The webview sends intent and renders returned snapshots. Dependency checks,
//! node transitions and the next executable work are owned here.

use crate::pi_event::{self, PiEventKind};
use crate::research_task::{ResearchNode, ResearchTask};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PiSnapshot {
    pub task: ResearchTask,
    pub ready_node_ids: Vec<String>,
    pub active_node_ids: Vec<String>,
    pub blocked_node_ids: Vec<String>,
    pub awaiting_decision_node_ids: Vec<String>,
    pub last_event_sequence: u64,
}

pub fn load_active_snapshot(workspace: &Path) -> Result<Option<PiSnapshot>, String> {
    let Some(task) = crate::research_task::load_or_migrate_active_task(workspace)? else {
        return Ok(None);
    };
    snapshot(workspace, task).map(Some)
}

pub fn record_project_created(workspace: &Path, task: ResearchTask) -> Result<PiSnapshot, String> {
    pi_event::append_event(
        workspace,
        &task.task_id,
        None,
        PiEventKind::ProjectCreated,
        &format!("project-created:{}", task.task_id),
        json!({"title": task.title, "goal": task.goal, "revision": task.revision}),
    )?;
    snapshot(workspace, task)
}

pub fn start_node(
    workspace: &Path,
    task_id: &str,
    expected_revision: u64,
    node_id: &str,
    idempotency_key: &str,
) -> Result<PiSnapshot, String> {
    if let Some(existing) = existing_operation_snapshot(workspace, task_id, idempotency_key)? {
        return Ok(existing);
    }
    let mut task = active_task(workspace, task_id, expected_revision)?;
    let index = node_index(&task, node_id)?;
    match task.nodes[index].status.as_str() {
        "completed" => return Err("已完成节点不能重新开始".to_string()),
        "blocked" => return Err("节点仍处于阻塞状态，请先解决阻塞原因".to_string()),
        _ => {}
    }
    if is_awaiting_decision(&task.nodes[index]) {
        return Err("节点需要 PI/用户批准后才能开始".to_string());
    }
    require_dependencies(&task, index)?;

    if task.nodes[index].status != "running" {
        task.nodes[index].status = "running".to_string();
        task =
            crate::research_task::replace_nodes(workspace, task_id, expected_revision, task.nodes)?;
    }
    pi_event::append_event(
        workspace,
        task_id,
        Some(node_id),
        PiEventKind::NodeStarted,
        idempotency_key,
        json!({"revision": task.revision}),
    )?;
    snapshot(workspace, task)
}

#[allow(clippy::too_many_arguments)]
pub fn complete_node(
    workspace: &Path,
    task_id: &str,
    expected_revision: u64,
    node_id: &str,
    result: String,
    evidence: Vec<String>,
    outputs: Vec<String>,
    idempotency_key: &str,
) -> Result<PiSnapshot, String> {
    if let Some(existing) = existing_operation_snapshot(workspace, task_id, idempotency_key)? {
        return Ok(existing);
    }
    let mut task = active_task(workspace, task_id, expected_revision)?;
    let index = node_index(&task, node_id)?;
    let result = result.trim();
    if result.is_empty() {
        return Err("节点完成时必须提供产出摘要".to_string());
    }
    if task.nodes[index].status == "blocked" {
        return Err("阻塞节点不能直接完成，请先解决阻塞原因".to_string());
    }
    require_dependencies(&task, index)?;

    if task.nodes[index].status != "completed" {
        let node = &mut task.nodes[index];
        node.status = "completed".to_string();
        node.result = Some(result.chars().take(10_000).collect());
        extend_unique(&mut node.evidence, evidence);
        extend_unique(&mut node.outputs, outputs);
        task =
            crate::research_task::replace_nodes(workspace, task_id, expected_revision, task.nodes)?;
    }
    pi_event::append_event(
        workspace,
        task_id,
        Some(node_id),
        PiEventKind::NodeCompleted,
        idempotency_key,
        json!({
            "revision": task.revision,
            "result": result.chars().take(2_000).collect::<String>(),
        }),
    )?;
    snapshot(workspace, task)
}

pub fn block_node(
    workspace: &Path,
    task_id: &str,
    expected_revision: u64,
    node_id: &str,
    reason: String,
    idempotency_key: &str,
) -> Result<PiSnapshot, String> {
    if let Some(existing) = existing_operation_snapshot(workspace, task_id, idempotency_key)? {
        return Ok(existing);
    }
    let mut task = active_task(workspace, task_id, expected_revision)?;
    let index = node_index(&task, node_id)?;
    let reason = reason.trim();
    if reason.is_empty() {
        return Err("阻塞节点必须说明原因".to_string());
    }
    let node = &mut task.nodes[index];
    node.status = "blocked".to_string();
    node.result = Some(reason.chars().take(4_000).collect());
    task = crate::research_task::replace_nodes(workspace, task_id, expected_revision, task.nodes)?;
    pi_event::append_event(
        workspace,
        task_id,
        Some(node_id),
        PiEventKind::NodeBlocked,
        idempotency_key,
        json!({"revision": task.revision, "reason": reason}),
    )?;
    snapshot(workspace, task)
}

pub fn approve_node(
    workspace: &Path,
    task_id: &str,
    expected_revision: u64,
    node_id: &str,
    idempotency_key: &str,
) -> Result<PiSnapshot, String> {
    if let Some(existing) = existing_operation_snapshot(workspace, task_id, idempotency_key)? {
        return Ok(existing);
    }
    let mut task = active_task(workspace, task_id, expected_revision)?;
    let index = node_index(&task, node_id)?;
    task.nodes[index].approval_required = false;
    task.nodes[index].status = "approved".to_string();
    task = crate::research_task::replace_nodes(workspace, task_id, expected_revision, task.nodes)?;
    pi_event::append_event(
        workspace,
        task_id,
        Some(node_id),
        PiEventKind::HumanDecisionResolved,
        idempotency_key,
        json!({"decision": "approved", "revision": task.revision}),
    )?;
    snapshot(workspace, task)
}

pub fn assign_node(
    workspace: &Path,
    task_id: &str,
    expected_revision: u64,
    node_id: &str,
    owner: Option<String>,
    idempotency_key: &str,
) -> Result<PiSnapshot, String> {
    if let Some(existing) = existing_operation_snapshot(workspace, task_id, idempotency_key)? {
        return Ok(existing);
    }
    let mut task = active_task(workspace, task_id, expected_revision)?;
    let index = node_index(&task, node_id)?;
    require_dependencies(&task, index)?;
    task.nodes[index].status = "assigned".to_string();
    if let Some(owner) = owner.filter(|value| !value.trim().is_empty()) {
        task.nodes[index].owner = Some(owner.trim().chars().take(120).collect());
    }
    task = crate::research_task::replace_nodes(workspace, task_id, expected_revision, task.nodes)?;
    pi_event::append_event(
        workspace,
        task_id,
        Some(node_id),
        PiEventKind::NodeAssigned,
        idempotency_key,
        json!({"owner": task.nodes[index].owner, "revision": task.revision}),
    )?;
    snapshot(workspace, task)
}

fn active_task(
    workspace: &Path,
    task_id: &str,
    expected_revision: u64,
) -> Result<ResearchTask, String> {
    let task =
        crate::research_task::load_active_task(workspace)?.ok_or("当前工作区没有活动研究任务")?;
    if task.task_id != task_id {
        return Err("PI_TASK_CONFLICT: 活动研究任务已经变化，请刷新后重试".to_string());
    }
    if task.revision != expected_revision {
        return Err(format!(
            "RESEARCH_TASK_CONFLICT: 任务版本已变化（期望 {expected_revision}，当前 {}），请刷新后重试",
            task.revision
        ));
    }
    Ok(task)
}

fn existing_operation_snapshot(
    workspace: &Path,
    task_id: &str,
    idempotency_key: &str,
) -> Result<Option<PiSnapshot>, String> {
    if idempotency_key.trim().is_empty()
        || idempotency_key.len() > 200
        || idempotency_key.contains(['\n', '\r'])
    {
        return Err("PI 操作幂等键无效".to_string());
    }
    if !pi_event::read_events(workspace, task_id)?
        .iter()
        .any(|event| event.idempotency_key == idempotency_key)
    {
        return Ok(None);
    }
    let task =
        crate::research_task::load_active_task(workspace)?.ok_or("当前工作区没有活动研究任务")?;
    if task.task_id != task_id {
        return Err("PI_TASK_CONFLICT: 活动研究任务已经变化，请刷新后重试".to_string());
    }
    snapshot(workspace, task).map(Some)
}

fn snapshot(workspace: &Path, task: ResearchTask) -> Result<PiSnapshot, String> {
    let mut events = pi_event::read_events(workspace, &task.task_id)?;
    if events.is_empty() {
        events.push(pi_event::append_event(
            workspace,
            &task.task_id,
            None,
            PiEventKind::ProjectCreated,
            &format!("project-created:{}", task.task_id),
            json!({
                "title": task.title,
                "goal": task.goal,
                "revision": task.revision,
                "recovered": true,
            }),
        )?);
    }
    for node in task.nodes.iter().filter(|node| is_awaiting_decision(node)) {
        let key = format!("decision-requested:{}", node.id);
        if events.iter().any(|event| event.idempotency_key == key) {
            continue;
        }
        events.push(pi_event::append_event(
            workspace,
            &task.task_id,
            Some(&node.id),
            PiEventKind::HumanDecisionRequested,
            &key,
            json!({"title": node.title, "revision": task.revision}),
        )?);
    }
    let mut ready_node_ids = Vec::new();
    let mut active_node_ids = Vec::new();
    let mut blocked_node_ids = Vec::new();
    let mut awaiting_decision_node_ids = Vec::new();
    for (index, node) in task.nodes.iter().enumerate() {
        match node.status.as_str() {
            "running" => active_node_ids.push(node.id.clone()),
            "blocked" => blocked_node_ids.push(node.id.clone()),
            _ if is_awaiting_decision(node) => awaiting_decision_node_ids.push(node.id.clone()),
            "pending" | "approved" | "assigned" | "returned"
                if missing_dependencies(&task, index).is_empty() =>
            {
                ready_node_ids.push(node.id.clone())
            }
            _ => {}
        }
    }
    Ok(PiSnapshot {
        task,
        ready_node_ids,
        active_node_ids,
        blocked_node_ids,
        awaiting_decision_node_ids,
        last_event_sequence: events.last().map_or(0, |event| event.sequence),
    })
}

fn node_index(task: &ResearchTask, node_id: &str) -> Result<usize, String> {
    task.nodes
        .iter()
        .position(|node| node.id == node_id)
        .ok_or_else(|| format!("研究节点不存在: {node_id}"))
}

fn require_dependencies(task: &ResearchTask, node_index: usize) -> Result<(), String> {
    let missing = missing_dependencies(task, node_index);
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("节点依赖尚未完成: {}", missing.join(", ")))
    }
}

fn missing_dependencies(task: &ResearchTask, node_index: usize) -> Vec<String> {
    let completed: std::collections::HashSet<&str> = task
        .nodes
        .iter()
        .filter(|node| node.status == "completed")
        .map(|node| node.id.as_str())
        .collect();
    task.nodes[node_index]
        .depends_on
        .iter()
        .filter(|dependency| !completed.contains(dependency.as_str()))
        .cloned()
        .collect()
}

fn is_awaiting_decision(node: &ResearchNode) -> bool {
    node.status == "pending_approval"
        || (node.approval_required && node.status != "approved" && node.status != "assigned")
}

fn extend_unique(target: &mut Vec<String>, values: Vec<String>) {
    for value in values {
        let value = value.trim();
        if !value.is_empty() && !target.iter().any(|existing| existing == value) {
            target.push(value.chars().take(2_000).collect());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_workspace(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "galen-pi-kernel-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn node(id: &str, depends_on: Vec<&str>) -> ResearchNode {
        ResearchNode {
            id: id.to_string(),
            index: id.to_string(),
            title: id.to_string(),
            description: None,
            node_type: "research".to_string(),
            status: "pending".to_string(),
            owner: Some("PI".to_string()),
            inputs: Vec::new(),
            outputs: Vec::new(),
            depends_on: depends_on.into_iter().map(str::to_string).collect(),
            tags: Vec::new(),
            risk_level: Some("low".to_string()),
            approval_required: false,
            sub_sessions: Vec::new(),
            result: None,
            evidence: Vec::new(),
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn kernel_enforces_dependencies_and_reveals_next_ready_node() {
        let workspace = temp_workspace("dependencies");
        let task = crate::research_task::create_task(
            &workspace,
            "测试研究".to_string(),
            "验证调度".to_string(),
            vec![node("n1", vec![]), node("n2", vec!["n1"])],
        )
        .unwrap();
        let snapshot = record_project_created(&workspace, task).unwrap();
        assert_eq!(snapshot.ready_node_ids, vec!["n1"]);
        assert!(start_node(
            &workspace,
            &snapshot.task.task_id,
            snapshot.task.revision,
            "n2",
            "start-n2-too-early"
        )
        .unwrap_err()
        .contains("依赖尚未完成"));

        let running = start_node(
            &workspace,
            &snapshot.task.task_id,
            snapshot.task.revision,
            "n1",
            "start-n1",
        )
        .unwrap();
        let completed = complete_node(
            &workspace,
            &running.task.task_id,
            running.task.revision,
            "n1",
            "完成第一节点".to_string(),
            vec!["证据 A".to_string()],
            vec!["output/n1.md".to_string()],
            "complete-n1",
        )
        .unwrap();
        assert_eq!(completed.ready_node_ids, vec!["n2"]);
        assert_eq!(completed.task.nodes[0].evidence, vec!["证据 A"]);
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn retry_with_same_idempotency_key_does_not_repeat_transition() {
        let workspace = temp_workspace("retry");
        let task = crate::research_task::create_task(
            &workspace,
            "测试研究".to_string(),
            "验证幂等".to_string(),
            vec![node("n1", vec![])],
        )
        .unwrap();
        let created = record_project_created(&workspace, task).unwrap();
        let first = start_node(
            &workspace,
            &created.task.task_id,
            created.task.revision,
            "n1",
            "same-start",
        )
        .unwrap();
        let retried = start_node(
            &workspace,
            &created.task.task_id,
            created.task.revision,
            "n1",
            "same-start",
        )
        .unwrap();
        assert_eq!(first.task.revision, retried.task.revision);
        assert_eq!(retried.last_event_sequence, 2);
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn snapshot_recovers_from_workspace_files() {
        let workspace = temp_workspace("restore");
        let task = crate::research_task::create_task(
            &workspace,
            "恢复研究".to_string(),
            "重启后继续".to_string(),
            vec![node("n1", vec![])],
        )
        .unwrap();
        let created = record_project_created(&workspace, task).unwrap();
        start_node(
            &workspace,
            &created.task.task_id,
            created.task.revision,
            "n1",
            "start-before-restart",
        )
        .unwrap();

        let restored = load_active_snapshot(&workspace).unwrap().unwrap();
        assert_eq!(restored.active_node_ids, vec!["n1"]);
        assert_eq!(restored.last_event_sequence, 2);
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn approval_gate_is_visible_and_resolved_by_the_kernel() {
        let workspace = temp_workspace("approval");
        let mut gated = node("n1", vec![]);
        gated.status = "pending_approval".to_string();
        gated.approval_required = true;
        let task = crate::research_task::create_task(
            &workspace,
            "审批研究".to_string(),
            "验证人工决策".to_string(),
            vec![gated],
        )
        .unwrap();
        let created = record_project_created(&workspace, task).unwrap();
        assert_eq!(created.awaiting_decision_node_ids, vec!["n1"]);
        assert!(created.ready_node_ids.is_empty());

        let approved = approve_node(
            &workspace,
            &created.task.task_id,
            created.task.revision,
            "n1",
            "approve-n1",
        )
        .unwrap();
        assert_eq!(approved.ready_node_ids, vec!["n1"]);
        assert!(approved.awaiting_decision_node_ids.is_empty());
        assert_eq!(approved.last_event_sequence, 3);
        let _ = std::fs::remove_dir_all(workspace);
    }
}
