use super::workspace_path::resolve_workspace_path;
use super::{GalenTool, ToolContext};
use crate::backend::ChatEvent;
use api::ToolDefinition;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct CompilePdfReport;
pub struct CompileLatexPaper;

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
            let mut cmd = std::process::Command::new(typst);
            cmd.arg("compile")
                .arg(&source_for_process)
                .arg(&output_for_process)
                .current_dir(&workspace_for_process);
            // Do not flash a console window when compiling reports from the GUI.
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }
            cmd.output()
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

#[async_trait]
impl GalenTool for CompileLatexPaper {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "compile_latex_paper".into(),
            description: Some(
                "Compile a XeLaTeX manuscript in the workspace into a black-and-white PDF and register it as a delivered artifact."
                    .into(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": {"type": "string", "description": "Workspace-relative .tex manuscript path."},
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
        if !source_rel.to_ascii_lowercase().ends_with(".tex") {
            return Err("LaTeX paper source must be a .tex file".into());
        }
        let output_rel = input["output"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}.pdf", source_rel.trim_end_matches(".tex")));
        if !output_rel.to_ascii_lowercase().ends_with(".pdf") {
            return Err("LaTeX paper output must be a .pdf file".into());
        }

        let source = resolve_workspace_path(&ctx.workspace_root, source_rel)?;
        if !source.is_file() {
            return Err(format!("LaTeX source does not exist: {source_rel}"));
        }
        let output = resolve_workspace_path(&ctx.workspace_root, &output_rel)?;
        let output_dir = output
            .parent()
            .ok_or("LaTeX paper output must have a parent directory")?
            .to_path_buf();
        std::fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;
        let workspace = ctx
            .workspace_root
            .lock()
            .map_err(|error| format!("Workspace lock error: {error}"))?
            .clone()
            .ok_or("请先选择工作区")?;
        let xelatex = super::resolve_xelatex()?;
        let source_for_process = source.clone();
        let output_dir_for_process = output_dir.clone();
        let workspace_for_process = workspace.clone();
        let result = tokio::task::spawn_blocking(move || {
            run_xelatex_twice(
                &xelatex,
                &source_for_process,
                &output_dir_for_process,
                &workspace_for_process,
            )
        })
        .await
        .map_err(|error| format!("XeLaTeX process join error: {error}"))??;
        if !result.success {
            return Err(format!(
                "XeLaTeX compilation failed:\n{}",
                result.diagnostics
            ));
        }

        let generated = output_dir.join(
            source
                .file_stem()
                .ok_or("LaTeX source has no file stem")?
                .to_string_lossy()
                .to_string()
                + ".pdf",
        );
        if !generated.is_file() {
            return Err("XeLaTeX completed but did not produce a PDF".into());
        }
        if generated != output {
            if output.exists() {
                std::fs::remove_file(&output).map_err(|error| error.to_string())?;
            }
            std::fs::rename(&generated, &output).map_err(|error| error.to_string())?;
        }
        let metadata =
            std::fs::metadata(&output).map_err(|error| format!("PDF was not produced: {error}"))?;
        if metadata.len() == 0 {
            return Err("PDF was produced but is empty".into());
        }

        register_pdf_artifact(
            &workspace,
            &output_rel,
            metadata.len(),
            input["node_id"].as_str(),
            ctx,
        )
    }
}

struct LatexRun {
    success: bool,
    diagnostics: String,
}

fn run_xelatex_twice(
    xelatex: &std::path::Path,
    source: &std::path::Path,
    output_dir: &std::path::Path,
    workspace: &std::path::Path,
) -> Result<LatexRun, String> {
    let mut diagnostics = String::new();
    // `resolve_workspace_path` deliberately produces an extended-length path
    // on Windows. XeLaTeX treats the leading `\\?\` as TeX syntax, so pass a
    // regular Windows path to the compiler while retaining the safe path for
    // filesystem operations elsewhere in this tool.
    let source_arg = latex_cli_path(source);
    let output_dir_arg = latex_cli_path(output_dir);
    let workspace_arg = latex_cli_path(workspace);
    for pass in 1..=2 {
        let mut command = std::process::Command::new(xelatex);
        command
            .arg("-interaction=nonstopmode")
            .arg("-halt-on-error")
            .arg("-file-line-error")
            .arg(format!("-output-directory={}", output_dir_arg.display()))
            .arg(&source_arg)
            .current_dir(&workspace_arg);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let output = command
            .output()
            .map_err(|error| format!("Failed to start XeLaTeX: {error}"))?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        diagnostics.push_str(&format!("pass {pass}:\n{stdout}\n{stderr}\n"));
        if !output.status.success() {
            return Ok(LatexRun {
                success: false,
                diagnostics,
            });
        }
    }
    Ok(LatexRun {
        success: true,
        diagnostics,
    })
}

fn latex_cli_path(path: &std::path::Path) -> std::path::PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        if let Some(without_prefix) = text.strip_prefix(r"\\?\") {
            return std::path::PathBuf::from(without_prefix);
        }
    }
    path.to_path_buf()
}

