use std::sync::{Arc, Mutex};

use api::{
    InputContentBlock, InputMessage, OpenAiCompatClient, OpenAiCompatConfig, ProviderClient, Usage,
};
use medical_core::types::Paper;
use medical_core::MedicalCore;
use model_router::ModelRouter;
use std::path::PathBuf;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
use crate::context_compaction::archive_compaction;

pub use crate::chat_loop::run_chat;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelConfig {
    pub name: String,
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String, // relative to workspace root
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum ChatEvent {
    Delta(String),
    ThinkingDelta(String),
    ThinkingDone(String),
    Done(String),
    Error(String),
    ToolProgress {
        turn: u32,
        max_turns: u32,
        tool: String,
        phase: String,
    },
    SearchResults(Vec<Paper>),
    #[allow(dead_code)]
    WorkspaceRoot(String),
    WorkspaceFileList(Vec<FileEntry>),
    WorkspaceFileContent {
        path: String,
        content: String,
    },
    ArtifactCreated(crate::artifact::ArtifactRecord),
    ResearchTaskUpdated(crate::research_task::ResearchTask),
}

/// 结构化工具调用轨迹，用于行为断言测试（第二层测试）。
/// `run_chat` 每执行一次工具调用就追加一条；收敛轮次追加特殊记录。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolTrace {
    pub turn: u32,
    pub tool: String,
    pub input: String,
    pub output: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRequestTiming {
    pub turn: u32,
    pub attempt_count: Option<u32>,
    pub stream_connect_ms: u64,
    pub first_reasoning_token_ms: Option<u64>,
    pub first_visible_token_ms: Option<u64>,
    pub total_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_hit_tokens: u64,
    pub cache_miss_tokens: u64,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRunSummary {
    pub iterations: u32,
    pub tool_call_count: usize,
    pub model_request_count: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub context_assembly_ms: u64,
    pub mcp_setup_ms: u64,
    pub ttft_ms: Option<u64>,
    pub ttfr_ms: Option<u64>,
    pub total_ms: u64,
    pub compaction_count: u32,
    pub stream_retry_count: u32,
    pub output_continuation_count: u32,
    pub requests: Vec<ModelRequestTiming>,
}

impl ChatRunSummary {
    pub(crate) fn absorb_usage(&mut self, usage: &Usage) {
        self.input_tokens = self
            .input_tokens
            .saturating_add(u64::from(usage.input_tokens));
        self.output_tokens = self
            .output_tokens
            .saturating_add(u64::from(usage.output_tokens));
        self.cache_creation_input_tokens = self
            .cache_creation_input_tokens
            .saturating_add(u64::from(usage.cache_creation_input_tokens));
        self.cache_read_input_tokens = self
            .cache_read_input_tokens
            .saturating_add(u64::from(usage.cache_read_input_tokens));
    }
}

pub(crate) type TraceSink = std::sync::Arc<std::sync::Mutex<Vec<ToolTrace>>>;

pub(crate) fn timing_probe(label: &str) {
    let Ok(path) = std::env::var("GALEN_TIMING_LOG") else {
        return;
    };
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default();
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{millis} {label}");
    }
}

pub(crate) fn record_trace(
    sink: &Option<TraceSink>,
    turn: u32,
    tool: String,
    input: String,
    output: String,
    is_error: bool,
) {
    if let Some(sink) = sink {
        if let Ok(mut guard) = sink.lock() {
            guard.push(ToolTrace {
                turn,
                tool,
                input,
                output,
                is_error,
            });
        }
    }
}

pub struct ChatBackend {
    pub router: ModelRouter,
    pub medical: Arc<MedicalCore>,
    pub workspace_root: Mutex<Option<PathBuf>>,
}

pub(crate) struct PendingToolCall {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) input_json: String,
}

// ---------------------------------------------------------------------------
// ChatBackend methods
// ---------------------------------------------------------------------------

