use super::workspace_path::resolve_workspace_path;
use super::{GalenTool, ToolContext};
use crate::backend::ChatEvent;
use api::ToolDefinition;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct CompilePdfReport;

#[async_trait]
impl GalenTool for CompilePdfReport {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "compile_pdf_report".into(),
            description: Some(
                "Compile a Typst source file in the workspace into a PDF and register it as a delivered artifact."
                    .into(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": {"type": "string", "description": "Workspace-relative .typ source path."},
                    "output": {"type": "string", "description": "Optional workspace-relative .pdf output path."},
                    "node_id": {"type": "string", "description": "Optional research node receiving the artifact."}
                },
                "required": ["source"]
            }),
        }
    }

    fn is_write(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let source_rel = input["source"].as_str().ok_or("Missing 'source'")?;
        if !source_rel.to_ascii_lowercase().ends_with(".typ") {
            return Err("PDF report source must be a .typ file".into());
        }
        let output_rel = input["output"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}.pdf", source_rel.trim_end_matches(".typ")));
        if !output_rel.to_ascii_lowercase().ends_with(".pdf") {
            return Err("PDF report output must be a .pdf file".into());
        }

        let source = resolve_workspace_path(&ctx.workspace_root, source_rel)?;
        if !source.is_file() {
            return Err(format!("Typst source does not exist: {source_rel}"));
        }
        let output = resolve_workspace_path(&ctx.workspace_root, &output_rel)?;
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let workspace = ctx
            .workspace_root
            .lock()
            .map_err(|error| format!("Workspace lock error: {error}"))?
            .clone()
            .ok_or("请先选择工作区")?;
        let typst = super::resolve_typst()?;
        let source_for_process = source.clone();
        let output_for_process = output.clone();
        let workspace_for_process = workspace.clone();
        let result = tokio::task::spawn_blocking(move || {
            std::process::Command::new(typst)
                .arg("compile")
                .arg(&source_for_process)
                .arg(&output_for_process)
                .current_dir(&workspace_for_process)
                .output()
        })
        .await
        .map_err(|error| format!("Typst process join error: {error}"))?
        .map_err(|error| format!("Failed to start Typst: {error}"))?;
        if !result.status.success() {
            return Err(format!(
                "Typst compilation failed:\n{}",
                String::from_utf8_lossy(&result.stderr).trim()
            ));
        }
        let metadata =
            std::fs::metadata(&output).map_err(|error| format!("PDF was not produced: {error}"))?;
        if metadata.len() == 0 {
            return Err("PDF was produced but is empty".into());
        }

        let preferred_node_id = input["node_id"].as_str();
        let active_task_id =
            crate::research_task::load_active_task(&workspace)?.map(|task| task.task_id);
        let artifact = crate::artifact::register_file(
            &workspace,
            &output_rel,
            active_task_id,
            preferred_node_id.map(str::to_string),
        )?;
        let task = crate::research_task::attach_artifact(
            &workspace,
            &artifact.id,
            &artifact.path,
            preferred_node_id,
        )?;
        let node_id = task
            .nodes
            .iter()
            .find(|node| node.outputs.iter().any(|item| item == &artifact.path))
            .map(|node| node.id.clone())
            .ok_or("PDF 已生成，但未能绑定研究节点")?;
        let artifact =
            crate::artifact::link_artifact(&workspace, &artifact.id, &task.task_id, &node_id)?;
        ctx.send_event(ChatEvent::ResearchTaskUpdated(task.clone()));
        ctx.send_event(ChatEvent::ArtifactCreated(artifact.clone()));

        Ok(json!({
            "status": "delivered",
            "source": source_rel,
            "file_path": output_rel,
            "bytes": metadata.len(),
            "artifact": artifact,
            "research_task": task,
        })
        .to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::ChatMode;
    use std::sync::{Arc, Mutex};

    fn context() -> ToolContext {
        let mut context = ToolContext::new(
            Arc::new(medical_core::MedicalCore::new(None)),
            Mutex::new(None),
        );
        context.mode = ChatMode::Auto;
        context
    }

    #[test]
    fn definition_exposes_a_single_closed_loop_tool() {
        let definition = CompilePdfReport.definition();
        assert_eq!(definition.name, "compile_pdf_report");
        assert!(definition
            .input_schema
            .get("required")
            .and_then(Value::as_array)
            .is_some_and(|required| required == &[Value::String("source".into())]));
    }

    #[tokio::test]
    async fn rejects_non_typst_sources_before_touching_workspace() {
        let result = CompilePdfReport
            .execute(json!({"source": "report.md"}), &context())
            .await;
        assert_eq!(result.unwrap_err(), "PDF report source must be a .typ file");
    }
}
