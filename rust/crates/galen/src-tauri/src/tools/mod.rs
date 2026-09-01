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
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use api::ToolDefinition;
use async_trait::async_trait;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::time::timeout;

use crate::backend::ChatEvent;

/// Per-tool execution timeout.
const TOOL_TIMEOUT: Duration = Duration::from_secs(30);

pub mod clinical;
pub mod command;
pub mod evidence_search;
pub mod fs;
pub mod medical;
pub mod rehab;
pub mod report;
pub mod research;
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

    /// Execute while retaining provider-native data needed by host provenance.
    /// Ordinary tools preserve the historical result and expose no metadata.
    async fn execute_observed(&self, input: Value, ctx: &ToolContext) -> ToolExecution {
        ToolExecution::from_result(self.execute(input, ctx).await)
    }

    /// Whether this tool modifies state (blocked in Discuss mode).
    fn is_write(&self) -> bool {
        false
    }
}

#[doc(hidden)]
pub struct ToolExecution {
    pub result: Result<String, String>,
    pub raw_output: Option<Value>,
    pub result_count: Option<usize>,
    pub query: Option<String>,
}

impl ToolExecution {
    pub(crate) fn from_result(result: Result<String, String>) -> Self {
        Self {
            result,
            raw_output: None,
            result_count: None,
            query: None,
        }
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

    /// Register domain-neutral kernel tools.
    pub fn register_kernel(&mut self) {
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

    /// Assemble the official workbench from a thin kernel and capability packs.
    pub fn register_builtin(&mut self) {
        self.register_kernel();
        crate::capability::register_official(&crate::capability::CapabilityConfig::default(), self);
    }

    /// Assemble the workbench using ~/.galen/capabilities.toml.
    pub fn configured() -> Self {
        let mut registry = Self::new();
        registry.register_kernel();
        crate::capability::register_official(
            &crate::capability::CapabilityConfig::load(),
            &mut registry,
        );
        registry
    }

    /// Tool definitions for the model.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Report whether a built-in tool mutates state. Unknown/MCP tools return
    /// `None` so callers do not accidentally cache an external side effect.
    pub fn is_write_tool(&self, name: &str) -> Option<bool> {
        self.tools.get(name).map(|tool| tool.is_write())
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
            for (server_name, tool) in self.mcp.mcp_tool_definitions() {
                defs.push(ToolDefinition {
                    name: crate::mcp_client::qualified_tool_name(&server_name, &tool.name),
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
            let search = research::recognized_builtin_search(name);
            let scope = match search {
                Some(_) => Some(snapshot_search_scope(ctx).map_err(provenance_failure)?),
                None => None,
            };
            let arguments = input.clone();
            let started_at = epoch_millis();
            let execution = match timeout(TOOL_TIMEOUT, tool.execute_observed(input, ctx)).await {
                Ok(execution) => execution,
                Err(_) => ToolExecution::from_result(Err(format!(
                    "工具「{tool_name}」执行超时（{} 秒）。请检查输入或重试。",
                    TOOL_TIMEOUT.as_secs()
                ))),
            };
            if let (Some(search), Some(scope)) = (search, scope.as_ref()) {
                if let Err(error) = record_search_run(
                    scope,
                    search,
                    name,
                    &arguments,
                    started_at,
                    &execution.result,
                    execution.raw_output.as_ref(),
                    execution.result_count,
                    execution.query.as_deref(),
                ) {
                    return Err(provenance_failure(error));
                }
            }
            return execution.result;
        }

        // Try MCP tools
        if name.starts_with("mcp__") {
            let mut available = Vec::new();
            for server in self.mcp.servers() {
                let server = server.lock().await;
                available.extend(
                    server
                        .tools()
                        .into_iter()
                        .map(|tool| (server.name.clone(), tool.name)),
                );
            }
            let (server_name, tool_name) = crate::mcp_client::resolve_tool_route(name, &available)
                .map_err(|e| e.to_string())?;
            let search = research::recognized_mcp_search(&server_name, &tool_name);
            let scope = match search {
                Some(_) => Some(snapshot_search_scope(ctx).map_err(provenance_failure)?),
                None => None,
            };
            let arguments = input.clone();
            let started_at = epoch_millis();
            let outcome = self
                .mcp
                .call_tool_observed(&server_name, &tool_name, input)
                .await;
            let result = outcome.result.map_err(|error| error.to_string());
            if let (Some(search), Some(scope)) = (search, scope.as_ref()) {
                if let Err(error) = record_search_run(
                    scope,
                    search,
                    &tool_name,
                    &arguments,
                    started_at,
                    &result,
                    outcome.raw_output.as_ref(),
                    None,
                    None,
                ) {
                    return Err(provenance_failure(error));
                }
            }
            return result;
        }

        Err(format!("Unknown tool: {name}"))
    }
}

struct SearchRunScope {
    workspace: PathBuf,
    task_id: String,
}

fn snapshot_search_scope(ctx: &ToolContext) -> Result<SearchRunScope, String> {
    let workspace = ctx
        .workspace_root
        .lock()
        .map_err(|error| format!("workspace lock failed: {error}"))?
        .clone()
        .ok_or("no workspace selected for literature coverage provenance")?;
    let task_id = crate::research_task::load_active_task(&workspace)?
        .ok_or("no active research task for literature coverage provenance")?
        .task_id;
    Ok(SearchRunScope { workspace, task_id })
}

#[allow(clippy::too_many_arguments)]
fn record_search_run(
    scope: &SearchRunScope,
    search: research::RecognizedSearch,
    tool_name: &str,
    arguments: &Value,
    started_at: u128,
    result: &Result<String, String>,
    raw_output: Option<&Value>,
    observed_count: Option<usize>,
    observed_query: Option<&str>,
) -> Result<(), String> {
    let finished_at = epoch_millis().max(started_at);
    let query = observed_query
        .map(str::to_string)
        .unwrap_or_else(|| search.query_from(arguments));
    let hash_input = raw_output
        .and_then(|raw| serde_json::to_vec(raw).ok())
        .unwrap_or_else(|| match result {
            Ok(output) => output.as_bytes().to_vec(),
            Err(error) => error.as_bytes().to_vec(),
        });
    let raw_result_hash = format!("{:x}", Sha256::digest(hash_input));
    let run = match result {
        Ok(_) => {
            let result_count =
                observed_count.or_else(|| raw_output.and_then(|raw| search.result_count_from(raw)));
            let mut run = crate::search_run::SearchRun::succeeded(
                scope.task_id.clone(),
                search.provider_id,
                tool_name,
                &query,
                arguments.clone(),
                started_at.to_string(),
                finished_at.to_string(),
                result_count.unwrap_or_default(),
                raw_result_hash,
            )?;
            if result_count.is_none() {
                run.result_count = None;
            }
            run
        }
        Err(error) => crate::search_run::SearchRun::failed(
            scope.task_id.clone(),
            search.provider_id,
            tool_name,
            &query,
            arguments.clone(),
            started_at.to_string(),
            finished_at.to_string(),
            crate::search_run::SearchErrorClass::classify(error),
            raw_result_hash,
        )?,
    };
    crate::search_run::append_search_run(&scope.workspace, &run)
}

fn provenance_failure(error: String) -> String {
    format!("Literature coverage provenance could not be recorded: {error}")
}

fn epoch_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
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
    use crate::search_run::{load_search_runs, SearchRunStatus};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct SuccessfulPubMed;

    struct SwitchingPubMed;

    #[async_trait]
    impl GalenTool for SuccessfulPubMed {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "search_pubmed".into(),
                description: None,
                input_schema: serde_json::json!({"type": "object"}),
            }
        }

        async fn execute(&self, _input: Value, _ctx: &ToolContext) -> Result<String, String> {
            Ok("Found 2 results.".into())
        }

        async fn execute_observed(&self, _input: Value, _ctx: &ToolContext) -> ToolExecution {
            ToolExecution {
                result: Ok("Found 2 results.".into()),
                raw_output: Some(serde_json::json!([{"pmid": "1"}, {"pmid": "2"}])),
                result_count: Some(2),
                query: None,
            }
        }
    }

    #[async_trait]
    impl GalenTool for SwitchingPubMed {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "search_pubmed".into(),
                description: None,
                input_schema: serde_json::json!({"type": "object"}),
            }
        }