fn register_pdf_artifact(
    workspace: &std::path::Path,
    output_rel: &str,
    bytes: u64,
    preferred_node_id: Option<&str>,
    ctx: &ToolContext,
) -> Result<String, String> {
    let active_task_id =
        crate::research_task::load_active_task(workspace)?.map(|task| task.task_id);
    let artifact = crate::artifact::register_file(
        workspace,
        output_rel,
        active_task_id,
        preferred_node_id.map(str::to_string),
    )?;
    let task = crate::research_task::attach_artifact(
        workspace,
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
        crate::artifact::link_artifact(workspace, &artifact.id, &task.task_id, &node_id)?;
    ctx.send_event(ChatEvent::ResearchTaskUpdated(task.clone()));
    ctx.send_event(ChatEvent::ArtifactCreated(artifact.clone()));

    Ok(json!({
        "status": "delivered",
        "file_path": output_rel,
        "bytes": bytes,
        "artifact": artifact,
        "research_task": task,
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::ChatMode;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn context() -> ToolContext {
        let mut context = ToolContext::new(
            Arc::new(medical_core::MedicalCore::new(None)),
            Mutex::new(None),
        );
        context.mode = ChatMode::Auto;
        context
    }

    fn temp_workspace(tag: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("galen-{tag}-{nonce}"));
        std::fs::create_dir_all(&workspace).unwrap();
        workspace
    }

    fn workspace_context(workspace: PathBuf) -> ToolContext {
        let mut context = ToolContext::new(
            Arc::new(medical_core::MedicalCore::new(None)),
            Mutex::new(Some(workspace)),
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

    #[test]
    fn latex_definition_exposes_a_closed_loop_tool() {
        let definition = CompileLatexPaper.definition();
        assert_eq!(definition.name, "compile_latex_paper");
        assert!(definition
            .input_schema
            .get("required")
            .and_then(Value::as_array)
            .is_some_and(|required| required == &[Value::String("source".into())]));
    }

    #[tokio::test]
    async fn xelatex_compiles_a_black_and_white_chinese_manuscript() {
        if super::super::resolve_xelatex().is_err() {
            return;
        }
        let workspace = temp_workspace("latex-paper");
        std::fs::write(
            workspace.join("paper.tex"),
            r#"\documentclass[UTF8,12pt]{ctexart}
\pagestyle{plain}
\begin{document}
\title{运动疲劳恢复的多模态纵向分析}
\author{Galen}
\date{}
\maketitle
\section{结果}
所有文字与表格均为黑白排版。
\end{document}
"#,
        )
        .unwrap();
        let result = CompileLatexPaper
            .execute(
                json!({"source": "paper.tex", "output": "output/paper.pdf"}),
                &workspace_context(workspace.clone()),
            )
            .await
            .unwrap();
        assert!(result.contains("\"status\":\"delivered\""));
        let pdf = std::fs::read(workspace.join("output/paper.pdf")).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[tokio::test]
    async fn latex_tool_rejects_non_tex_sources_before_touching_workspace() {
        let result = CompileLatexPaper
            .execute(json!({"source": "paper.typ"}), &context())
            .await;
        assert_eq!(
            result.unwrap_err(),
            "LaTeX paper source must be a .tex file"
        );
    }
}
