//! Galen tool system — plugin-based architecture.
//!
//! Tools implement the [`GalenTool`] trait and register themselves with the
//! [`ToolRegistry`].  No hardcoded dispatch — adding a tool is just:
//!
//! ```ignore
//! registry.register(MyTool::new());
//! ```
//!
//! Medical-specific tools live in their own modules but are registered the
//! same way as any other tool.  The core tool system has zero medical knowledge.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use api::ToolDefinition;
use async_trait::async_trait;
use serde_json::Value;
use tokio::time::timeout;

use crate::backend::ChatEvent;

/// Per-tool execution timeout.
const TOOL_TIMEOUT: Duration = Duration::from_secs(30);

pub mod clinical;
pub mod command;
pub mod fs;
pub mod medical;
pub mod rehab;
pub mod search;
pub mod workspace_path;

// ---------------------------------------------------------------------------
// GalenTool trait
// ---------------------------------------------------------------------------

/// Every Galen tool implements this trait.  Tools are registered by name and
/// dispatched dynamically — no `match` statement needed.
#[async_trait]
pub trait GalenTool: Send + Sync {
    /// The tool definition sent to the model (name, description, schema).
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with the given JSON input and shared context.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String, String>;

    /// Whether this tool modifies state (blocked in Discuss mode).
    fn is_write(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Bundled binary resolution
// ---------------------------------------------------------------------------

/// Detect a bundled or system-installed binary by name.
pub fn resolve_binary(name: &str) -> Option<PathBuf> {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let bundled = if cfg!(windows) {
                exe_dir.join(format!("{name}.exe"))
            } else {
                exe_dir.join(name)
            };
            if bundled.exists() {
                return Some(bundled);
            }
            let subdir = exe_dir.join("binaries").join(if cfg!(windows) {
                format!("{name}.exe")
            } else {
                name.to_string()
            });
            if subdir.exists() {
                return Some(subdir);
            }
            let nested = exe_dir.join("binaries").join(name).join(if cfg!(windows) {
                format!("{name}.exe")
            } else {
                name.to_string()
            });
            if nested.exists() {
                return Some(nested);
            }
        }
    }
    which::which(name).ok()
}

pub fn resolve_typst() -> Result<PathBuf, String> {
    resolve_binary("typst").ok_or_else(|| {
        "Typst 未安装。请运行 `cargo install typst-cli` 或从 \
         https://github.com/typst/typst/releases 下载后放在 Galen.exe 同目录下。"
            .to_string()
    })
}

// ---------------------------------------------------------------------------
// ToolContext — shared execution context
// ---------------------------------------------------------------------------

pub struct ToolContext {
    pub medical: Arc<medical_core::MedicalCore>,
    pub workspace_root: Mutex<Option<PathBuf>>,
    pub bin_dirs: Vec<PathBuf>,
    pub mode: crate::modes::ChatMode,
    event_sender: Option<Arc<dyn Fn(ChatEvent) + Send + Sync>>,
}

impl ToolContext {
    pub fn new(
        medical: Arc<medical_core::MedicalCore>,
        workspace_root: Mutex<Option<PathBuf>>,
    ) -> Self {
        Self {
            medical,
            workspace_root,
            bin_dirs: Vec::new(),
            mode: crate::modes::ChatMode::default(),
            event_sender: None,
        }
    }

    pub fn with_event_sender(
        medical: Arc<medical_core::MedicalCore>,
        workspace_root: Mutex<Option<PathBuf>>,
        event_sender: Arc<dyn Fn(ChatEvent) + Send + Sync>,
    ) -> Self {
        Self {
            medical,
            workspace_root,
            bin_dirs: Vec::new(),
            mode: crate::modes::ChatMode::default(),
            event_sender: Some(event_sender),
        }
    }

    pub fn send_event(&self, event: ChatEvent) {
        if let Some(ref sender) = self.event_sender {
            sender(event);
        }
    }
}

