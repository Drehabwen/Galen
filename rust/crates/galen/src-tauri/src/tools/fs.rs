use super::workspace_path::resolve_workspace_path;
use super::{GalenTool, ToolContext};
use crate::backend::{ChatEvent, FileEntry};
use api::ToolDefinition;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;

// ── CreateDirectory ──
pub struct CreateDirectory;
#[async_trait]
impl GalenTool for CreateDirectory {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "create_directory".into(),
            description: Some("Create a directory in the workspace.".into()),
            input_schema: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
        }
    }
    fn is_write(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let path = input["path"].as_str().ok_or("Missing 'path'")?;
        let target = resolve_workspace_path(&ctx.workspace_root, path)?;
        fs::create_dir_all(&target).map_err(|e| format!("{e}"))?;
        Ok(format!("Created: {}", target.display()))
    }
}

// ── WriteFile ──
pub struct WriteFile;
#[async_trait]
impl GalenTool for WriteFile {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write_file".into(),
            description: Some("Write content to a file in the workspace.".into()),
            input_schema: json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]}),
        }
    }
    fn is_write(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let path = input["path"].as_str().ok_or("Missing 'path'")?;
        let content = input["content"].as_str().ok_or("Missing 'content'")?;
        let target = resolve_workspace_path(&ctx.workspace_root, path)?;
        let target_str = target.to_string_lossy().to_string();
        let content_owned = content.to_string();
        let result =
            tokio::task::spawn_blocking(move || runtime::write_file(&target_str, &content_owned))
                .await
                .map_err(|e| format!("{e}"))?
                .map_err(|e| format!("{e}"))?;
        Ok(format!("Wrote to {}", result.file_path))
    }
}

// ── ReadFile ──
pub struct ReadFile;
#[async_trait]
impl GalenTool for ReadFile {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".into(),
            description: Some("Read the contents of a file from the workspace.".into()),
            input_schema: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
        }
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let path = input["path"].as_str().ok_or("Missing 'path'")?.to_string();
        let target = resolve_workspace_path(&ctx.workspace_root, &path)?;
        let target_str = target.to_string_lossy().to_string();
        let path_clone = path.clone();
        let result =
            tokio::task::spawn_blocking(move || runtime::read_file(&target_str, None, None))
                .await
                .map_err(|e| format!("{e}"))?
                .map_err(|e| format!("{e}"))?;
        let content = result.file.content.clone();
        let num_lines = result.file.num_lines;
        ctx.send_event(ChatEvent::WorkspaceFileContent {
            path: path_clone,
            content: content.clone(),
        });
        // 关键：文件内容必须随工具结果返回给模型（之前只返回行数，
        // 模型看不到内容会反复重试同一工具，导致行为失控）。
        let mut out = format!("Read {num_lines} lines from {path}:\n");
        out.push_str(&content);
        Ok(out)
    }
}

// ── ListFiles ──
pub struct ListFiles;
#[async_trait]
impl GalenTool for ListFiles {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_files".into(),
            description: Some("List files and directories in the workspace.".into()),
            input_schema: json!({"type":"object","properties":{"path":{"type":"string"}},"required":[]}),
        }
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let sub = input["path"].as_str().unwrap_or("").to_string();
        let target = resolve_workspace_path(&ctx.workspace_root, &sub)?;
        let mut entries: Vec<FileEntry> = Vec::new();
        for entry in fs::read_dir(&target).map_err(|e| format!("{e}"))? {
            let entry = entry.map_err(|e| format!("{e}"))?;
            let name = entry.file_name().to_string_lossy().to_string();
            let meta = entry.metadata().ok();
            let ep = entry.path();
            let rel = ep
                .strip_prefix(&target)
                .unwrap_or(&ep)
                .to_string_lossy()
                .to_string();
            entries.push(FileEntry {
                name,
                path: if sub.is_empty() {
                    rel
                } else {
                    format!("{}/{}", sub, rel)
                },
                is_dir: meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
            });
        }
        entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
        ctx.send_event(ChatEvent::WorkspaceFileList(entries.clone()));
        let listing: Vec<String> = entries
            .iter()
            .map(|e| {
                format!(
                    "{} {} ({} bytes)",
                    if e.is_dir { "[DIR]" } else { "[FILE]" },
                    e.path,
                    e.size
                )
            })
            .collect();
        Ok(if listing.is_empty() {
            format!("Empty: {}", if sub.is_empty() { "root" } else { &sub })
        } else {
            format!("Contents:\n{}", listing.join("\n"))
        })
    }
}