impl ChatBackend {
    pub fn new() -> Self {
        let router = ModelRouter::load().unwrap_or_else(|e| {
            eprintln!("Failed to load models.toml: {e}, using defaults");
            ModelRouter::default()
        });
        let medical = Arc::new(MedicalCore::new(None));
        Self {
            router,
            medical,
            workspace_root: Mutex::new(None),
        }
    }

    pub fn all_models(&self) -> Vec<ModelConfig> {
        let default_alias = self.router.default_alias();
        let mut models = self
            .router
            .all_models()
            .iter()
            .map(|(alias, entry)| ModelConfig {
                name: alias.clone(),
                model_id: entry.model_id.clone(),
                description: entry.description.clone(),
            })
            .collect::<Vec<_>>();
        // The frontend selects the first model during startup. Keep this
        // deterministic and aligned with [router].default instead of relying
        // on HashMap iteration order.
        models.sort_by(|left, right| {
            let left_rank = usize::from(left.name != default_alias);
            let right_rank = usize::from(right.name != default_alias);
            left_rank
                .cmp(&right_rank)
                .then_with(|| left.name.cmp(&right.name))
        });
        models
    }

    pub fn resolve_model(&self, alias: &str) -> String {
        self.router.resolve_model_id(alias)
    }

    /// Set workspace root (called from UI thread)
    pub fn set_workspace_root(&self, root: Option<PathBuf>) {
        if let Ok(mut guard) = self.workspace_root.lock() {
            *guard = root;
        }
    }

    /// Get current workspace root
    pub fn get_workspace_root(&self) -> Option<PathBuf> {
        self.workspace_root.lock().ok()?.clone()
    }
}

// ---------------------------------------------------------------------------
// String interning — avoid per-call Box::leak by caching strings globally
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::Mutex as StdMutex;
static INTERN_CACHE: StdMutex<Option<HashMap<String, &'static str>>> = StdMutex::new(None);

/// Intern a String into a &'static str by storing it in a global pool.
/// The pool grows bounded by the number of unique model configs (typically 1-5),
/// so this is effectively bounded, not a leak per API call.
fn intern(s: String) -> &'static str {
    let mut guard = INTERN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(&existing) = cache.get(&s) {
        return existing;
    }
    let leaked: &'static str = Box::leak(s.clone().into_boxed_str());
    cache.insert(s, leaked);
    leaked
}

// ---------------------------------------------------------------------------
// Standalone functions (ported from ChatBackend methods)
// ---------------------------------------------------------------------------

pub fn make_client(model_alias: &str, router: &ModelRouter) -> Result<ProviderClient, String> {
    // Try model-router config first (for models.toml entries)
    if let Some(provider_config) = router.to_provider_config(model_alias) {
        if let Some(api_key) = provider_config.api_key() {
            let config = OpenAiCompatConfig {
                provider_name: intern(provider_config.provider.clone()),
                api_key_env: "",
                base_url_env: "",
                default_base_url: intern(
                    provider_config
                        .base_url
                        .clone()
                        .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
                ),
                max_request_body_bytes: 104_857_600,
            };
            // Desktop interactions must fail fast. The API crate default of
            // eight exponential retries can hide transient upstream failures
            // behind 2–4 minutes of apparent "thinking" before the first byte.
            let client = OpenAiCompatClient::new(api_key, config).with_retry_policy(
                2,
                Duration::from_millis(500),
                Duration::from_secs(2),
            );
            return Ok(ProviderClient::OpenAi(client));
        }
        return Err(format!(
            "模型 \"{model_alias}\" 缺少 API Key：请在 ~/.galen/models.toml 的 [models.{model_alias}] 中填写 api_key"
        ));
    }
    Err("未配置可用模型：请在 ~/.galen/models.toml 中配置 DeepSeek（默认建议 model_id = \"deepseek-v4-flash\"，provider = \"openai_compat\"，base_url = \"https://api.deepseek.com/v1\"），或重启应用后在欢迎向导中保存 DeepSeek API Key。"
        .to_string())
}

// ---------------------------------------------------------------------------
// History parsing (from frontend simple format)
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct HistoryEntry {
    role: String,
    content: String,
}