// ---------------------------------------------------------------------------
// ToolRegistry — plugin-based
// ---------------------------------------------------------------------------

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn GalenTool>>,
    pub mcp: crate::mcp_client::McpServerRegistry,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            mcp: crate::mcp_client::McpServerRegistry::new(),
        }
    }

    /// Register a tool instance.
    pub fn register(&mut self, tool: impl GalenTool + 'static) {
        let def = tool.definition();
        self.tools.insert(def.name.clone(), Box::new(tool));
    }

    /// Register all built-in tools (medical, file ops, command, search).
    pub fn register_builtin(&mut self) {
        // Medical domain tools
        self.register(medical::SearchPubMed);
        self.register(medical::FetchArticle);
        self.register(medical::FormatCitation);
        self.register(clinical::AnalyzeClinicalCase);
        self.register(rehab::RehabData);
        self.register(medical::SearchRehabLiterature);

        // File operations
        self.register(fs::CreateDirectory);
        self.register(fs::WriteFile);
        self.register(fs::ReadFile);
        self.register(fs::ListFiles);
        self.register(fs::SavePaper);
        self.register(fs::DeleteFile);
        self.register(fs::DeleteDirectory);
        self.register(fs::MoveFile);

        // Search & command
        self.register(search::SearchFiles);
        self.register(command::ExecuteCommand);
    }

    /// Tool definitions for the model.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    // ── MCP ──

    pub async fn connect_mcp_servers(&mut self) {
        self.mcp = crate::mcp_client::connect_configured_servers().await;
    }

    pub fn load_mcp_from_cache(
        &mut self,
        servers: Vec<Arc<tokio::sync::Mutex<crate::mcp_client::McpServer>>>,
    ) {
        self.mcp = crate::mcp_client::McpServerRegistry::from_cached(servers);
    }

    pub async fn all_definitions(&self) -> Vec<ToolDefinition> {
        self.all_definitions_for_mode(crate::modes::ChatMode::Auto)
            .await
    }

    /// 按模式裁剪工具定义（L1 层）：
    /// 讨论模式只暴露只读工具，不暴露 MCP；计划/自动模式全量。
    pub async fn all_definitions_for_mode(
        &self,
        mode: crate::modes::ChatMode,
    ) -> Vec<ToolDefinition> {
        let mut defs = self.definitions();
        if mode == crate::modes::ChatMode::Discuss {
            defs.retain(|d| {
                self.tools
                    .get(&d.name)
                    .map(|t| !t.is_write())
                    .unwrap_or(false)
            });
        } else {
            for tool in self.mcp.mcp_tool_definitions() {
                defs.push(ToolDefinition {
                    name: format!("mcp__{}", tool.name),
                    description: tool.description,
                    input_schema: tool
                        .input_schema
                        .unwrap_or(serde_json::json!({"type":"object","properties":{}})),
                });
            }
        }
        defs
    }

    // ── Dispatch ──

    pub async fn execute_dynamic(
        &self,
        name: &str,
        input: Value,
        ctx: &ToolContext,
    ) -> Result<String, String> {
        // Mode-gate: Discuss blocks write tools
        if ctx.mode == crate::modes::ChatMode::Discuss {
            if let Some(tool) = self.tools.get(name) {
                if tool.is_write() {
                    return Err("当前模式为「讨论」，不允许执行写操作。\n请点击顶栏模式按钮，切换到「计划」或「自动」模式后重试。".into());
                }
            }
            if name.starts_with("mcp__") {
                return Err("当前模式为「讨论」，不允许执行 MCP 工具。\n请点击顶栏模式按钮，切换到「计划」或「自动」模式后重试。".into());
            }
        }

        // Try built-in tools (with timeout)
        if let Some(tool) = self.tools.get(name) {
            let tool_name = name.to_string();
            return match timeout(TOOL_TIMEOUT, tool.execute(input, ctx)).await {
                Ok(result) => result,
                Err(_) => Err(format!(
                    "工具「{tool_name}」执行超时（{} 秒）。请检查输入或重试。",
                    TOOL_TIMEOUT.as_secs()
                )),
            };
        }

        // Try MCP tools
        if let Some(tool_name) = name.strip_prefix("mcp__") {
            for server in self.mcp.servers() {
                let s = server.lock().await;
                if s.tools().iter().any(|t| t.name == tool_name) {
                    let server_name = s.name.clone();
                    drop(s);
                    return self
                        .mcp
                        .call_tool(&server_name, tool_name, input)
                        .await
                        .map_err(|e| e.to_string());
                }
            }
        }

        Err(format!("Unknown tool: {name}"))
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register_builtin();
        registry
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::ChatMode;

    fn test_ctx(mode: ChatMode) -> ToolContext {
        let mut ctx = ToolContext::new(
            Arc::new(medical_core::MedicalCore::new(None)),
            Mutex::new(None),
        );
        ctx.mode = mode;
        ctx
    }

    // ── Mode gate ──

    #[tokio::test]
    async fn discuss_mode_blocks_write_tools() {
        let registry = ToolRegistry::default();
        let ctx = test_ctx(ChatMode::Discuss);
        let write_tools = ["write_file", "create_directory", "execute_command"];
        for name in write_tools {
            let result = registry
                .execute_dynamic(name, serde_json::json!({}), &ctx)
                .await;
            assert!(result.is_err(), "Discuss should block: {name}");
        }
    }

    #[tokio::test]
    async fn discuss_mode_allows_read_tools() {
        let registry = ToolRegistry::default();
        let ctx = test_ctx(ChatMode::Discuss);
        let result = registry
            .execute_dynamic("read_file", serde_json::json!({"path":"test.txt"}), &ctx)
            .await;
        // Will fail because no workspace, but NOT because of mode gate
        assert!(!result.unwrap_err().contains("讨论"));
    }

    #[tokio::test]
    async fn auto_mode_allows_write_tools() {
        let registry = ToolRegistry::default();
        let ctx = test_ctx(ChatMode::Auto);
        for name in ["write_file", "create_directory", "execute_command"] {
            let result = registry
                .execute_dynamic(name, serde_json::json!({}), &ctx)
                .await;
            match result {
                Err(e) => assert!(!e.contains("讨论"), "Auto should NOT block {name}: {e}"),
                Ok(_) => {}
            }
        }
    }

    // ── Registry ──

    #[test]
    fn registry_has_all_builtin_definitions() {
        let registry = ToolRegistry::default();
        let defs = registry.definitions();
        assert_eq!(defs.len(), 16);
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        for expected in &[
            "search_pubmed",
            "fetch_article",
            "format_citation",
            "analyze_clinical_case",
            "rehab_data",
            "search_rehab_literature",
            "write_file",
            "read_file",
            "delete_file",
            "execute_command",
            "search_files",
        ] {
            assert!(names.contains(expected), "missing: {expected}");
        }
    }

    #[test]
    fn plugin_registration_is_idempotent_by_name() {
        let mut r = ToolRegistry::new();
        r.register(fs::WriteFile);
        r.register(fs::WriteFile); // register twice
        assert_eq!(
            r.definitions()
                .iter()
                .filter(|d| d.name == "write_file")
                .count(),
            1
        );
    }

    #[test]
    fn write_tools_mark_is_write() {
        let mut r = ToolRegistry::new();
        r.register(fs::WriteFile);
        r.register(fs::ReadFile);
        assert!(r.tools.get("write_file").unwrap().is_write());
        assert!(!r.tools.get("read_file").unwrap().is_write());
    }

    // ── Dispatch ──

    #[tokio::test]
    async fn unknown_tool_returns_clear_error() {
        let registry = ToolRegistry::default();
        let result = registry
            .execute_dynamic(
                "nonexistent_tool",
                serde_json::json!({}),
                &test_ctx(ChatMode::Auto),
            )
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown tool"));
    }

    #[tokio::test]
    async fn tool_timeout_is_configured() {
        // Verify the timeout constant is set (actual timeout tested manually)
        assert_eq!(TOOL_TIMEOUT, Duration::from_secs(30));
    }
}
