use super::workspace_path::resolve_workspace_path;
use super::{GalenTool, ToolContext};
use api::ToolDefinition;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;

pub struct SearchFiles;

#[async_trait]
impl GalenTool for SearchFiles {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_files".into(),
            description: Some(
                "Search files by glob pattern in the workspace. Supports optional grep filtering."
                    .into(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Glob pattern (e.g. '*.json')"},
                    "grep": {"type": "string", "description": "Optional text to search within matching files"},
                    "path": {"type": "string", "description": "Optional subdirectory"}
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let pattern = input["pattern"].as_str().ok_or("Missing 'pattern'")?;
        let grep = input["grep"].as_str().filter(|s| !s.is_empty());
        let target =
            resolve_workspace_path(&ctx.workspace_root, input["path"].as_str().unwrap_or(""))?;
        let glob_pat = target.join(pattern).to_string_lossy().to_string();
        let paths = glob::glob(&glob_pat).map_err(|e| format!("Invalid glob: {e}"))?;
        let mut results = Vec::new();
        for entry in paths.flatten() {
            let rel = entry
                .strip_prefix(&target)
                .unwrap_or(&entry)
                .to_string_lossy()
                .to_string();
            let meta = fs::metadata(&entry).ok();
            let prefix = if meta.as_ref().map(|m| m.is_dir()).unwrap_or(false) {
                "[DIR]"
            } else {
                "[FILE]"
            };
            if let Some(g) = grep {
                if !meta.as_ref().map(|m| m.is_dir()).unwrap_or(true) {
                    if let Ok(content) = fs::read_to_string(&entry) {
                        if content.contains(g) {
                            let preview = content
                                .lines()
                                .filter(|l| l.contains(g))
                                .take(5)
                                .collect::<Vec<_>>()
                                .join("\n  ");
                            results.push(format!("{prefix} {rel}\n  {preview}"));
                        }
                    }
                }
            } else {
                results.push(format!(
                    "{prefix} {rel} ({} bytes)",
                    meta.map(|m| m.len()).unwrap_or(0)
                ));
            }
        }
        Ok(if results.is_empty() {
            format!("No files matching '{}'", pattern)
        } else {
            format!("Search results for '{}':\n{}", pattern, results.join("\n"))
        })
    }
}
