//! Durable artifact registry for files delivered by Galen tools.

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

static ARTIFACT_STORE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRecord {
    pub id: String,
    pub path: String,
    pub kind: String,
    pub mime_type: String,
    pub size: u64,
    pub content_hash: String,
    pub task_id: Option<String>,
    pub node_id: Option<String>,
    pub created_at: String,
    pub source: String,
}

pub fn register_file(
    workspace: &Path,
    relative_path: &str,
    task_id: Option<String>,
    node_id: Option<String>,
) -> Result<ArtifactRecord, String> {
    let _guard = lock_store()?;
    let normalized = normalize_relative_path(relative_path)?;
    let absolute = workspace.join(&normalized);
    let bytes = fs::read(&absolute)
        .map_err(|error| format!("读取待登记产物 {} 失败: {error}", absolute.display()))?;
    if bytes.is_empty() {
        return Err(format!("拒绝登记空产物: {normalized}"));
    }
    let content_hash = stable_hash(&bytes);
    let existing = load_artifacts_unlocked(workspace)?;
    if let Some(record) = existing
        .into_iter()
        .rev()
        .find(|record| record.path == normalized && record.content_hash == content_hash)
    {
        return Ok(record);
    }

    let created_at = now_timestamp();
    let id = format!("art-{}-{}", now_millis(), &content_hash[..8]);
    let record = ArtifactRecord {
        id,
        path: normalized.clone(),
        kind: artifact_kind(&normalized).to_string(),
        mime_type: mime_type(&normalized).to_string(),
        size: bytes.len() as u64,
        content_hash,
        task_id,
        node_id,
        created_at,
        source: "agent".to_string(),
    };
    append_record(workspace, &record)?;
    Ok(record)
}

pub fn list_artifacts(workspace: &Path) -> Result<Vec<ArtifactRecord>, String> {
    let _guard = lock_store()?;
    load_artifacts_unlocked(workspace)
}

pub fn link_artifact(
    workspace: &Path,
    artifact_id: &str,
    task_id: &str,
    node_id: &str,
) -> Result<ArtifactRecord, String> {
    let _guard = lock_store()?;
    let path = artifact_ledger(workspace);
    let mut records = load_artifacts_unlocked(workspace)?;
    let record = records
        .iter_mut()
        .find(|record| record.id == artifact_id)
        .ok_or_else(|| format!("找不到待绑定产物: {artifact_id}"))?;
    record.task_id = Some(task_id.to_string());
    record.node_id = Some(node_id.to_string());
    let linked = record.clone();
    let text = records
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("序列化产物登记簿失败: {error}"))?
        .join("\n")
        + "\n";
    let pending = path.with_extension("jsonl.pending");
    fs::write(&pending, text).map_err(|error| format!("写入产物绑定快照失败: {error}"))?;
    // Windows does not allow rename() to replace an existing file. Move the
    // current ledger aside first so the update remains recoverable if the
    // second rename fails.
    let backup = path.with_extension(format!("jsonl.backup-{}", now_millis()));
    if path.exists() {
        fs::rename(&path, &backup).map_err(|error| format!("备份产物登记簿失败: {error}"))?;
    }
    if let Err(error) = fs::rename(&pending, &path) {
        if backup.exists() {
            let _ = fs::rename(&backup, &path);
        }
        return Err(format!("提交产物绑定快照失败: {error}"));
    }
    if backup.exists() {
        let _ = fs::remove_file(backup);
    }
    Ok(linked)
}

fn append_record(workspace: &Path, record: &ArtifactRecord) -> Result<(), String> {
    let path = artifact_ledger(workspace);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建产物登记目录失败: {error}"))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("打开产物登记簿 {} 失败: {error}", path.display()))?;
    let line = serde_json::to_string(record).map_err(|error| format!("序列化产物失败: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("写入产物登记簿失败: {error}"))
}

fn load_artifacts_unlocked(workspace: &Path) -> Result<Vec<ArtifactRecord>, String> {
    let path = artifact_ledger(workspace);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("读取产物登记簿 {} 失败: {error}", path.display()))?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).map_err(|error| format!("产物登记记录无效: {error}"))
        })
        .collect()
}

fn artifact_ledger(workspace: &Path) -> PathBuf {
    workspace.join(".galen").join("artifacts.jsonl")
}

fn normalize_relative_path(path: &str) -> Result<String, String> {
    let candidate = Path::new(path);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(format!("产物路径必须位于工作区内: {path}"));
    }
    let normalized = path.replace('\\', "/").trim_start_matches("./").to_string();
    if normalized.is_empty() {
        return Err("产物路径不能为空".to_string());
    }
    Ok(normalized)
}

fn artifact_kind(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "md" | "txt" => "document",
        "csv" | "tsv" | "xlsx" | "xls" => "data",
        "png" | "jpg" | "jpeg" | "svg" | "webp" => "figure",
        "pdf" | "docx" | "typ" => "report",
        "py" | "r" | "js" | "ts" | "rs" => "code",
        _ => "file",
    }
}

fn mime_type(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "md" => "text/markdown",
        "txt" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn stable_hash(bytes: &[u8]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn now_timestamp() -> String {
    now_millis().to_string()
}

fn lock_store() -> Result<MutexGuard<'static, ()>, String> {
    ARTIFACT_STORE_LOCK
        .lock()
        .map_err(|error| format!("产物登记锁失败: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("galen-artifact-{name}-{}", now_millis()));
        fs::create_dir_all(path.join("output")).unwrap();
        path
    }

    #[test]
    fn registers_and_deduplicates_a_non_empty_file() {
        let workspace = temp_workspace("register");
        fs::write(workspace.join("output/result.md"), "# result").unwrap();
        let first = register_file(&workspace, "output/result.md", None, None).unwrap();
        let second = register_file(&workspace, "output/result.md", None, None).unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(list_artifacts(&workspace).unwrap().len(), 1);
        assert_eq!(first.mime_type, "text/markdown");
    }

    #[test]
    fn rejects_empty_and_outside_artifacts() {
        let workspace = temp_workspace("reject");
        fs::write(workspace.join("output/empty.md"), "").unwrap();
        assert!(register_file(&workspace, "output/empty.md", None, None).is_err());
        assert!(register_file(&workspace, "../escape.md", None, None).is_err());
    }

    #[test]
    fn links_an_existing_artifact_without_losing_the_ledger() {
        let workspace = temp_workspace("link");
        fs::write(workspace.join("output/result.md"), "# result").unwrap();
        let record = register_file(&workspace, "output/result.md", None, None).unwrap();
        let linked = link_artifact(&workspace, &record.id, "task-1", "node-1").unwrap();
        assert_eq!(linked.task_id.as_deref(), Some("task-1"));
        assert_eq!(linked.node_id.as_deref(), Some("node-1"));
        let saved = list_artifacts(&workspace).unwrap();
        assert_eq!(saved, vec![linked]);
    }
}
