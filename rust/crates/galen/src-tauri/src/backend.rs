use std::sync::{Arc, Mutex};

use api::{
    ContentBlockDelta, InputContentBlock, InputMessage, MessageRequest, OpenAiCompatClient,
    OpenAiCompatConfig, OutputContentBlock, ProviderClient, StreamEvent as ApiStreamEvent,
    ThinkingConfig, ToolChoice, ToolDefinition, ToolResultContentBlock, Usage,
};
use medical_core::types::Paper;
use medical_core::MedicalCore;
use model_router::ModelRouter;
use std::path::PathBuf;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

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
    WorkspaceFileContent {
        path: String,
        content: String,
    },
}

/// 结构化工具调用轨迹，用于行为断言测试（第二层测试）。
/// `run_chat` 每执行一次工具调用就追加一条；收敛轮次追加特殊记录。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolTrace {
    pub turn: u32,
    pub tool: String,
    pub input: String,
    pub output: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRunSummary {
    pub iterations: u32,
    pub tool_call_count: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
}

impl ChatRunSummary {
    fn absorb_usage(&mut self, usage: &Usage) {
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

type TraceSink = std::sync::Arc<std::sync::Mutex<Vec<ToolTrace>>>;

fn timing_probe(label: &str) {
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

fn record_trace(
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
        Self {
            router,
            medical,
            workspace_root: Mutex::new(None),
        }
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

/// Build the cache-stable system prompt.
///
/// Nothing derived from the workspace or the host environment belongs here:
/// those values change independently from the persona and would invalidate
/// DeepSeek's automatic prefix cache. Dynamic state is assembled per turn by
/// `build_turn_context` instead.
fn build_system_prompt(persona: &crate::personas::Persona, mode: crate::modes::ChatMode) -> String {
    let mode_prompt = crate::modes::mode_prompt(mode);
    // L0 常驻核心：人格 + 科研品味 + 模式。只保留真正稳定的内容。
    let taste = if persona.id == "medical" {
        crate::skills::RESEARCH_TASTE
    } else {
        ""
    };
    format!(
        "{}\n\n{}\n\n{}\n\n## 回复要求\n\
         优先行动并报告可验证结果。不要复述内部推理；需要说明决策时只给简短依据。\
         达到当前任务的验收条件后立即收敛输出。",
        persona.system_prompt, taste, mode_prompt,
    )
}

/// Assemble the dynamic context for the current turn.
///
/// L1 skills follow the current intent. L2 workspace/task/evidence state is
/// refreshed on every turn so a durable flow-back becomes visible immediately,
/// even when the frontend still has conversational history. Session-opening
/// instructions are included only once.
fn build_turn_context(
    user_message: &str,
    mode: crate::modes::ChatMode,
    workspace_root: &Mutex<Option<PathBuf>>,
    first_turn: bool,
) -> String {
    // L1：按任务意图装配技能模块
    let task_kind = model_router::TaskKind::from_intent(user_message);
    let skills = crate::skills::assemble_skills_for_intent(task_kind, user_message);
    // L2：项目画像
    let plan = plan_progress_summary(workspace_root);
    let memory = memory_index(workspace_root);
    let evidence = workspace_root_path(workspace_root)
        .map(|root| crate::evidence::evidence_chain_summary(&root, 8))
        .unwrap_or_default();
    let resume = if first_turn {
        resume_protocol(workspace_root)
    } else {
        String::new()
    };
    let status = crate::runtime_manager::detect_all();
    let env_summary = crate::runtime_manager::status_summary(&status);
    let workspace = workspace_summary(workspace_root);
    let wants_plan = matches!(mode, crate::modes::ChatMode::Plan)
        || user_message.contains("计划")
        || user_message.contains("方案")
        || user_message.contains("拆解");
    let plan_format = if wants_plan {
        let ending = if matches!(mode, crate::modes::ChatMode::Plan) {
            "生成后等待用户确认。"
        } else {
            "自动模式下生成后直接执行，不等待确认。"
        };
        format!(
            "\n\n## 科研计划格式\n\
             仅当本任务确实需要多节点计划时输出：\n\
             <!-- PLAN_START -->\n\
             编号 | 标题 | 描述 | 依赖\n\
             01 | 课题定义 | 明确研究问题 | -\n\
             <!-- PLAN_END -->\n\
             规则：`|` 分隔四个字段，编号两位数字，依赖逗号分隔。{ending}"
        )
    } else {
        String::new()
    };
    let opening = if !first_turn {
        String::new()
    } else if matches!(mode, crate::modes::ChatMode::Auto) {
        "\n\n## 自动执行协议\n\
         直接完成用户目标。先用最少的只读操作确认输入，再执行、验证并交付；\
         不复述工作区清单，不输出冗长计划，不请求批准。\n\
         需要产出文件时，验证文件存在且非空后立即结束。"
            .to_string()
    } else {
        format!("\n\n## 会话开局（本轮对话开始前先完成）\n\
         1. 用一句话陈述本次任务目标。\n\
         2. 陈述当前工作区状态（哪些证据/计划/记忆已存在，哪些缺失）。\n\
         3. 列出本次任务的收尾标准（什么样算完成、交付什么）。\n\
         4. 然后再开始执行。\n\
         小心：不要复述全部记忆/列表；只需在回答里体现\"我理解了哪些状态、接下来做什么\"，一段话即可。\n\
         \n\
         ## 交付质量门（必须遵守）\n\
         任务要求产出文件（PDF / 报告 / 图）时：写完源文件后必须执行编译或验证命令，确认产物存在且非空后再汇报；\n\
         编译失败必须修复后重试，禁止交付未验证的产物。\n\
         写 Typst 时：比较符号前后加空格（如 p < 0.05），避免 < 被当作标签；强调用 *...* 且必须闭合；\n\
         写完 .typ 后立即用 typst compile 验证，报错则修复重试。")
    };
    format!(
        "{opening}{skills}\n\n## 当前工作区\n{workspace}\n\n## 当前科研环境\n{env_summary}\n\n{plan}\n\n{memory}{evidence}{resume}{plan_format}"
    )
}

/// Keep tool evidence useful without replaying unbounded logs or entire data
/// files into every subsequent model request.
fn compact_tool_result(tool_name: &str, text: &str, is_error: bool) -> String {
    const LIMIT: usize = 8_000;
    const HEAD: usize = 5_500;
    const TAIL: usize = 1_500;
    if text.chars().count() <= LIMIT {
        return text.to_string();
    }
    let head: String = text.chars().take(HEAD).collect();
    let tail: String = text
        .chars()
        .rev()
        .take(TAIL)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!(
        "[工具结果已压缩] tool={tool_name} error={is_error} original_chars={}\n\
         --- 前部 ---\n{head}\n\
         --- 省略 {} 字符；如需细节请按文件路径或行范围读取 ---\n\
         --- 尾部 ---\n{tail}",
        text.chars().count(),
        text.chars().count().saturating_sub(HEAD + TAIL),
    )
}

/// Input compaction is independent from the model's response `max_tokens`.
/// DeepSeek currently exposes a much larger input window than the desired
/// response size, so coupling the two causes premature summarization and an
/// avoidable extra model request. The environment override is primarily for
/// probes and constrained providers.
fn compact_trigger_bytes() -> usize {
    const DEFAULT_TRIGGER_TOKENS: usize = 72_000;
    const BYTES_PER_TOKEN_ESTIMATE: usize = 3;
    let tokens = std::env::var("GALEN_COMPACT_TRIGGER_TOKENS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_TRIGGER_TOKENS)
        .clamp(8_000, 120_000);
    tokens.saturating_mul(BYTES_PER_TOKEN_ESTIMATE)
}

fn select_tools_for_task(
    mut tools: Vec<ToolDefinition>,
    kind: model_router::TaskKind,
    user_message: &str,
) -> Vec<ToolDefinition> {
    let lower = user_message.to_lowercase();
    let data_task = is_local_data_task(&lower);
    let literature_task = ["文献", "pubmed", "综述", "证据", "检索"]
        .iter()
        .any(|needle| lower.contains(needle));
    let allowed: Option<&[&str]> = if data_task && !literature_task {
        Some(&[
            "list_files",
            "read_file",
            "search_files",
            "create_directory",
            "write_file",
            "execute_command",
        ])
    } else if matches!(kind, model_router::TaskKind::QuickLookup) || literature_task {
        Some(&[
            "search_pubmed",
            "fetch_article",
            "format_citation",
            "list_files",
            "read_file",
            "search_files",
            "save_paper",
            "write_file",
        ])
    } else {
        None
    };
    if let Some(allowed) = allowed {
        tools.retain(|tool| allowed.contains(&tool.name.as_str()));
    }
    tools
}

fn is_local_data_task(text: &str) -> bool {
    let lower = text.to_lowercase();
    ["数据", "肌电", "emg", "csv", "统计", "回归", "脚本"]
        .iter()
        .any(|needle| lower.contains(needle))
}

/// 读取工作区根目录；无工作区返回 None。
fn workspace_root_path(workspace_root: &Mutex<Option<PathBuf>>) -> Option<PathBuf> {
    workspace_root.lock().ok().and_then(|g| g.clone())
}

/// 记忆索引：短记忆全文注入；长记忆只注入最近记录 + 总量（全文按需读取）。
fn memory_index(workspace_root: &Mutex<Option<PathBuf>>) -> String {
    let Some(root) = workspace_root_path(workspace_root) else {
        return String::new();
    };
    let text = std::fs::read_to_string(root.join("GALEN.md")).unwrap_or_default();
    if text.trim().is_empty() {
        return "\n\n## 项目记忆\n\
            工作区根目录下有 GALEN.md 文件。\
            每次完成文献检索、数据分析或得出重要结论后，请用 write_file 追加更新。\
            格式：日期 | 来源 | 关键发现 | 关联文件"
            .to_string();
    }
    if text.len() <= 1600 {
        return format!("\n\n## 项目记忆 (GALEN.md)\n{text}\n如有新发现请用 write_file 追加。");
    }
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let total = lines.len();
    let recent_lines: Vec<&str> = lines.iter().rev().take(5).copied().collect();
    let recent_count = recent_lines.len();
    let recent_text = recent_lines
        .iter()
        .rev()
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "\n\n## 项目记忆 (GALEN.md)\n共 {} 条记录，最近 {} 条：\n{recent_text}\n如需更早记录可要求读取 GALEN.md 全文。",
        total,
        recent_count
    )
}

/// 计划进度摘要：从宿主权威 ResearchTask 生成结构化进度。
fn plan_progress_summary(workspace_root: &Mutex<Option<PathBuf>>) -> String {
    let Some(root) = workspace_root_path(workspace_root) else {
        return "\n\n## 科研计划进度\n未选择工作区，暂无计划。".to_string();
    };
    let task = match crate::research_task::load_or_migrate_active_task(&root) {
        Ok(Some(task)) => task,
        Ok(None) => return "\n\n## 科研计划进度\n暂无已确认的科研计划。".to_string(),
        Err(error) => return format!("\n\n## 科研计划进度\n任务状态不可读：{error}"),
    };
    let nodes = task.nodes;
    if nodes.is_empty() {
        return "\n\n## 科研计划进度\n暂无已确认的科研计划。".to_string();
    }
    let total = nodes.len();
    let done = nodes.iter().filter(|n| n.status == "completed").count();
    let mut out = format!(
        "\n\n## 科研计划进度：{}（{done}/{total} 完成，任务状态 {:?}）",
        task.title, task.status
    );
    for n in nodes
        .iter()
        .filter(|n| n.status == "running" || n.status == "pending")
        .take(4)
    {
        out.push_str(&format!("\n- 待执行：{}（{}）", n.title, n.status));
    }
    for n in nodes
        .iter()
        .filter(|n| n.status == "completed")
        .rev()
        .take(2)
    {
        if let Some(r) = &n.result {
            let snippet: String = r.chars().take(80).collect();
            out.push_str(&format!("\n- 已产出：{} → {}", n.title, snippet));
        }
    }
    out
}

/// 任务恢复协议：如果存在进行中的宿主任务，提醒模型先交代【已做到哪、接着做什么】，
/// 再继续执行，而不是从零重新开始或假装不知道现状。
fn resume_protocol(workspace_root: &Mutex<Option<PathBuf>>) -> String {
    let Some(root) = workspace_root_path(workspace_root) else {
        return String::new();
    };
    let Ok(Some(task)) = crate::research_task::load_or_migrate_active_task(&root) else {
        return String::new();
    };
    let nodes = task.nodes;
    let active: Vec<&crate::research_task::ResearchNode> = nodes
        .iter()
        .filter(|n| n.status == "running" || n.status == "pending" || n.status == "blocked")
        .collect();
    if active.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n\n## 任务恢复协议（重要）\n");
    out.push_str("检测到已有未完成的研究计划。先交代现状，再继续：\n");
    out.push_str("- 上一轮已完成：列出当前 ResearchTask 中 completed 节点的结果摘要（若有）\n");
    out.push_str("- 接下来要做：明确说出第一个未完成节点及其依赖\n");
    out.push_str("- 然后直接继续执行，不要重新询问用户是否开始\n");
    for n in active.iter().take(3) {
        out.push_str(&format!("  - 未完成：{}（{}）\n", n.title, n.status));
    }
    out
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
        if has_cargo {
            lines.push("构建系统: Cargo (Rust)".into());
        }
        if has_package_json {
            lines.push("构建系统: npm (Node.js)".into());
        }
        if has_git {
            lines.push("Git 仓库: 是".into());
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod context_tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_ws(tag: &str, files: &[(&str, &str)]) -> Mutex<Option<PathBuf>> {
        let dir =
            std::env::temp_dir().join(format!("galen_ctx_test_{}_{}", std::process::id(), tag));
        let _ = std::fs::create_dir_all(&dir);
        for (name, content) in files {
            std::fs::write(dir.join(name), content).unwrap();
        }
        Mutex::new(Some(dir))
    }

    #[test]
    fn memory_short_is_injected_fully() {
        let ws = tmp_ws(
            "short",
            &[("GALEN.md", "2026-08-12 | 检索 | 发现 A | plan.json")],
        );
        let idx = memory_index(&ws);
        assert!(idx.contains("发现 A"));
        assert!(!idx.contains("共 "));
    }

    #[test]
    fn memory_long_is_indexed() {
        let mut lines = String::new();
        for i in 0..60 {
            lines.push_str(&format!("2026-08-12 | 记录{i} | 关键发现{i} | plan.json\n"));
        }
        let ws = tmp_ws("long", &[("GALEN.md", &lines)]);
        let idx = memory_index(&ws);
        assert!(idx.contains("共 60 条记录，最近 5 条"));
        assert!(idx.contains("关键发现59"));
        assert!(!idx.contains("关键发现0"), "旧记录不应全量注入");
    }

    #[test]
    fn plan_summary_reports_progress() {
        let plan = r#"[
            {"id":"s01","index":"01","title":"课题定义","status":"completed","result":"明确了 PICO"},
            {"id":"s02","index":"02","title":"文献检索","status":"pending"}
        ]"#;
        let ws = tmp_ws("plan", &[("plan.json", plan)]);
        let s = plan_progress_summary(&ws);
        assert!(s.contains("1/2 完成"));
        assert!(s.contains("待执行：文献检索"));
        assert!(s.contains("已产出：课题定义"));
    }

    #[test]
    fn plan_summary_uses_active_task_after_legacy_migration() {
        let legacy = r#"[
            {"id":"s01","index":"01","title":"数据质检","type":"data","status":"pending"}
        ]"#;
        let ws = tmp_ws("task_authority", &[("plan.json", legacy)]);
        let root = workspace_root_path(&ws).unwrap();

        // First read migrates the legacy source into the host task store.
        let initial = plan_progress_summary(&ws);
        assert!(initial.contains("0/1 完成"));
        let task = crate::research_task::load_active_task(&root)
            .unwrap()
            .unwrap();
        let mut completed = task.nodes;
        completed[0].status = "completed".to_string();
        completed[0].result = Some("缺失率已验证".to_string());
        crate::research_task::replace_nodes(&root, &task.task_id, task.revision, completed)
            .unwrap();

        // The untouched root plan still says pending; it must no longer win.
        assert!(std::fs::read_to_string(root.join("plan.json"))
            .unwrap()
            .contains("pending"));
        let current = plan_progress_summary(&ws);
        assert!(current.contains("1/1 完成"));
        assert!(current.contains("缺失率已验证"));
    }

    #[test]
    fn plan_summary_empty_when_no_plan() {
        let ws = tmp_ws("noplan", &[]);
        let s = plan_progress_summary(&ws);
        assert!(s.contains("暂无已确认的科研计划"));
    }

    #[test]
    fn tail_assembles_skills_and_context() {
        let ws = tmp_ws("tail", &[]);
        let tail = build_turn_context(
            "请检索康复运动干预的 RCT 证据",
            crate::modes::ChatMode::Auto,
            &ws,
            true,
        );
        assert!(tail.contains("模块 B"));
        assert!(tail.contains("科研计划进度"));
        assert!(tail.contains("GALEN.md"));
        assert!(!tail.contains("确认前询问用户"));
    }

    #[test]
    fn auto_tail_only_adds_plan_protocol_for_plan_intent() {
        let ws = tmp_ws("intent_tail", &[]);
        let direct =
            build_turn_context("分析这批肌电数据", crate::modes::ChatMode::Auto, &ws, true);
        assert!(!direct.contains("PLAN_START"));
        let planned = build_turn_context(
            "请制定肌电分析方案",
            crate::modes::ChatMode::Auto,
            &ws,
            true,
        );
        assert!(planned.contains("PLAN_START"));
        assert!(planned.contains("不等待确认"));
    }

    #[test]
    fn compact_tool_result_preserves_head_tail_and_bounds_context() {
        let long = format!("HEAD{}TAIL", "x".repeat(10_000));
        let compacted = compact_tool_result("execute_command", &long, false);
        assert!(compacted.contains("HEAD"));
        assert!(compacted.contains("TAIL"));
        assert!(compacted.contains("工具结果已压缩"));
        assert!(compacted.chars().count() < long.chars().count());
    }

    #[test]
    fn compaction_budget_is_not_derived_from_response_limit() {
        // The default threshold should leave ample room in a 128K input window
        // and must not collapse to the usual 2K/4K response budget.
        assert!(compact_trigger_bytes() >= 72_000 * 3);
    }

    #[test]
    fn data_task_only_exposes_execution_tools() {
        let defs = vec![
            ToolDefinition {
                name: "list_files".into(),
                description: None,
                input_schema: serde_json::json!({}),
            },
            ToolDefinition {
                name: "execute_command".into(),
                description: None,
                input_schema: serde_json::json!({}),
            },
            ToolDefinition {
                name: "search_pubmed".into(),
                description: None,
                input_schema: serde_json::json!({}),
            },
            ToolDefinition {
                name: "mcp__unrelated".into(),
                description: None,
                input_schema: serde_json::json!({}),
            },
        ];
        let selected = select_tools_for_task(
            defs,
            model_router::TaskKind::DeepAnalysis,
            "分析这批肌电数据",
        );
        let names: Vec<&str> = selected.iter().map(|tool| tool.name.as_str()).collect();
        assert_eq!(names, vec!["list_files", "execute_command"]);
    }

    #[test]
    fn auto_opening_is_compact_and_action_oriented() {
        let ws = tmp_ws("auto_opening", &[]);
        let tail = build_turn_context("分析肌电数据", crate::modes::ChatMode::Auto, &ws, true);
        assert!(tail.contains("自动执行协议"));
        assert!(!tail.contains("会话开局"));
        assert!(!tail.contains("模块 B"));
    }

    #[test]
    fn stable_system_prompt_does_not_include_workspace_or_environment() {
        let persona = crate::personas::find_persona("medical");
        let prompt = build_system_prompt(&persona, crate::modes::ChatMode::Auto);
        assert!(!prompt.contains("工作区:"));
        assert!(!prompt.contains("科研环境"));
        assert!(!prompt.contains("Cargo"));
    }

    #[test]
    fn later_turn_refreshes_state_without_repeating_opening_protocol() {
        let ws = tmp_ws(
            "later_turn",
            &[("GALEN.md", "2026-08-22 | 分析 | 新结论 | result.md")],
        );
        let context = build_turn_context("继续分析", crate::modes::ChatMode::Auto, &ws, false);
        assert!(context.contains("新结论"));
        assert!(context.contains("当前工作区"));
        assert!(!context.contains("自动执行协议"));
        assert!(!context.contains("任务恢复协议"));
    }
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
    trace: Option<TraceSink>,
    on_event: F,
) -> Result<ChatRunSummary, String> {
    timing_probe("run_chat:start");
    let client =
        make_client(&model_alias, &router).map_err(|e| format!("创建模型客户端失败: {e}"))?;
    timing_probe("run_chat:client_ready");

    // Map user-facing thinking intensity to provider params.
    // DeepSeek V4 (and compatible reasoning models) accept low/medium/high;
    // "off" disables reasoning entirely. Unknown values fall back to medium.
    let (reasoning_effort, thinking) = match thinking_level.as_str() {
        "off" => (
            None,
            Some(ThinkingConfig {
                thinking_type: "disabled".to_string(),
            }),
        ),
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
    let system_prompt = build_system_prompt(&persona, mode);
    // Dynamic state is refreshed every turn while the cache-stable prefix stays unchanged.
    let first_turn = history.is_empty();
    let turn_context = build_turn_context(&user_message, mode, &workspace_root, first_turn);
    timing_probe("run_chat:context_ready");
    let task_kind = model_router::TaskKind::from_intent(&user_message);

    let mut history = history; // mutable copy
                               // The frontend stores the user's raw text, so adding a fresh context envelope
                               // here does not accumulate duplicate snapshots in later requests.
    let first_user_text = format!("{turn_context}\n\n---\n\n用户: {user_message}");
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
        timing_probe("run_chat:mcp_start");
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
        timing_probe("run_chat:mcp_ready");
    }
    let on_event: Arc<dyn Fn(ChatEvent) + Send + Sync> = Arc::new(on_event);
    let mut ctx = ToolContext::with_event_sender(medical.clone(), workspace_root, on_event.clone());
    ctx.mode = mode;

    // ── Auto-compaction knob ──
    const KEEP_HEAD: usize = 2; // keep first N messages (context)
    const KEEP_TAIL: usize = 6; // keep last N messages (recent)
    const MAX_COMPACTIONS: u32 = 3; // 长会话可多次压缩，旧摘要自动并入新摘要

    // Multi-turn loop: keep going until model responds with text (no tool calls)
    let mut turn = 0;
    let max_turns = 28;
    let mut last_tool_name: Option<String> = None;
    let mut same_tool_streak: u32 = 0;
    let mut con_error_streak: u32 = 0;
    let mut final_chance_used = false;
    let mut final_turn = false;
    let mut compaction_count: u32 = 0;
    let mut run_summary = ChatRunSummary::default();
    // Input compaction has its own budget. `max_tokens` below is the response
    // budget and must not be used as a proxy for the provider context window.
    let max_tokens = router
        .all_models()
        .get(&model_alias)
        .and_then(|entry| entry.max_tokens)
        .unwrap_or(4096) as u32;
    let compact_limit = compact_trigger_bytes();
    loop {
        turn += 1;
        if turn > max_turns {
            if final_chance_used {
                on_event(ChatEvent::Error("Reached max tool-call turns".into()));
                break;
            }
            // Smart termination: the next (final) turn is stripped of tools,
            // so the model can only produce the closing answer.
            final_chance_used = true;
            final_turn = true;
        }

        // ── Auto-compaction: fold middle when context grows too large ──
        if compaction_count < MAX_COMPACTIONS {
            let total_bytes: usize = history
                .iter()
                .map(|m| {
                    m.content
                        .iter()
                        .map(|b| match b {
                            InputContentBlock::Text { text } => text.len(),
                            InputContentBlock::Thinking { thinking, .. } => thinking.len(),
                            _ => 0,
                        })
                        .sum::<usize>()
                })
                .sum();
            if total_bytes > compact_limit && history.len() > KEEP_HEAD + KEEP_TAIL + 2 {
                let folded = history.len() - KEEP_HEAD - KEEP_TAIL;
                let head = history.drain(..KEEP_HEAD).collect::<Vec<_>>();
                let middle = history.drain(..folded).collect::<Vec<_>>();
                // 保留原始任务锚点（Cline 的做法：截断后仍保留最初任务，保持连续性）
                let task_anchor = history.iter().find_map(|m| {
                    if m.role != "user" {
                        return None;
                    }
                    let text: String = m
                        .content
                        .iter()
                        .filter_map(|b| match b {
                            InputContentBlock::Text { text } => Some(text.clone()),
                            _ => None,
                        })
                        .collect();
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                });
                let tail = std::mem::take(&mut history);
                // 智能压缩：把中间消息压成结构化摘要（失败则回退占位符）
                let summary = summarize_middle(&client, &model_id, &middle)
                    .await
                    .unwrap_or_default();
                // 摘要落盘：完整存档可追溯，模型需要细节时可 read_file 取回
                archive_compaction(&ctx.workspace_root, &summary);
                history = head;
                let archive_hint =
                    "完整过程存档于 .galen/context-archive.md，需要历史细节时用 read_file 取回。";
                if summary.trim().is_empty() {
                    history.push(InputMessage {
                        role: "user".to_string(),
                        content: vec![InputContentBlock::Text {
                            text: format!(
                                "[原始任务] {anchor}\n[上下文已折叠] {folded} 条较早的消息被移除，保留最近 {KEEP_TAIL} 条。{archive_hint}",
                                anchor = task_anchor.as_deref().unwrap_or("（无）"),
                            ),
                        }],
                    });
                } else {
                    history.push(InputMessage {
                        role: "user".to_string(),
                        content: vec![InputContentBlock::Text {
                            text: format!(
                                "[原始任务] {anchor}\n【已压缩摘要】\n{summary}\n{archive_hint}",
                                anchor = task_anchor.as_deref().unwrap_or("（无）")
                            ),
                        }],
                    });
                }
                history.extend(tail);
                compaction_count += 1;
                on_event(ChatEvent::Delta("[上下文已自动压缩]\n".to_string()));
            }
        }

        let tools = if final_turn {
            // Force convergence: no tools available on the final turn.
            Vec::new()
        } else {
            select_tools_for_task(
                registry.all_definitions_for_mode(ctx.mode).await,
                task_kind,
                &user_message,
            )
        };
        timing_probe(&format!("turn:{turn}:tools_ready:{}", tools.len()));

        // Inject a strong convergence instruction before the final request.
        if final_turn {
            record_trace(
                &trace,
                turn,
                "__convergence__".into(),
                String::new(),
                "final turn: tools stripped".into(),
                false,
            );
            history.push(InputMessage {
                role: "user".to_string(),
                content: vec![InputContentBlock::Text {
                    text: "[系统收敛指令] 本轮已到达工具调用轮次上限，后续不再提供任何工具。\
                          请基于以上已经获取到的全部信息，直接输出最终完整回答：\
                          先给结论，再列关键证据/结果，最后说明未解决的问题与建议的下一步。\
                          禁止再调用任何工具，也禁止重复之前的分析过程。"
                        .to_string(),
                }],
            });
        }

        // The first turn gets the user-selected reasoning budget. Mechanical
        // tool follow-ups run without deep thinking; DeepSeek still receives
        // the required prior reasoning_content in assistant history.
        let (turn_reasoning_effort, turn_thinking) =
            if turn > 1 || is_local_data_task(&user_message) {
                (
                    None,
                    Some(ThinkingConfig {
                        thinking_type: "disabled".to_string(),
                    }),
                )
            } else {
                (reasoning_effort.clone(), thinking.clone())
            };
        let request_max_tokens = if final_turn {
            max_tokens.min(8_192).max(4_096)
        } else if !tools.is_empty() {
            max_tokens.min(2_048)
        } else {
            max_tokens.min(4_096)
        };
        let request = MessageRequest {
            model: model_id.clone(),
            messages: history.clone(),
            max_tokens: request_max_tokens,
            system: Some(system_prompt.clone()),
            tools: if tools.is_empty() {
                None
            } else {
                Some(tools.clone())
            },
            tool_choice: (!tools.is_empty()).then_some(ToolChoice::Auto),
            stream: true,
            reasoning_effort: turn_reasoning_effort,
            thinking: turn_thinking,
            ..Default::default()
        };

        timing_probe(&format!("turn:{turn}:stream_start"));
        let mut stream = client
            .stream_message(&request)
            .await
            .map_err(|e| format_api_error(&e))?;
        timing_probe(&format!("turn:{turn}:stream_connected"));

        // Collect content blocks for this response
        let mut text_blocks: Vec<String> = Vec::new();
        let mut tool_calls: Vec<PendingToolCall> = Vec::new();
        let mut current_tool: Option<PendingToolCall> = None;
        let mut current_text: String = String::new();
        let mut current_thinking: String = String::new();
        let mut request_usage = Usage::default();

        loop {
            match stream.next_event().await {
                Ok(Some(ApiStreamEvent::MessageStart(event))) => {
                    request_usage = event.message.usage;
                }
                Ok(Some(ApiStreamEvent::MessageDelta(event))) => {
                    // Providers may repeat cumulative usage deltas. Keep the
                    // greatest value for this request, then add it once.
                    request_usage.input_tokens =
                        request_usage.input_tokens.max(event.usage.input_tokens);
                    request_usage.output_tokens =
                        request_usage.output_tokens.max(event.usage.output_tokens);
                    request_usage.cache_creation_input_tokens = request_usage
                        .cache_creation_input_tokens
                        .max(event.usage.cache_creation_input_tokens);
                    request_usage.cache_read_input_tokens = request_usage
                        .cache_read_input_tokens
                        .max(event.usage.cache_read_input_tokens);
                }
                Ok(Some(ApiStreamEvent::ContentBlockStart(event))) => {
                    timing_probe(&format!("turn:{turn}:first_content_block"));
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
                Ok(Some(ApiStreamEvent::ContentBlockDelta(event))) => match event.delta {
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
                },
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
                    run_summary.absorb_usage(&request_usage);
                    run_summary.iterations = turn;
                    on_event(ChatEvent::Error(format!("stream error: {e}")));
                    return Ok(run_summary);
                }
            }
        }
        run_summary.absorb_usage(&request_usage);
        run_summary.iterations = turn;
        run_summary.tool_call_count = run_summary.tool_call_count.saturating_add(tool_calls.len());

        // Build assistant message from collected blocks
        let mut assistant_content: Vec<InputContentBlock> = Vec::new();
        // DeepSeek thinking-mode tool calls require the exact reasoning_content
        // to be echoed with the assistant tool-call message. Other providers do
        // not need it, so avoid retaining the trace there.
        if model_id.to_ascii_lowercase().contains("deepseek") && !current_thinking.is_empty() {
            assistant_content.push(InputContentBlock::Thinking {
                thinking: std::mem::take(&mut current_thinking),
                signature: None,
            });
        } else {
            current_thinking.clear();
        }
        for text in &text_blocks {
            assistant_content.push(InputContentBlock::Text { text: text.clone() });
        }
        for tool in &tool_calls {
            let input: serde_json::Value =
                serde_json::from_str(&tool.input_json).unwrap_or(serde_json::Value::Null);
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
            let input_preview: String = tool.input_json.chars().take(300).collect();
            if is_error {
                con_error_streak += 1;
            } else {
                con_error_streak = 0;
            }
            let (text, is_error) = match result {
                Ok(ok) => (ok, false),
                Err(e) => (e, true),
            };
            record_trace(
                &trace,
                turn,
                tool.name.clone(),
                input_preview,
                text.clone(),
                is_error,
            );

            tool_results.push(InputContentBlock::ToolResult {
                tool_use_id: tool.id.clone(),
                content: vec![ToolResultContentBlock::Text {
                    text: compact_tool_result(&tool.name, &text, is_error),
                }],
                is_error,
            });
        }

        // --- consecutive error escalation (Cline-style progressive directives) ---
        if con_error_streak == 2 {
            tool_results.push(InputContentBlock::ToolResult {
                tool_use_id: "__hint_error_2__".to_string(),
                content: vec![ToolResultContentBlock::Text {
                    text: "[系统提示] 连续两次工具调用失败。请更换方法：检查参数、换一种工具，或缩小范围重试。不要重复同样的调用。"
                        .to_string(),
                }],
                is_error: false,
            });
        } else if con_error_streak >= 3 {
            tool_results.push(InputContentBlock::ToolResult {
                tool_use_id: "__hint_error_3__".to_string(),
                content: vec![ToolResultContentBlock::Text {
                    text: "[系统提示] 连续三次及以上工具调用失败，继续重试同一路径只会浪费轮次。请立即改用完全不同的策略：                          换工具、换数据源，或基于已有信息给出当前可交付的结论并说明缺口。禁止再试同一参数组合。"
                        .to_string(),
                }],
                is_error: false,
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

        history.push(InputMessage {
            role: "user".to_string(),
            content: tool_results,
        });
    }

    Ok(run_summary)
}

/// 把被折叠的中间消息压缩为结构化摘要（额外一次轻量 LLM 调用）。
/// 只保留 user/assistant 的文本，跳过工具调用细节；失败时返回空串，
/// 由调用方回退为普通占位符。
async fn summarize_middle(
    client: &ProviderClient,
    model_id: &str,
    middle: &[InputMessage],
) -> Result<String, String> {
    let mut text_messages: Vec<InputMessage> = middle
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .map(|m| InputMessage {
            role: m.role.clone(),
            content: m
                .content
                .iter()
                .filter_map(|b| match b {
                    InputContentBlock::Text { text } => {
                        Some(InputContentBlock::Text { text: text.clone() })
                    }
                    _ => None,
                })
                .collect(),
        })
        .filter(|m| !m.content.is_empty())
        .collect();
    if text_messages.is_empty() {
        return Ok(String::new());
    }

    let mut messages = Vec::with_capacity(text_messages.len() + 1);
    messages.push(InputMessage {
        role: "user".to_string(),
        content: vec![InputContentBlock::Text {
            text: "请将以下对话压缩为结构化摘要：\n1. 对话中若包含以【已压缩摘要】开头的文本，必须完整保留其全部条目（那是更早轮次的摘要，一旦丢失将无法恢复）。\n2. 保留：研究目标、已完成的动作与结果、关键结论、未决问题。\n3. 出现过的文件路径、PMID、明确要求保留的数据条目逐项保留。\n用简洁条目输出，新增内容不要复述过程。"
                .to_string(),
        }],
    });
    messages.append(&mut text_messages);

    let request = MessageRequest {
        model: model_id.to_string(),
        max_tokens: 600,
        messages,
        system: None,
        tools: None,
        tool_choice: None,
        stream: false,
        temperature: Some(0.2),
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: None,
        reasoning_effort: None,
        thinking: None,
    };

    // Compaction is a fallback, not user-visible work. Never let a slow
    // summarizer add another minute to time-to-first-result; the caller has a
    // deterministic placeholder when this short attempt fails.
    let resp = tokio::time::timeout(
        std::time::Duration::from_secs(12),
        client.send_message(&request),
    )
    .await
    .map_err(|_| "摘要生成超时".to_string())?
    .map_err(|e| format!("摘要生成失败: {e}"))?;

    let text = resp
        .content
        .iter()
        .filter_map(|b| match b {
            OutputContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(text)
}

/// 把折叠摘要追加到工作区 `.galen/context-archive.md`，让被折叠的历史可追溯。
/// 写盘失败静默（压缩本身不因存档失败而失败）。
fn archive_compaction(workspace_root: &Mutex<Option<PathBuf>>, summary: &str) {
    if summary.trim().is_empty() {
        return;
    }
    let root = match workspace_root.lock().ok().and_then(|g| g.clone()) {
        Some(r) => r,
        None => return,
    };
    let dir = root.join(".galen");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    use std::io::Write;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let entry = format!("## 上下文压缩存档 (unix {now})\n{summary}\n\n");
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("context-archive.md"))
        .and_then(|mut f| f.write_all(entry.as_bytes()));
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