        async fn execute(&self, _input: Value, ctx: &ToolContext) -> Result<String, String> {
            switch_active_task(ctx)?;
            Ok("Found 1 result.".into())
        }

        async fn execute_observed(&self, _input: Value, ctx: &ToolContext) -> ToolExecution {
            if let Err(error) = switch_active_task(ctx) {
                return ToolExecution::from_result(Err(error));
            }
            ToolExecution {
                result: Ok("Found 1 result.".into()),
                raw_output: Some(serde_json::json!([{"pmid": "1"}])),
                result_count: Some(1),
                query: None,
            }
        }
    }

    fn switch_active_task(ctx: &ToolContext) -> Result<(), String> {
        let root = ctx
            .workspace_root
            .lock()
            .map_err(|error| error.to_string())?
            .clone()
            .ok_or("missing workspace")?;
        crate::research_task::create_task(
            &root,
            "task-2".into(),
            "switched while search was in flight".into(),
            Vec::new(),
        )?;
        Ok(())
    }

    fn temp_workspace(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "galen-tool-search-run-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn active_task(root: &std::path::Path) -> String {
        crate::research_task::create_task(
            root,
            "task-1".into(),
            "test search recording".into(),
            Vec::new(),
        )
        .unwrap()
        .task_id
    }

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
        assert_eq!(defs.len(), 19);
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        for expected in &[
            "search_pubmed",
            "fetch_article",
            "format_citation",
            "analyze_clinical_case",
            "rehab_data",
            "search_rehab_literature",
            "search_evidence",
            "create_research_plan",
            "write_file",
            "read_file",
            "delete_file",
            "execute_command",
            "search_files",
            "compile_pdf_report",
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
    async fn pubmed_success_appends_one_terminal_search_run_with_paper_count() {
        // Removing the post-dispatch ledger append, or recording twice, must fail this test.
        let root = temp_workspace("pubmed-success");
        let task_id = active_task(&root);
        let ctx = ToolContext::new(
            Arc::new(medical_core::MedicalCore::new(None)),
            Mutex::new(Some(root.clone())),
        );
        let mut registry = ToolRegistry::new();
        registry.register(SuccessfulPubMed);
        let input = serde_json::json!({"query": "stroke rehabilitation", "max_results": 5});

        let result = registry.execute_dynamic("search_pubmed", input, &ctx).await;
        let runs = load_search_runs(&root, &task_id).unwrap();

        assert_eq!(result.unwrap(), "Found 2 results.");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].provider_id, "pubmed");
        assert_eq!(runs[0].tool_name, "search_pubmed");
        assert_eq!(runs[0].query, "stroke rehabilitation");
        assert_eq!(runs[0].status, SearchRunStatus::Succeeded);
        assert_eq!(runs[0].result_count, Some(2));
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn pubmed_success_is_not_reported_when_search_run_append_fails() {
        // Ignoring append_search_run errors would report provider success without durable coverage.
        let root = temp_workspace("pubmed-ledger-failure");
        let task_id = active_task(&root);
        let ledger_path = root
            .join(".galen")
            .join("tasks")
            .join(&task_id)
            .join("search-runs.jsonl");
        std::fs::create_dir_all(&ledger_path).unwrap();
        let ctx = ToolContext::new(
            Arc::new(medical_core::MedicalCore::new(None)),
            Mutex::new(Some(root.clone())),
        );
        let mut registry = ToolRegistry::new();
        registry.register(SuccessfulPubMed);

        let result = registry
            .execute_dynamic(
                "search_pubmed",
                serde_json::json!({"query": "stroke rehabilitation"}),
                &ctx,
            )
            .await;

        let error = result.unwrap_err();
        assert!(
            error.starts_with("Literature coverage provenance could not be recorded:"),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn in_flight_task_switch_records_search_against_original_task() {
        // Looking up the active task after dispatch would attach this run to task-2.
        let root = temp_workspace("pubmed-task-switch");
        let original_task_id = active_task(&root);
        let ctx = ToolContext::new(
            Arc::new(medical_core::MedicalCore::new(None)),
            Mutex::new(Some(root.clone())),
        );
        let mut registry = ToolRegistry::new();
        registry.register(SwitchingPubMed);

        let result = registry
            .execute_dynamic(
                "search_pubmed",
                serde_json::json!({"query": "stroke rehabilitation"}),
                &ctx,
            )
            .await;
        let switched_task_id = crate::research_task::load_active_task(&root)
            .unwrap()
            .unwrap()
            .task_id;
        let original_runs = load_search_runs(&root, &original_task_id).unwrap();
        let switched_runs = load_search_runs(&root, &switched_task_id).unwrap();

        assert_eq!(result.unwrap(), "Found 1 result.");
        assert_ne!(switched_task_id, original_task_id);
        assert_eq!(original_runs.len(), 1);
        assert_eq!(original_runs[0].task_id, original_task_id);
        assert!(switched_runs.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn tool_timeout_is_configured() {
        // Verify the timeout constant is set (actual timeout tested manually)
        assert_eq!(TOOL_TIMEOUT, Duration::from_secs(30));
    }
}