// ── SavePaper ──
pub struct SavePaper;
#[async_trait]
impl GalenTool for SavePaper {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "save_paper".into(),
            description: Some("Save paper metadata as JSON to workspace papers/ directory.".into()),
            input_schema: json!({"type":"object","properties":{"pmid":{"type":"string"}},"required":["pmid"]}),
        }
    }
    fn is_write(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let pmid = input["pmid"].as_str().ok_or("Missing 'pmid'")?;
        let paper = ctx
            .medical
            .fetch_article(pmid)
            .await
            .map_err(|e| format!("{e}"))?
            .ok_or_else(|| format!("No article for PMID: {pmid}"))?;
        let dir = resolve_workspace_path(&ctx.workspace_root, "papers")?;
        fs::create_dir_all(&dir).map_err(|e| format!("{e}"))?;
        let target = dir.join(format!("{pmid}.json"));
        let json = serde_json::to_string_pretty(&paper).map_err(|e| format!("{e}"))?;
        fs::write(&target, json).map_err(|e| format!("{e}"))?;
        Ok(format!("Saved: papers/{}.json", pmid))
    }
}

// ── DeleteFile ──
pub struct DeleteFile;
#[async_trait]
impl GalenTool for DeleteFile {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "delete_file".into(),
            description: Some("Delete a file from the workspace. Irreversible.".into()),
            input_schema: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
        }
    }
    fn is_write(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let target = resolve_workspace_path(
            &ctx.workspace_root,
            input["path"].as_str().ok_or("Missing 'path'")?,
        )?;
        if fs::metadata(&target).map_err(|e| format!("{e}"))?.is_dir() {
            return Err("Use delete_directory for directories.".into());
        }
        fs::remove_file(&target).map_err(|e| format!("{e}"))?;
        Ok(format!("Deleted: {}", target.display()))
    }
}

// ── DeleteDirectory ──
pub struct DeleteDirectory;
#[async_trait]
impl GalenTool for DeleteDirectory {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "delete_directory".into(),
            description: Some("Recursively delete a directory. Irreversible.".into()),
            input_schema: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
        }
    }
    fn is_write(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let target = resolve_workspace_path(
            &ctx.workspace_root,
            input["path"].as_str().ok_or("Missing 'path'")?,
        )?;
        if !fs::metadata(&target).map_err(|e| format!("{e}"))?.is_dir() {
            return Err("Not a directory. Use delete_file.".into());
        }
        fs::remove_dir_all(&target).map_err(|e| format!("{e}"))?;
        Ok(format!("Deleted: {}", target.display()))
    }
}

// ── MoveFile ──
pub struct MoveFile;
#[async_trait]
impl GalenTool for MoveFile {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "move_file".into(),
            description: Some("Move or rename a file/directory within the workspace.".into()),
            input_schema: json!({"type":"object","properties":{"from":{"type":"string"},"to":{"type":"string"}},"required":["from","to"]}),
        }
    }
    fn is_write(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let from = resolve_workspace_path(
            &ctx.workspace_root,
            input["from"].as_str().ok_or("Missing 'from'")?,
        )?;
        let to = resolve_workspace_path(
            &ctx.workspace_root,
            input["to"].as_str().ok_or("Missing 'to'")?,
        )?;
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{e}"))?;
        }
        fs::rename(&from, &to).map_err(|e| format!("{e}"))?;
        Ok(format!("Moved {} -> {}", from.display(), to.display()))
    }
}
