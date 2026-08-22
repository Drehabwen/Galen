//! Durable, host-authoritative state for a Galen research task.
//!
//! The frontend may render and edit a task, but the canonical snapshot lives at
//! `<workspace>/.galen/tasks/<task-id>/task.json`.  This keeps the research loop
//! recoverable when the webview or model session disappears.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: u32 = 2;
static TASK_STORE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResearchNode {
    pub id: String,
    pub index: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, rename = "type")]
    pub node_type: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<String>,
    #[serde(default)]
    pub approval_required: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sub_sessions: Vec<ResearchNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
    /// Preserve forward-compatible frontend fields during a read/write cycle.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ResearchTaskStatus {
    Draft,
    Ready,
    Running,
    Verifying,
    Deliverable,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResearchTask {
    pub schema_version: u32,
    #[serde(default = "initial_revision")]
    pub revision: u64,
    pub task_id: String,
    pub title: String,
    pub goal: String,
    pub status: ResearchTaskStatus,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub nodes: Vec<ResearchNode>,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
    #[serde(default)]
    pub artifact_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActiveTaskPointer {
    task_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActiveTaskPointerRef<'a> {
    task_id: &'a str,
}

pub fn create_task(
    workspace: &Path,
    title: String,
    goal: String,
    nodes: Vec<ResearchNode>,
) -> Result<ResearchTask, String> {
    let _guard = lock_task_store()?;
    create_task_unlocked(workspace, title, goal, nodes)
}

fn create_task_unlocked(
    workspace: &Path,
    title: String,
    goal: String,
    nodes: Vec<ResearchNode>,
) -> Result<ResearchTask, String> {
    let now = now_id();
    let title = clean_text(title, "未命名研究任务");
    let goal = clean_text(goal, &title);
    let task_id = format!("{}-{}", slug(&title), now);
    let timestamp = now_timestamp();
    let status = derive_status(&nodes);
    let task = ResearchTask {
        schema_version: SCHEMA_VERSION,
        revision: initial_revision(),
        task_id,
        title,
        goal,
        status,
        created_at: timestamp.clone(),
        updated_at: timestamp,
        nodes,
        evidence_ids: Vec::new(),
        artifact_ids: Vec::new(),
    };
    save_task(workspace, &task)?;
    save_active_pointer(workspace, &task.task_id)?;
    Ok(task)
}

pub fn load_active_task(workspace: &Path) -> Result<Option<ResearchTask>, String> {
    let _guard = lock_task_store()?;
    load_active_task_unlocked(workspace)
}

fn load_active_task_unlocked(workspace: &Path) -> Result<Option<ResearchTask>, String> {
    let pointer_path = galen_dir(workspace).join("active-task.json");
    if !pointer_path.exists() {
        return Ok(None);
    }
    let text =
        std::fs::read_to_string(&pointer_path).map_err(|e| format!("读取当前研究任务失败: {e}"))?;
    let pointer: ActiveTaskPointer =
        serde_json::from_str(&text).map_err(|e| format!("当前研究任务指针无效: {e}"))?;
    Ok(Some(load_task(workspace, &pointer.task_id)?))
}

/// One-time compatibility bridge for the previous frontend-owned `plan.json`.
/// The legacy file is deliberately retained as a recoverable source.
pub fn load_or_migrate_active_task(workspace: &Path) -> Result<Option<ResearchTask>, String> {
    let _guard = lock_task_store()?;
    load_or_migrate_active_task_unlocked(workspace)
}

fn load_or_migrate_active_task_unlocked(workspace: &Path) -> Result<Option<ResearchTask>, String> {
    if let Some(task) = load_active_task_unlocked(workspace)? {
        return Ok(Some(task));
    }
    let legacy_path = workspace.join("plan.json");
    if !legacy_path.exists() {
        return Ok(None);
    }
    let text =
        std::fs::read_to_string(&legacy_path).map_err(|e| format!("读取旧 plan.json 失败: {e}"))?;
    let nodes: Vec<ResearchNode> =
        serde_json::from_str(&text).map_err(|e| format!("旧 plan.json 无法迁移: {e}"))?;
    if nodes.is_empty() {
        return Ok(None);
    }
    create_task_unlocked(
        workspace,
        "恢复的研究任务".to_string(),
        "从旧版 plan.json 恢复".to_string(),
        nodes,
    )
    .map(Some)
}

pub fn replace_nodes(
    workspace: &Path,
    task_id: &str,
    expected_revision: u64,
    nodes: Vec<ResearchNode>,
) -> Result<ResearchTask, String> {
    let _guard = lock_task_store()?;
    let mut task = load_task(workspace, task_id)?;
    if task.revision != expected_revision {
        return Err(format!(
            "RESEARCH_TASK_CONFLICT: 任务版本已变化（期望 {expected_revision}，当前 {}），请刷新后重试",
            task.revision
        ));
    }
    task.status = derive_status(&nodes);
    task.nodes = nodes;
    task.schema_version = SCHEMA_VERSION;
    task.revision = task.revision.saturating_add(1);
    task.updated_at = now_timestamp();
    save_task(workspace, &task)?;
    Ok(task)
}

/// Return the durable directory for the active task, migrating a legacy plan
/// first when necessary.
pub fn active_task_dir(workspace: &Path) -> Result<Option<PathBuf>, String> {
    let _guard = lock_task_store()?;
    let Some(task) = load_or_migrate_active_task_unlocked(workspace)? else {
        return Ok(None);
    };
    Ok(task_path(workspace, &task.task_id)
        .parent()
        .map(Path::to_path_buf))
}

/// Attach evidence identifiers to the canonical task snapshot. Existing IDs
/// are retained in insertion order and never duplicated.
pub fn attach_evidence_ids(
    workspace: &Path,
    evidence_ids: &[String],
) -> Result<ResearchTask, String> {
    let _guard = lock_task_store()?;
    let mut task = load_or_migrate_active_task_unlocked(workspace)?
        .ok_or("当前工作区没有活动研究任务，无法登记证据")?;
    let mut changed = false;
    for evidence_id in evidence_ids {
        if !task.evidence_ids.contains(evidence_id) {
            task.evidence_ids.push(evidence_id.clone());
            changed = true;
        }
    }
    if changed {
        task.schema_version = SCHEMA_VERSION;
        task.revision = task.revision.saturating_add(1);
        task.updated_at = now_timestamp();
        save_task(workspace, &task)?;
    }
    Ok(task)
}

/// Bind a delivered artifact to the active task and one concrete node. When a
/// chat writes a file without first creating a plan, create a one-node direct
/// delivery task so the artifact is never orphaned from the research canvas.
pub fn attach_artifact(
    workspace: &Path,
    artifact_id: &str,
    artifact_path: &str,
    preferred_node_id: Option<&str>,
) -> Result<ResearchTask, String> {
    let _guard = lock_task_store()?;
    let mut task = match load_or_migrate_active_task_unlocked(workspace)? {
        Some(task) => task,
        None => create_task_unlocked(
            workspace,
            artifact_title(artifact_path),
            format!("交付工作区产物 {artifact_path}"),
            vec![ResearchNode {
                id: "delivery".to_string(),
                index: "01".to_string(),
                title: "直接交付".to_string(),
                description: Some("由 Galen 自动创建并登记的工作区产物".to_string()),
                node_type: "delivery".to_string(),
                status: "pending".to_string(),
                owner: Some("Galen".to_string()),
                inputs: Vec::new(),
                outputs: Vec::new(),
                depends_on: Vec::new(),
                tags: vec!["auto-created".to_string()],
                risk_level: Some("low".to_string()),
                approval_required: false,
                sub_sessions: Vec::new(),
                result: None,
                evidence: Vec::new(),
                extra: BTreeMap::new(),
            }],
        )?,
    };

    if !task.artifact_ids.iter().any(|id| id == artifact_id) {
        task.artifact_ids.push(artifact_id.to_string());
    }
    let node_index = preferred_node_id
        .and_then(|id| task.nodes.iter().position(|node| node.id == id))
        .or_else(|| task.nodes.iter().position(|node| node.status == "running"))
        .or_else(|| {
            task.nodes.iter().position(|node| {
                node.status != "completed"
                    && node.depends_on.iter().all(|dependency| {
                        task.nodes.iter().any(|candidate| {
                            candidate.id == *dependency && candidate.status == "completed"
                        })
                    })
            })
        })
        .or_else(|| task.nodes.len().checked_sub(1))
        .ok_or("研究任务没有可绑定产物的节点")?;
    let node = &mut task.nodes[node_index];
    if !node.outputs.iter().any(|path| path == artifact_path) {
        node.outputs.push(artifact_path.to_string());
    }
    node.status = "completed".to_string();
    node.result = Some(format!("已生成产物 {artifact_path}"));

    task.status = if task.nodes.iter().all(|node| node.status == "completed") {
        ResearchTaskStatus::Deliverable
    } else {
        derive_status(&task.nodes)
    };
    task.schema_version = SCHEMA_VERSION;
    task.revision = task.revision.saturating_add(1);
    task.updated_at = now_timestamp();
    save_task(workspace, &task)?;
    Ok(task)
}

fn artifact_title(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("研究交付")
        .to_string()
}

fn lock_task_store() -> Result<MutexGuard<'static, ()>, String> {
    TASK_STORE_LOCK
        .lock()
        .map_err(|error| format!("研究任务存储锁失败: {error}"))
}

