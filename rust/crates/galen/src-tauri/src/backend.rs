use std::sync::{Arc, Mutex};

use api::{
    ContentBlockDelta, InputContentBlock, InputMessage, MessageRequest, OpenAiCompatClient,
    OpenAiCompatConfig, OutputContentBlock, ProviderClient, StreamEvent as ApiStreamEvent,
    ThinkingConfig, ToolChoice, ToolResultContentBlock,
};
use medical_core::MedicalCore;
use medical_core::types::Paper;
use model_router::ModelRouter;
use std::path::PathBuf;

use crate::tools::{ToolContext, ToolRegistry};

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
    SearchResults(Vec<Paper>),
    #[allow(dead_code)]
    WorkspaceRoot(String),
    WorkspaceFileList(Vec<FileEntry>),
    WorkspaceFileContent { path: String, content: String },
}

pub struct ChatBackend {
    pub router: ModelRouter,
    pub medical: Arc<MedicalCore>,
    pub workspace_root: Mutex<Option<PathBuf>>,
}

struct PendingToolCall {
    id: String,
    name: String,
    input_json: String,
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
        Self { router, medical, workspace_root: Mutex::new(None) }
    }

    pub fn all_models(&self) -> Vec<ModelConfig> {
        self.router
            .all_models()
            .iter()
            .map(|(alias, entry)| ModelConfig {
                name: alias.clone(),
                model_id: entry.model_id.clone(),
                description: entry.description.clone(),
            })
            .collect()
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

pub fn make_client(
    model_alias: &str,
    router: &ModelRouter,
) -> Result<ProviderClient, String> {
    // Try model-router config first (for models.toml entries)
    if let Some(provider_config) = router.to_provider_config(model_alias) {
        if let Some(api_key) = provider_config.api_key() {
            let config = OpenAiCompatConfig {
                provider_name: intern(provider_config.provider.clone()),
                api_key_env: "",
                base_url_env: "",
                default_base_url: intern(provider_config.base_url.clone()
                    .unwrap_or_else(|| "https://api.openai.com/v1".to_string())),
                max_request_body_bytes: 104_857_600,
            };
            return Ok(ProviderClient::OpenAi(OpenAiCompatClient::new(
                api_key, config,
            )));
        }
        return Err(format!(
            "模型 \"{model_alias}\" 缺少 API Key：请在 ~/.galen/models.toml 的 [models.{model_alias}] 中填写 api_key"
        ));
    }
    Err("未配置可用模型：请在 ~/.galen/models.toml 中配置 DeepSeek（provider = \"openai_compat\"，model_id = \"deepseek-v4-pro\"，base_url = \"https://api.deepseek.com/v1\"），或重启应用后在欢迎向导中保存 DeepSeek API Key。"
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

/// Build the CACHE-STABLE system prompt — loaded once per session, never changed.
/// Memory and plan instructions ride in the first user turn (turn tail) to keep
/// the prompt prefix byte-stable for DeepSeek's automatic prefix cache.
fn build_system_prompt(
    persona: &crate::personas::Persona,
    mode: crate::modes::ChatMode,
    workspace_root: &Mutex<Option<PathBuf>>,
) -> String {
    let status = crate::runtime_manager::detect_all();
    let env_summary = crate::runtime_manager::status_summary(&status);
    let mode_prompt = crate::modes::mode_prompt(mode);
    let skills = if persona.id == "medical" {
        format!(
            "{}\n\n{}",
            crate::skills::RESEARCH_TASTE,
            crate::skills::RESEARCH_SKILLS_V2
        )
    } else {
        String::new()
    };
    let ws = workspace_summary(workspace_root);
    // Stable prefix — loaded once, never mutated mid-session
    format!(
        "{}\n\n{}\n\n{}\n\n{}\n\n{}\n\n## 回复要求\n\
         每次回复前先在思考中简洁列出关键步骤（≤3步），然后将最终回答完整输出。\
         思考链控制在总回复的40%以内，确保内容部分始终可见。",
        persona.system_prompt, mode_prompt, ws, skills, env_summary,
    )
}

/// Build the dynamic first-turn tail: memory + plan instructions.
/// Injected into the FIRST user message of a session to preserve cache shape.
fn build_first_turn_tail(workspace_root: &Mutex<Option<PathBuf>>) -> String {
    let memory = load_memory(workspace_root);
    let memory_part = if memory.is_empty() {
        "\n\n## 项目记忆\n\
         工作区根目录下有 GALEN.md 文件。\
         每次完成文献检索、数据分析或得出重要结论后，请用 write_file 追加更新。\
         格式：日期 | 来源 | 关键发现 | 关联文件".to_string()
    } else {
        format!("\n\n## 项目记忆 (GALEN.md)\n{memory}\n如有新发现请用 write_file 追加。")
    };
    let plan_part = "\n\n## 科研计划\n\
        需要制定研究计划时用以下格式输出：\n\
        <!-- PLAN_START -->\n\
        编号 | 标题 | 描述 | 依赖\n\
        01 | 课题定义 | 明确研究问题 | -\n\
        <!-- PLAN_END -->\n\
        规则：`|` 分隔四个字段，编号两位数字，依赖逗号分隔。确认前询问用户。";
    format!("{memory_part}{plan_part}")
}

/// Load the GALEN.md memory file from the workspace root.
/// Returns empty string if no workspace is selected or the file doesn't exist.
fn load_memory(workspace_root: &Mutex<Option<PathBuf>>) -> String {
    let root = match workspace_root.lock().ok().and_then(|g| g.clone()) {
        Some(r) => r,
        None => return String::new(),
    };
    let path = root.join("GALEN.md");
    std::fs::read_to_string(&path).unwrap_or_default()
}

/// Build a one-paragraph summary of the current workspace for the system prompt.
fn workspace_summary(workspace_root: &Mutex<Option<PathBuf>>) -> String {
    let root = match workspace_root.lock().ok().and_then(|g| g.clone()) {
        Some(r) => r,
        None => return "工作区: 未选择。提醒用户点击顶部「选择工作区」打开项目目录。".to_string(),
    };

    let mut lines = vec![format!("工作区: {}", root.display())];
    if let Ok(entries) = std::fs::read_dir(&root) {
        let mut files = 0u32;
        let mut dirs = 0u32;
        let mut has_cargo = false;
        let mut has_git = false;
        let mut has_package_json = false;

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                dirs += 1;
            } else {
                files += 1;
            }
            match name.as_str() {
                "Cargo.toml" => has_cargo = true,
                "package.json" => has_package_json = true,
                ".git" => has_git = true,
                _ => {}
            }
        }

        lines.push(format!("共 {dirs} 个目录, {files} 个文件"));
        if has_cargo { lines.push("构建系统: Cargo (Rust)".into()); }
        if has_package_json { lines.push("构建系统: npm (Node.js)".into()); }
        if has_git { lines.push("Git 仓库: 是".into()); }
    }

    lines.join("\n")
}

// ---------------------------------------------------------------------------
// API error formatting
// ---------------------------------------------------------------------------

fn format_api_error(e: &dyn std::error::Error) -> String {
    let msg = e.to_string().to_lowercase();
    let category = if msg.contains("401") || msg.contains("unauthorized") {
        "API Key 无效或已过期。请检查 ~\\.galen\\models.toml 中的 api_key。"
    } else if msg.contains("403") || msg.contains("forbidden") || msg.contains("disabled") {
        "接口访问被拒绝。可能原因：API Key 权限不足、接口已禁用、或账户欠费。"
    } else if msg.contains("429") || msg.contains("rate") {
        "请求频率过高，请稍后重试。"
    } else if msg.contains("500") || msg.contains("503") || msg.contains("unavailable") {
        "模型服务暂时不可用，请稍后重试。"
    } else if msg.contains("timeout") || msg.contains("timed out") {
        "请求超时。请检查网络连接后重试。"
    } else if msg.contains("connect") || msg.contains("resolve") || msg.contains("dns") {
        "无法连接到 API 服务器。请检查网络或 base_url 配置。"
    } else {
        "API 请求失败，请稍后重试。"
    };
    format!("{category}\n\n详细信息: {e}\n配置路径: ~\\.galen\\models.toml")
}

// ---------------------------------------------------------------------------
// Main chat loop
// ---------------------------------------------------------------------------

pub async fn run_chat<F: Fn(ChatEvent) + Send + Sync + 'static>(
    model_alias: String,
    model_id: String,
    user_message: String,
    history: Vec<InputMessage>,
    mode: crate::modes::ChatMode,
    persona: crate::personas::Persona,
    thinking_level: String,
    medical: Arc<MedicalCore>,
    router: ModelRouter,
    workspace_root: Mutex<Option<PathBuf>>,
    on_event: F,
) -> Result<(), String> {
    let client = make_client(&model_alias, &router)
        .map_err(|e| format!("创建模型客户端失败: {e}"))?;

    // Map user-facing thinking intensity to provider params.
    // DeepSeek V4 (and compatible reasoning models) accept low/medium/high;
    // "off" disables reasoning entirely. Unknown values fall back to medium.
    let (reasoning_effort, thinking) = match thinking_level.as_str() {
        "off" => (None, None),
        "high" => (
            Some("high".to_string()),
            Some(ThinkingConfig {
                thinking_type: "enabled".to_string(),
            }),
        ),
        "low" => (
            Some("low".to_string()),
            Some(ThinkingConfig {
                thinking_type: "enabled".to_string(),
            }),
        ),
        _ => (
            Some("medium".to_string()),
            Some(ThinkingConfig {
                thinking_type: "enabled".to_string(),
            }),
        ),
    };

    // Build cache-stable prefix
    let system_prompt = build_system_prompt(&persona, mode, &workspace_root);
    // Dynamic tail injected into first user message (memory + plan instructions)
    let turn_tail = build_first_turn_tail(&workspace_root);

    let mut history = history; // mutable copy
    // First turn: inject memory + plan instructions as turn tail, not in system prompt
    let first_user_text = if history.is_empty() {
        format!("{turn_tail}\n\n---\n\n用户: {user_message}")
    } else {
        user_message.clone()
    };
    history.push(InputMessage {
        role: "user".to_string(),
        content: vec![InputContentBlock::Text {
            text: first_user_text,
        }],
    });

    // Build tool registry and shared context
    let mut registry = ToolRegistry::default();
    // Cache MCP connections globally — connect once, reuse across turns.
    {
        use std::sync::OnceLock;
        static MCP_CACHE: OnceLock<Vec<Arc<tokio::sync::Mutex<crate::mcp_client::McpServer>>>> =
            OnceLock::new();
        if let Some(cached) = MCP_CACHE.get() {
            registry.load_mcp_from_cache(cached.clone());
        } else {
            registry.connect_mcp_servers().await;
            let servers = registry.mcp.servers().to_vec();
            if MCP_CACHE.set(servers).is_err() {
                eprintln!("[galen] mcp: cache already set, reusing existing connections");
            }
        }
    }
    let on_event: Arc<dyn Fn(ChatEvent) + Send + Sync> = Arc::new(on_event);
    let mut ctx = ToolContext::with_event_sender(medical.clone(), workspace_root, on_event.clone());
    ctx.mode = mode;

    // ── Auto-compaction knob ──
    const COMPACT_CHAR_LIMIT: usize = 24_000; // ~6K tokens — compact when history exceeds this
    const KEEP_HEAD: usize = 2; // keep first N messages (context)
    const KEEP_TAIL: usize = 6; // keep last N messages (recent)

    // Multi-turn loop: keep going until model responds with text (no tool calls)
    let mut turn = 0;
    let max_turns = 10;
    let mut last_tool_name: Option<String> = None;
    let mut same_tool_streak: u32 = 0;
    let mut final_chance_used = false;
    let mut compacted = false;
    loop {
        turn += 1;
        if turn > max_turns {
            if final_chance_used {
                on_event(ChatEvent::Error("Reached max tool-call turns".into()));
                break;
            }
            // Smart termination: give one more turn with a hint to summarize
            final_chance_used = true;
        }

        // ── Auto-compaction: fold middle when context grows too large ──
        if !compacted {
            let total_chars: usize = history.iter()
                .map(|m| m.content.iter().map(|b| match b {
                    InputContentBlock::Text { text } => text.len(),
                    InputContentBlock::Thinking { thinking, .. } => thinking.len(),
                    _ => 0,
                }).sum::<usize>())
                .sum();
            if total_chars > COMPACT_CHAR_LIMIT && history.len() > KEEP_HEAD + KEEP_TAIL + 2 {
                let folded = history.len() - KEEP_HEAD - KEEP_TAIL;
                let head = history.drain(..KEEP_HEAD).collect::<Vec<_>>();
                let _middle = history.drain(..folded).collect::<Vec<_>>(); // dropped
                let tail = std::mem::take(&mut history);
                // Rebuild: head + compact placeholder + tail
                history = head;
                history.push(InputMessage {
                    role: "user".to_string(),
                    content: vec![InputContentBlock::Text {
                        text: format!("[上下文已折叠: {folded} 条较早的消息被移除，保留最近 {KEEP_TAIL} 条。如需之前的信息请询问用户。]"),
                    }],
                });
                history.extend(tail);
                compacted = true;
                on_event(ChatEvent::Delta("[上下文已自动压缩]\n".to_string()));
            }
        }

        let tools = registry.all_definitions().await;

        let max_tokens = router
            .all_models()
            .get(&model_alias)
            .and_then(|entry| entry.max_tokens)
            .unwrap_or(4096) as u32;

        let request = MessageRequest {
            model: model_id.clone(),
            messages: history.clone(),
            max_tokens,
            system: Some(system_prompt.clone()),
            tools: Some(tools.clone()),
            tool_choice: Some(ToolChoice::Auto),
            stream: true,
            reasoning_effort: reasoning_effort.clone(),
            thinking: thinking.clone(),
            ..Default::default()
        };

        let mut stream = client
            .stream_message(&request)
            .await
            .map_err(|e| format_api_error(&e))?;

        // Collect content blocks for this response
        let mut text_blocks: Vec<String> = Vec::new();
        let mut tool_calls: Vec<PendingToolCall> = Vec::new();
        let mut current_tool: Option<PendingToolCall> = None;
        let mut current_text: String = String::new();
        let mut current_thinking: String = String::new();

        loop {
            match stream.next_event().await {
                Ok(Some(ApiStreamEvent::ContentBlockStart(event))) => {
                    match event.content_block {
                        OutputContentBlock::Text { text } => {
                            // Only initialize the accumulator — don't emit.
                            // ContentBlockDelta events carry the actual streamed text.
                            current_text = text;
                        }
                        OutputContentBlock::ToolUse { id, name, .. } => {
                            // Flush any text/thinking before starting tool call
                            if !current_text.is_empty() {
                                text_blocks.push(std::mem::take(&mut current_text));
                            }
                            current_tool = Some(PendingToolCall {
                                id,
                                name,
                                input_json: String::new(),
                            });
                        }
                        OutputContentBlock::Thinking { thinking, .. } => {
                            on_event(ChatEvent::ThinkingDelta(thinking.clone()));
                            current_thinking = thinking;
                        }
                        _ => {}
                    }
                }
                Ok(Some(ApiStreamEvent::ContentBlockDelta(event))) => {
                    match event.delta {
                        ContentBlockDelta::TextDelta { text } => {
                            current_text.push_str(&text);
                            on_event(ChatEvent::Delta(text));
                        }
                        ContentBlockDelta::InputJsonDelta { partial_json } => {
                            if let Some(ref mut tool) = current_tool {
                                tool.input_json.push_str(&partial_json);
                            }
                        }
                        ContentBlockDelta::ThinkingDelta { thinking } => {
                            on_event(ChatEvent::ThinkingDelta(thinking.clone()));
                            current_thinking.push_str(&thinking);
                        }
                        _ => {}
                    }
                }
                Ok(Some(ApiStreamEvent::ContentBlockStop(_))) => {
                    // Emit ThinkingDone event (clone so original stays for history)
                    if !current_thinking.is_empty() {
                        on_event(ChatEvent::ThinkingDone(current_thinking.clone()));
                    }
                    // Finish current tool call
                    if let Some(tool) = current_tool.take() {
                        if !tool.input_json.is_empty() {
                            tool_calls.push(tool);
                        }
                    }
                }
                Ok(Some(ApiStreamEvent::MessageStop(_))) => {
                    if !current_text.is_empty() {
                        text_blocks.push(std::mem::take(&mut current_text));
                    }
                    break;
                }
                Ok(None) => break,
                Err(e) => {
                    on_event(ChatEvent::Error(format!("stream error: {e}")));
                    return Ok(());
                }
                _ => {}
            }
        }

        // Build assistant message from collected blocks
        let mut assistant_content: Vec<InputContentBlock> = Vec::new();
        // Reasoning / chain-of-thought must be included so it can be
        // round-tripped back to the API on subsequent turns (DeepSeek V4).
        if !current_thinking.is_empty() {
            assistant_content.push(InputContentBlock::Thinking {
                thinking: std::mem::take(&mut current_thinking),
                signature: None,
            });
        }
        for text in &text_blocks {
            assistant_content.push(InputContentBlock::Text {
                text: text.clone(),
            });
        }
        for tool in &tool_calls {
            let input: serde_json::Value = serde_json::from_str(&tool.input_json)
                .unwrap_or(serde_json::Value::Null);
            assistant_content.push(InputContentBlock::ToolUse {
                id: tool.id.clone(),
                name: tool.name.clone(),
                input,
            });
        }
        if !assistant_content.is_empty() {
            history.push(InputMessage {
                role: "assistant".to_string(),
                content: assistant_content,
            });
        }

        // If no tool calls, this is the final text response
        if tool_calls.is_empty() {
            let full_text = text_blocks.join("");
            on_event(ChatEvent::Done(full_text));
            break;
        }

        // Execute tools and build result message
        let mut tool_results: Vec<InputContentBlock> = Vec::new();
        for tool in &tool_calls {
            // --- stagnation detection ---
            if let Some(ref last_name) = last_tool_name {
                if *last_name == tool.name {
                    same_tool_streak += 1;
                } else {
                    same_tool_streak = 0;
                }
            }
            last_tool_name = Some(tool.name.clone());

            let input: serde_json::Value =
                serde_json::from_str(&tool.input_json).unwrap_or(serde_json::Value::Null);
            let result = registry.execute_dynamic(&tool.name, input, &ctx).await;
            let is_error = result.is_err();
            let text = result.unwrap_or_else(|e| e);

            tool_results.push(InputContentBlock::ToolResult {
                tool_use_id: tool.id.clone(),
                content: vec![ToolResultContentBlock::Text { text }],
                is_error,
            });
        }

        // --- result validation hints ---
        // Check for consecutive empty results
        if same_tool_streak >= 3 {
            let hint_name = last_tool_name.as_deref().unwrap_or("unknown");
            tool_results.push(InputContentBlock::ToolResult {
                tool_use_id: "__hint_stagnation__".to_string(),
                content: vec![ToolResultContentBlock::Text {
                    text: format!(
                        "[系统提示] 工具 `{hint_name}` 已连续调用 {same_tool_streak} 次。\
                         请尝试不同的方法或工具来完成任务，例如更换搜索关键词、调整查询策略、\
                         或使用其他工具获取信息。"
                    ),
                }],
                is_error: false,
            });
            same_tool_streak = 0;
        }

        // --- smart termination hint ---
        if final_chance_used {
            tool_results.push(InputContentBlock::ToolResult {
                tool_use_id: "__hint_final__".to_string(),
                content: vec![ToolResultContentBlock::Text {
                    text: "[系统提示] 已达到最大轮次限制。请基于已获取的所有信息，\
                         给出当前最好的完整回答，不要再调用更多工具。".to_string(),
                }],
                is_error: false,
            });
        }

        history.push(InputMessage {
            role: "user".to_string(),
            content: tool_results,
        });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
}