/// Parse a JSON array of `{role, content}` objects from the frontend
/// into `Vec<InputMessage>` suitable for the chat loop.
pub fn parse_history_json(json: &str) -> Vec<InputMessage> {
    let entries: Vec<HistoryEntry> = serde_json::from_str(json).unwrap_or_default();
    entries
        .into_iter()
        .map(|entry| InputMessage {
            role: entry.role,
            content: vec![InputContentBlock::Text {
                text: entry.content,
            }],
        })
        .collect()
}

/// Build the cache-stable system prompt.
///
/// Nothing derived from the workspace or the host environment belongs here:
/// those values change independently from the persona and would invalidate
/// DeepSeek's automatic prefix cache. Dynamic state is assembled per turn by
/// `build_turn_context` instead.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_history_json_valid() {
        let json = r#"[
            {"role": "user", "content": "hello"},
            {"role": "assistant", "content": "hi there"}
        ]"#;
        let messages = parse_history_json(json);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        match &messages[0].content[0] {
            InputContentBlock::Text { text } => assert_eq!(text, "hello"),
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn parse_history_json_empty() {
        assert!(parse_history_json("").is_empty());
        assert!(parse_history_json("[]").is_empty());
    }

    #[test]
    fn parse_history_json_malformed_returns_empty() {
        // Current behavior: silently returns empty on malformed input
        let messages = parse_history_json("not valid json");
        assert!(messages.is_empty());
    }

    #[test]
    fn run_summary_serializes_for_the_frontend_contract() {
        let summary = ChatRunSummary {
            total_ms: 1_250,
            input_tokens: 800,
            output_tokens: 120,
            cache_read_input_tokens: 600,
            ..ChatRunSummary::default()
        };
        let value = serde_json::to_value(summary).unwrap();

        assert_eq!(value["totalMs"], 1_250);
        assert_eq!(value["inputTokens"], 800);
        assert_eq!(value["outputTokens"], 120);
        assert_eq!(value["cacheReadInputTokens"], 600);
    }

    #[test]
    fn model_list_places_configured_default_first() {
        let root = std::env::temp_dir().join(format!(
            "galen-default-model-order-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("models.toml");
        std::fs::write(
            &path,
            r#"[router]
default = "deepseek-v4-flash"

[models.deepseek-v4-pro]
provider = "openai_compat"
model_id = "deepseek-v4-pro"

[models.deepseek-v4-flash]
provider = "openai_compat"
model_id = "deepseek-v4-flash"
"#,
        )
        .unwrap();
        let backend = ChatBackend {
            router: ModelRouter::load_from(&path).unwrap(),
            medical: Arc::new(MedicalCore::new(None)),
            workspace_root: Mutex::new(None),
        };

        let models = backend.all_models();
        assert_eq!(models[0].name, "deepseek-v4-flash");
        assert_eq!(models[1].name, "deepseek-v4-pro");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn archive_compaction_writes_appends_and_skips_empty() {
        let root = std::env::temp_dir().join("galen-archive-test");
        let _ = std::fs::remove_dir_all(&root);
        let ws: Mutex<Option<PathBuf>> = Mutex::new(Some(root.clone()));

        archive_compaction(&ws, "第一条摘要");
        archive_compaction(&ws, "第二条摘要");
        archive_compaction(&ws, "   "); // 空摘要不写

        let path = root.join(".galen").join("context-archive.md");
        let content = std::fs::read_to_string(&path).expect("archive file should exist");
        assert!(content.contains("第一条摘要"), "first entry present");
        assert!(content.contains("第二条摘要"), "second entry present");
        assert_eq!(
            content.matches("## 上下文压缩存档").count(),
            2,
            "no entry for empty summary"
        );

        // 无工作区时不崩溃
        let no_ws: Mutex<Option<PathBuf>> = Mutex::new(None);
        archive_compaction(&no_ws, "无工作区摘要"); // must not panic

        let _ = std::fs::remove_dir_all(&root);
    }
}
