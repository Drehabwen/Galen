use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{artifact::ArtifactRecord, backend::FileEntry};

use super::{lock_mutex, AppState};

#[tauri::command]
pub fn get_workspace_root(state: State<AppState>) -> Result<Option<String>, String> {
    Ok(lock_mutex(&state.backend)?
        .get_workspace_root()
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn set_workspace(state: State<AppState>, path: String) -> Result<(), String> {
    let pb = std::path::PathBuf::from(&path);
    if !pb.exists() || !pb.is_dir() {
        return Err("Path does not exist or is not a directory".into());
    }
    let backend = lock_mutex(&state.backend)?;
    backend.set_workspace_root(Some(pb));
    let mut config = lock_mutex(&state.ws_config)?;
    config.set_workspace(&path);
    Ok(())
}

#[tauri::command]
pub fn list_workspace_files(
    state: State<AppState>,
    path: Option<String>,
) -> Result<Vec<FileEntry>, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend
        .get_workspace_root()
        .ok_or("No workspace selected")?;
    let target = crate::tools::workspace_path::resolve_workspace_path_from_root(
        &root,
        path.as_deref().unwrap_or(""),
    )?;

    let mut entries = Vec::new();
    let dir_iter =
        std::fs::read_dir(&target).map_err(|e| format!("Failed to read directory: {e}"))?;
    for entry in dir_iter {
        let entry = entry.map_err(|e| format!("Failed: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        let meta = entry.metadata().ok();
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let ep = entry.path();
        let rel = ep
            .strip_prefix(&target)
            .unwrap_or(&ep)
            .to_string_lossy()
            .to_string();
        entries.push(FileEntry {
            name,
            path: rel,
            is_dir,
            size,
        });
    }
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
    Ok(entries)
}

#[tauri::command]
pub fn read_workspace_file(state: State<AppState>, path: String) -> Result<String, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend
        .get_workspace_root()
        .ok_or("No workspace selected")?;
    let target = crate::tools::workspace_path::resolve_workspace_path_from_root(&root, &path)?;
    std::fs::read_to_string(&target).map_err(|e| format!("Failed to read file: {e}"))
}

fn read_artifact_bytes_at(root: &Path, path: &str) -> Result<Vec<u8>, String> {
    let target = crate::tools::workspace_path::resolve_workspace_path_from_root(root, path)?;
    std::fs::read(&target).map_err(|e| format!("Failed to read file: {e}"))
}

#[tauri::command]
pub fn read_artifact_bytes(
    state: State<AppState>,
    path: String,
) -> Result<tauri::ipc::Response, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = backend
        .get_workspace_root()
        .ok_or("No workspace selected")?;
    let bytes = read_artifact_bytes_at(&root, &path)?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Cleaned tabular data is always written as a new, timestamped CSV together
/// with its machine-readable quality report. The source file is never touched.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanedDatasetInput {
    pub source_path: String,
    pub csv: String,
    pub report_json: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanedDatasetOutput {
    pub dataset: ArtifactRecord,
    pub quality_report: ArtifactRecord,
}

#[tauri::command]
pub fn write_cleaned_dataset(
    state: State<AppState>,
    input: CleanedDatasetInput,
) -> Result<CleanedDatasetOutput, String> {
    const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;
    if input.csv.trim().is_empty() {
        return Err("清洗结果为空，未生成新数据集。".into());
    }
    if input.csv.len() > MAX_OUTPUT_BYTES || input.report_json.len() > MAX_OUTPUT_BYTES {
        return Err("当前清洗结果过大，请先拆分数据或使用批处理流程。".into());
    }
    serde_json::from_str::<serde_json::Value>(&input.report_json)
        .map_err(|error| format!("数据质量报告不是有效 JSON: {error}"))?;
    let backend = lock_mutex(&state.backend)?;
    let root = backend.get_workspace_root().ok_or("请先选择工作区")?;
    let task_id = crate::research_task::load_active_task(&root)?.map(|task| task.task_id);
    let source_name = std::path::Path::new(&input.source_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("dataset");
    let safe_name: String = source_name
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default();
    let data_relative = format!("output/galen-cleaned-{safe_name}-{stamp}.csv");
    let report_relative = format!("output/galen-quality-{safe_name}-{stamp}.json");
    std::fs::create_dir_all(root.join("output"))
        .map_err(|error| format!("创建数据输出目录失败: {error}"))?;
    std::fs::write(root.join(&data_relative), input.csv)
        .map_err(|error| format!("写入清洗数据失败: {error}"))?;
    std::fs::write(root.join(&report_relative), input.report_json)
        .map_err(|error| format!("写入数据质量报告失败: {error}"))?;
    let dataset = crate::artifact::register_file(&root, &data_relative, task_id.clone(), None)?;
    let quality_report = crate::artifact::register_file(&root, &report_relative, task_id, None)?;
    Ok(CleanedDatasetOutput {
        dataset,
        quality_report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_workspace(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!("galen-cmd-{name}-{stamp}"));
        std::fs::create_dir_all(path.join("output")).unwrap();
        path
    }

    #[test]
    fn read_artifact_bytes_returns_raw_bytes_unchanged() {
        let workspace = temp_workspace("bytes");
        let binary: Vec<u8> = vec![0x25, 0x50, 0x44, 0x46, 0x00, 0xff, 0x10];
        std::fs::write(workspace.join("output/brief.pdf"), &binary).unwrap();

        let bytes = read_artifact_bytes_at(&workspace, "output/brief.pdf").unwrap();
        assert_eq!(bytes, binary);
    }

    #[test]
    fn read_artifact_bytes_rejects_outside_and_missing_paths() {
        let workspace = temp_workspace("bytes-reject");
        std::fs::write(workspace.join("output/ok.md"), "# ok").unwrap();

        assert!(read_artifact_bytes_at(&workspace, "../escape.pdf").is_err());
        assert!(read_artifact_bytes_at(&workspace, "output/missing.pdf").is_err());
        assert!(read_artifact_bytes_at(&workspace, "output/ok.md").is_ok());
    }
}