fn initial_revision() -> u64 {
    1
}

fn derive_status(nodes: &[ResearchNode]) -> ResearchTaskStatus {
    if nodes.is_empty() {
        return ResearchTaskStatus::Draft;
    }
    if nodes.iter().any(|node| node.status == "blocked") {
        return ResearchTaskStatus::Blocked;
    }
    if nodes.iter().all(|node| node.status == "completed") {
        return ResearchTaskStatus::Verifying;
    }
    if nodes.iter().any(|node| node.status == "running") {
        return ResearchTaskStatus::Running;
    }
    ResearchTaskStatus::Ready
}

fn load_task(workspace: &Path, task_id: &str) -> Result<ResearchTask, String> {
    validate_task_id(task_id)?;
    let path = task_path(workspace, task_id);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("读取研究任务 {} 失败: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("研究任务数据无效: {e}"))
}

fn save_task(workspace: &Path, task: &ResearchTask) -> Result<(), String> {
    validate_task_id(&task.task_id)?;
    let path = task_path(workspace, &task.task_id);
    let json =
        serde_json::to_string_pretty(task).map_err(|e| format!("序列化研究任务失败: {e}"))?;
    write_json(&path, &json)
}

fn save_active_pointer(workspace: &Path, task_id: &str) -> Result<(), String> {
    validate_task_id(task_id)?;
    let pointer = ActiveTaskPointerRef { task_id };
    let json = serde_json::to_string_pretty(&pointer)
        .map_err(|e| format!("序列化当前任务指针失败: {e}"))?;
    write_json(&galen_dir(workspace).join("active-task.json"), &json)
}

fn write_json(path: &Path, content: &str) -> Result<(), String> {
    let parent = path.parent().ok_or("研究任务路径没有父目录")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("创建研究任务目录失败: {e}"))?;
    let pending = path.with_extension("json.pending");
    std::fs::write(&pending, content).map_err(|e| format!("写入研究任务临时文件失败: {e}"))?;

    if path.exists() {
        let backup = path.with_extension("json.backup");
        let _ = std::fs::remove_file(&backup);
        std::fs::rename(path, &backup).map_err(|e| format!("备份旧研究任务失败: {e}"))?;
        if let Err(error) = std::fs::rename(&pending, path) {
            let _ = std::fs::rename(&backup, path);
            return Err(format!("替换研究任务失败: {error}"));
        }
        let _ = std::fs::remove_file(backup);
    } else {
        std::fs::rename(&pending, path).map_err(|e| format!("保存研究任务失败: {e}"))?;
    }
    Ok(())
}

