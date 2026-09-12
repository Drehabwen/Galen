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
            input_schema: json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"},"node_id":{"type":"string","description":"Optional research node id to receive this artifact."}},"required":["path","content"]}),
        }
    }
    fn is_write(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let path = input["path"].as_str().ok_or("Missing 'path'")?;
        let content = input["content"].as_str().ok_or("Missing 'content'")?;
        let preferred_node_id = input["node_id"].as_str().map(str::to_string);
        let target = resolve_workspace_path(&ctx.workspace_root, path)?;
        let workspace = ctx
            .workspace_root
            .lock()
            .map_err(|error| format!("Workspace lock error: {error}"))?
            .clone()
            .ok_or("请先选择工作区")?;
        let target_str = target.to_string_lossy().to_string();
        let content_owned = content.to_string();
        let result =
            tokio::task::spawn_blocking(move || runtime::write_file(&target_str, &content_owned))
                .await
                .map_err(|e| format!("{e}"))?
                .map_err(|e| format!("{e}"))?;
        let active_task_id =
            crate::research_task::load_active_task(&workspace)?.map(|task| task.task_id);
        let artifact = crate::artifact::register_file(
            &workspace,
            path,
            active_task_id,
            preferred_node_id.clone(),
        )?;
        let task = crate::research_task::attach_artifact(
            &workspace,
            &artifact.id,
            &artifact.path,
            preferred_node_id.as_deref(),
        )?;
        let node_id = task
            .nodes
            .iter()
            .find(|node| node.outputs.iter().any(|output| output == &artifact.path))
            .map(|node| node.id.clone())
            .ok_or("产物已写入，但未能绑定研究节点")?;
        let artifact =
            crate::artifact::link_artifact(&workspace, &artifact.id, &task.task_id, &node_id)?;
        ctx.send_event(ChatEvent::ResearchTaskUpdated(task.clone()));
        ctx.send_event(ChatEvent::ArtifactCreated(artifact.clone()));
        Ok(json!({
            "status": "delivered",
            "file_path": result.file_path,
            "artifact": artifact,
            "research_task": task,
        })
        .to_string())
    }
}

// ── AppendFile ──
// Large manuscripts should not need a shell workaround merely because one
// model tool payload cannot hold an entire source file. The agent writes the
// first section with `write_file`, then appends bounded sections with this
// first-class workspace tool; every revision remains an Artifact update.
pub struct AppendFile;
#[async_trait]
impl GalenTool for AppendFile {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "append_file".into(),
            description: Some(
                "Append content to a workspace file and register the updated artifact. Use after write_file for long documents."
                    .into(),
            ),
            input_schema: json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"},"node_id":{"type":"string","description":"Optional research node receiving this artifact."}},"required":["path","content"]}),
        }
    }
    fn is_write(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let path = input["path"].as_str().ok_or("Missing 'path'")?;
        let content = input["content"].as_str().ok_or("Missing 'content'")?;
        let preferred_node_id = input["node_id"].as_str().map(str::to_string);
        let target = resolve_workspace_path(&ctx.workspace_root, path)?;
        let workspace = ctx
            .workspace_root
            .lock()
            .map_err(|error| format!("Workspace lock error: {error}"))?
            .clone()
            .ok_or("请先选择工作区")?;
        let target_for_write = target.clone();
        let content_owned = content.to_string();
        tokio::task::spawn_blocking(move || {
            if let Some(parent) = target_for_write.parent() {
                fs::create_dir_all(parent)?;
            }
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&target_for_write)?;
            file.write_all(content_owned.as_bytes())?;
            file.flush()
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;

        let active_task_id =
            crate::research_task::load_active_task(&workspace)?.map(|task| task.task_id);
        let artifact = crate::artifact::register_file(
            &workspace,
            path,
            active_task_id,
            preferred_node_id.clone(),
        )?;
        let task = crate::research_task::attach_artifact(
            &workspace,
            &artifact.id,
            &artifact.path,
            preferred_node_id.as_deref(),
        )?;
        let node_id = task
            .nodes
            .iter()
            .find(|node| node.outputs.iter().any(|output| output == &artifact.path))
            .map(|node| node.id.clone())
            .ok_or("产物已追加，但未能绑定研究节点")?;
        let artifact =
            crate::artifact::link_artifact(&workspace, &artifact.id, &task.task_id, &node_id)?;
        ctx.send_event(ChatEvent::ResearchTaskUpdated(task.clone()));
        ctx.send_event(ChatEvent::ArtifactCreated(artifact.clone()));
        Ok(json!({
            "status": "delivered",
            "file_path": target.to_string_lossy(),
            "bytes_appended": content.len(),
            "artifact": artifact,
            "research_task": task,
        })
        .to_string())
    }
}

