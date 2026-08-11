use api::ToolDefinition;
use async_trait::async_trait;
use serde_json::{json, Value};
use super::workspace_path::resolve_workspace_path;
use super::{GalenTool, ToolContext};

pub struct ExecuteCommand;

#[async_trait]
impl GalenTool for ExecuteCommand {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "execute_command".into(),
            description: Some("Execute a shell command in the workspace. 30s timeout, sandboxed.".into()),
            input_schema: json!({"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}),
        }
    }
    fn is_write(&self) -> bool { true }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String> {
        let command = input["command"].as_str().ok_or("Missing 'command'")?;
        let _ws = resolve_workspace_path(&ctx.workspace_root, "")?;
        let cmd_owned = command.to_string();
        let result = tokio::task::spawn_blocking(move || {
            runtime::execute_bash(runtime::BashCommandInput {
                command: cmd_owned,
                description: None,
                timeout: Some(30_000),
                run_in_background: Some(false),
                dangerously_disable_sandbox: Some(false),
                namespace_restrictions: None,
                isolate_network: None,
                filesystem_mode: None,
                allowed_mounts: None,
            })
        }).await.map_err(|e| format!("join error: {e}"))?.map_err(|e| format!("bash error: {e}"))?;
        let mut out = String::new();
        if !result.stdout.is_empty() { out.push_str(&format!("stdout:\n{}", result.stdout)); }
        if !result.stderr.is_empty() {
            if !out.is_empty() { out.push('\n'); }
            out.push_str(&format!("stderr:\n{}", result.stderr));
        }
        if result.interrupted { out.push_str("\n[interrupted]"); }
        Ok(out)
    }
}