fn galen_dir(workspace: &Path) -> PathBuf {
    workspace.join(".galen")
}

fn task_path(workspace: &Path, task_id: &str) -> PathBuf {
    galen_dir(workspace)
        .join("tasks")
        .join(task_id)
        .join("task.json")
}

fn validate_task_id(task_id: &str) -> Result<(), String> {
    if task_id.is_empty()
        || !task_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err("研究任务 ID 无效".to_string());
    }
    Ok(())
}

fn clean_text(value: String, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.chars().take(240).collect()
    }
}

fn slug(value: &str) -> String {
    let slug: String = value
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() {
                Some(ch.to_ascii_lowercase())
            } else if ch == '-' || ch == '_' || ch.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect();
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "research".to_string()
    } else {
        slug.chars().take(48).collect()
    }
}

fn now_id() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn now_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_workspace(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "galen-task-test-{}-{}-{}",
            std::process::id(),
            tag,
            now_id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn node(id: &str, status: &str) -> ResearchNode {
        ResearchNode {
            id: id.to_string(),
            index: "01".to_string(),
            title: "数据质检".to_string(),
            description: None,
            node_type: "data".to_string(),
            status: status.to_string(),
            owner: None,
            inputs: Vec::new(),
            outputs: vec!["output/qc.md".to_string()],
            depends_on: Vec::new(),
            tags: Vec::new(),
            risk_level: None,
            approval_required: false,
            sub_sessions: Vec::new(),
            result: None,
            evidence: Vec::new(),
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn task_round_trip_uses_workspace_store() {
        let workspace = temp_workspace("round-trip");
        let created = create_task(
            &workspace,
            "脊柱侧弯筛查".to_string(),
            "分析筛查数据".to_string(),
            vec![node("s01", "pending")],
        )
        .unwrap();
        let loaded = load_active_task(&workspace).unwrap().unwrap();
        assert_eq!(loaded.task_id, created.task_id);
        assert_eq!(loaded.status, ResearchTaskStatus::Ready);
        assert!(workspace
            .join(".galen/tasks")
            .join(&created.task_id)
            .join("task.json")
            .exists());
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn replacing_nodes_derives_host_status() {
        let workspace = temp_workspace("status");
        let created = create_task(
            &workspace,
            "研究".to_string(),
            "目标".to_string(),
            vec![node("s01", "pending")],
        )
        .unwrap();
        let running = replace_nodes(
            &workspace,
            &created.task_id,
            created.revision,
            vec![node("s01", "running")],
        )
        .unwrap();
        assert_eq!(running.status, ResearchTaskStatus::Running);
        let verifying = replace_nodes(
            &workspace,
            &created.task_id,
            running.revision,
            vec![node("s01", "completed")],
        )
        .unwrap();
        assert_eq!(verifying.status, ResearchTaskStatus::Verifying);
        assert_eq!(created.revision, 1);
        assert_eq!(running.revision, 2);
        assert_eq!(verifying.revision, 3);
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn rejects_stale_revision_without_overwriting_current_task() {
        let workspace = temp_workspace("conflict");
        let created = create_task(
            &workspace,
            "并发研究".to_string(),
            "验证版本冲突".to_string(),
            vec![node("s01", "pending")],
        )
        .unwrap();
        let running = replace_nodes(
            &workspace,
            &created.task_id,
            created.revision,
            vec![node("s01", "running")],
        )
        .unwrap();

        let error = replace_nodes(
            &workspace,
            &created.task_id,
            created.revision,
            vec![node("s01", "completed")],
        )
        .unwrap_err();

        assert!(error.contains("RESEARCH_TASK_CONFLICT"));
        let current = load_active_task(&workspace).unwrap().unwrap();
        assert_eq!(current.revision, running.revision);
        assert_eq!(current.nodes[0].status, "running");
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn migrates_legacy_plan_without_deleting_it() {
        let workspace = temp_workspace("migration");
        let legacy = serde_json::to_string_pretty(&vec![node("s01", "completed")]).unwrap();
        std::fs::write(workspace.join("plan.json"), legacy).unwrap();
        let task = load_or_migrate_active_task(&workspace).unwrap().unwrap();
        assert_eq!(task.nodes.len(), 1);
        assert_eq!(task.status, ResearchTaskStatus::Verifying);
        assert!(workspace.join("plan.json").exists());
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn loads_schema_v1_task_without_revision() {
        let workspace = temp_workspace("schema-v1");
        let created = create_task(
            &workspace,
            "旧任务".to_string(),
            "验证 revision 默认值".to_string(),
            vec![node("s01", "pending")],
        )
        .unwrap();
        let path = task_path(&workspace, &created.task_id);
        let mut value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        value.as_object_mut().unwrap().remove("revision");
        value["schemaVersion"] = serde_json::json!(1);
        std::fs::write(&path, serde_json::to_string_pretty(&value).unwrap()).unwrap();

        let loaded = load_active_task(&workspace).unwrap().unwrap();
        assert_eq!(loaded.schema_version, 1);
        assert_eq!(loaded.revision, 1);
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn rejects_path_like_task_ids() {
        let workspace = temp_workspace("traversal");
        let error = replace_nodes(&workspace, "../outside", 1, Vec::new()).unwrap_err();
        assert!(error.contains("ID 无效"));
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn artifact_creates_a_direct_delivery_task_when_no_plan_exists() {
        let workspace = temp_workspace("direct-artifact");
        let task = attach_artifact(&workspace, "art-1", "output/result.md", None).unwrap();
        assert_eq!(task.status, ResearchTaskStatus::Deliverable);
        assert_eq!(task.nodes.len(), 1);
        assert_eq!(task.nodes[0].outputs, vec!["output/result.md"]);
        assert_eq!(task.artifact_ids, vec!["art-1"]);
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn artifact_completes_the_preferred_plan_node() {
        let workspace = temp_workspace("planned-artifact");
        let mut second = node("s02", "pending");
        second.depends_on = vec!["s01".to_string()];
        let created = create_task(
            &workspace,
            "研究计划".to_string(),
            "生成计划产物".to_string(),
            vec![node("s01", "pending"), second],
        )
        .unwrap();
        let task = attach_artifact(&workspace, "art-2", "output/brief.md", Some("s01")).unwrap();
        assert_eq!(task.task_id, created.task_id);
        assert_eq!(task.nodes[0].status, "completed");
        assert_eq!(task.nodes[1].status, "pending");
        assert_eq!(task.status, ResearchTaskStatus::Ready);
        let _ = std::fs::remove_dir_all(workspace);
    }
}