// ── ReplaceText ──
// A manuscript revision must not require an unrestricted shell command. This
// tool makes a narrow, auditable replacement in an existing workspace file.
pub struct ReplaceText;
#[async_trait]
impl GalenTool for ReplaceText {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "replace_text".into(),
            description: Some(
                "Replace one exact text fragment in an existing workspace file. The expected match count defaults to 1; use it for targeted manuscript revisions."
                    .into(),
            ),
            input_schema: json!({"type":"object","properties":{"path":{"type":"string"},"search":{"type":"string","description":"Exact existing text to replace."},"replace":{"type":"string","description":"Replacement text; may be empty."},"expected_matches":{"type":"integer","minimum":1,"description":"Exact number of occurrences expected; defaults to 1."},"node_id":{"type":"string","description":"Optional research node receiving the updated artifact."}},"required":["path","search","replace"]}),
        }
    }
    fn is_write(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let path = input["path"].as_str().ok_or("Missing 'path'")?;
        let search = input["search"].as_str().ok_or("Missing 'search'")?;
        let replace = input["replace"].as_str().ok_or("Missing 'replace'")?;
        if search.is_empty() {
            return Err("'search' must not be empty".into());
        }
        let expected_matches = input["expected_matches"].as_u64().unwrap_or(1) as usize;
        let preferred_node_id = input["node_id"].as_str().map(str::to_string);
        let target = resolve_workspace_path(&ctx.workspace_root, path)?;
        let workspace = ctx
            .workspace_root
            .lock()
            .map_err(|error| format!("Workspace lock error: {error}"))?
            .clone()
            .ok_or("请先选择工作区")?;

        let target_for_write = target.clone();
        let search_owned = search.to_string();
        let replace_owned = replace.to_string();
        let actual_matches = tokio::task::spawn_blocking(move || -> Result<usize, String> {
            let content = fs::read_to_string(&target_for_write).map_err(|error| error.to_string())?;
            let matches = content.matches(&search_owned).count();
            if matches != expected_matches {
                return Err(format!(
                    "Targeted replacement aborted: expected {expected_matches} exact match(es), found {matches}."
                ));
            }
            fs::write(&target_for_write, content.replace(&search_owned, &replace_owned))
                .map_err(|error| error.to_string())?;
            Ok(matches)
        })
        .await
        .map_err(|error| error.to_string())??;

        let active_task_id =
            crate::research_task::load_active_task(&workspace)?.map(|task| task.task_id);
        let artifact = crate::artifact::register_file(
            &workspace,
            path,
            active_task_id,
            preferred_node_id.clone(),
        )?;
        let task = crate::research_task::attach_artifact(
            &workspace,
            &artifact.id,
            &artifact.path,
            preferred_node_id.as_deref(),
        )?;
        let node_id = task
            .nodes
            .iter()
            .find(|node| node.outputs.iter().any(|output| output == &artifact.path))
            .map(|node| node.id.clone())
            .ok_or("产物已更新，但未能绑定研究节点")?;
        let artifact =
            crate::artifact::link_artifact(&workspace, &artifact.id, &task.task_id, &node_id)?;
        ctx.send_event(ChatEvent::ResearchTaskUpdated(task.clone()));
        ctx.send_event(ChatEvent::ArtifactCreated(artifact.clone()));
        Ok(json!({
            "status": "delivered",
            "file_path": target.to_string_lossy(),
            "matches_replaced": actual_matches,
            "artifact": artifact,
            "research_task": task,
        })
        .to_string())
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
