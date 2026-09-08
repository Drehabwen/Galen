//! Append-only audit trail for PI orchestration decisions.
//!
//! The research task JSON remains the fast materialized view. Every PI state
//! transition is also recorded here so the workbench can explain, resume and
//! evaluate a long-running research project without reconstructing it from chat.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

static EVENT_STORE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PiEventKind {
    ProjectCreated,
    PlanUpdated,
    NodeAssigned,
    NodeStarted,
    NodeCompleted,
    NodeBlocked,
    HumanDecisionRequested,
    HumanDecisionResolved,
    EvidenceAttached,
    ArtifactAttached,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PiEvent {
    pub sequence: u64,
    pub event_id: String,
    pub task_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    pub kind: PiEventKind,
    pub created_at: String,
    pub idempotency_key: String,
    #[serde(default)]
    pub payload: Value,
}

pub fn append_event(
    workspace: &Path,
    task_id: &str,
    node_id: Option<&str>,
    kind: PiEventKind,
    idempotency_key: &str,
    payload: Value,
) -> Result<PiEvent, String> {
    validate_identifier(task_id, "研究任务")?;
    validate_idempotency_key(idempotency_key)?;
    let _guard = lock_event_store()?;
    let events = read_events_unlocked(workspace, task_id)?;
    if let Some(existing) = events
        .iter()
        .find(|event| event.idempotency_key == idempotency_key)
    {
        return Ok(existing.clone());
    }

    let sequence = events
        .last()
        .map_or(1, |event| event.sequence.saturating_add(1));
    let event = PiEvent {
        sequence,
        event_id: uuid::Uuid::new_v4().to_string(),
        task_id: task_id.to_string(),
        node_id: node_id.map(str::to_string),
        kind,
        created_at: now_timestamp(),
        idempotency_key: idempotency_key.to_string(),
        payload,
    };
    let path = event_path(workspace, task_id);
    let parent = path.parent().ok_or("PI 事件路径没有父目录")?;
    std::fs::create_dir_all(parent).map_err(|error| format!("创建 PI 事件目录失败: {error}"))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("打开 PI 事件日志失败: {error}"))?;
    let line =
        serde_json::to_string(&event).map_err(|error| format!("序列化 PI 事件失败: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("写入 PI 事件日志失败: {error}"))?;
    file.sync_data()
        .map_err(|error| format!("持久化 PI 事件日志失败: {error}"))?;
    Ok(event)
}

pub fn read_events(workspace: &Path, task_id: &str) -> Result<Vec<PiEvent>, String> {
    validate_identifier(task_id, "研究任务")?;
    let _guard = lock_event_store()?;
    read_events_unlocked(workspace, task_id)
}

fn read_events_unlocked(workspace: &Path, task_id: &str) -> Result<Vec<PiEvent>, String> {
    let path = event_path(workspace, task_id);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text =
        std::fs::read_to_string(&path).map_err(|error| format!("读取 PI 事件日志失败: {error}"))?;
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str(line)
                .map_err(|error| format!("PI 事件日志第 {} 行无效: {error}", index + 1))
        })
        .collect()
}

fn event_path(workspace: &Path, task_id: &str) -> PathBuf {
    workspace
        .join(".galen")
        .join("tasks")
        .join(task_id)
        .join("pi-events.jsonl")
}

fn validate_identifier(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        return Err(format!("{label} ID 无效"));
    }
    Ok(())
}

fn validate_idempotency_key(value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 200 || value.contains(['\n', '\r']) {
        return Err("PI 操作幂等键无效".to_string());
    }
    Ok(())
}

fn lock_event_store() -> Result<MutexGuard<'static, ()>, String> {
    EVENT_STORE_LOCK
        .lock()
        .map_err(|error| format!("PI 事件存储锁失败: {error}"))
}

fn now_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_workspace(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "galen-pi-event-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn event_log_is_ordered_and_idempotent() {
        let workspace = temp_workspace("idempotent");
        let first = append_event(
            &workspace,
            "task-1",
            Some("node-1"),
            PiEventKind::NodeStarted,
            "start-node-1",
            serde_json::json!({}),
        )
        .unwrap();
        let duplicate = append_event(
            &workspace,
            "task-1",
            Some("node-1"),
            PiEventKind::NodeStarted,
            "start-node-1",
            serde_json::json!({"ignored": true}),
        )
        .unwrap();
        let second = append_event(
            &workspace,
            "task-1",
            Some("node-1"),
            PiEventKind::NodeCompleted,
            "complete-node-1",
            serde_json::json!({}),
        )
        .unwrap();

        assert_eq!(first.event_id, duplicate.event_id);
        assert_eq!(second.sequence, 2);
        assert_eq!(read_events(&workspace, "task-1").unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(workspace);
    }
}
