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
use std::time::{Duration, Instant};
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

use std::collections::{HashMap, HashSet};
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

fn build_system_prompt_for_contract(
    persona: &crate::personas::Persona,
    mode: crate::modes::ChatMode,
    contract: &TaskContract,
) -> String {
    if contract.class == TaskClass::DirectAnswer {
        return format!(
            "你是 Galen，当前角色是{}。请用中文提供准确、克制的医学科研解释。\n\n\
             ## 快速回答要求\n直接回答用户问题；不调用工具，不展开检索或执行计划，不复述题目。\
             保留用户指定的术语、字数和结论方向；信息不足时使用简短限定语。",
            persona.label,
        );
    }
    build_system_prompt(persona, mode)
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
    let contract = compile_task_contract(task_kind, user_message);
    if contract.class == TaskClass::DirectAnswer {
        return contract.execution_policy.to_string();
    }
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
    let execution_policy = task_execution_policy(user_message);
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
        "{opening}{skills}{execution_policy}\n\n## 当前工作区\n{workspace}\n\n## 当前科研环境\n{env_summary}\n\n{plan}\n\n{memory}{evidence}{resume}{plan_format}"
    )
}

const DATA_TOOLS: &[&str] = &[
    "list_files",
    "read_file",
    "search_files",
    "create_directory",
    "write_file",
    "execute_command",
];
const LITERATURE_TOOLS: &[&str] = &[
    "search_pubmed",
    "fetch_article",
    "format_citation",
    "list_files",
    "read_file",
    "search_files",
    "save_paper",
    "write_file",
];
const FOCUSED_ARTIFACT_TOOLS: &[&str] = &[
    "create_research_plan",
    "list_files",
    "read_file",
    "write_file",
];
const WORKSPACE_TOOLS: &[&str] = &[
    "list_files",
    "read_file",
    "search_files",
    "create_directory",
    "write_file",
];
const LOOKUP_TOOLS: &[&str] = &[
    "search_pubmed",
    "fetch_article",
    "format_citation",
    "list_files",
    "read_file",
    "search_files",
];
const REHAB_QUERY_TOOLS: &[&str] = &["rehab_data", "list_files", "read_file", "write_file"];
const NO_TOOLS: &[&str] = &[];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskClass {
    OpenEnded,
    DirectAnswer,
    QuickLookup,
    Literature,
    LocalData,
    Workspace,
    FocusedPlanArtifact,
    ArtifactCreation,
    RehabQuery,
}

#[derive(Debug, Clone)]
struct TaskContract {
    class: TaskClass,
    allowed_tools: Option<&'static [&'static str]>,
    max_tool_turns: u32,
    execution_policy: &'static str,
    artifact_paths: Vec<String>,
    disable_deep_reasoning: bool,
    response_token_cap: Option<u32>,
}

#[derive(Debug, Default)]
struct WorkingMemory {
    observed_resources: HashSet<String>,
    delivered_artifacts: HashSet<String>,
    consecutive_no_gain_turns: u32,
}

impl WorkingMemory {
    fn observe_tool_result(
        &mut self,
        tool_name: &str,
        input: &serde_json::Value,
        output: &str,
        is_error: bool,
        cache_hit: bool,
    ) -> bool {
        if is_error || cache_hit {
            return false;
        }
        let path = normalize_contract_path(
            input
                .get("path")
                .and_then(|value| value.as_str())
                .unwrap_or_default(),
        );
        match tool_name {
            "write_file" => {
                if !path.is_empty() {
                    self.delivered_artifacts.insert(path.clone());
                }
                let content = input
                    .get("content")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                self.observed_resources
                    .insert(format!("write:{path}:{}", stable_text_hash(content)))
            }
            "read_file" => self.observed_resources.insert(format!("read:{path}")),
            "list_files" | "search_files" => {
                let mut gained = false;
                for line in output.lines().filter(|line| {
                    line.trim_start().starts_with("[FILE]")
                        || line.trim_start().starts_with("[DIR]")
                }) {
                    gained |= self
                        .observed_resources
                        .insert(format!("entry:{}", line.trim()));
                }
                gained
            }
            _ => self
                .observed_resources
                .insert(format!("result:{tool_name}:{}", stable_text_hash(output))),
        }
    }

    fn finish_turn(&mut self, gained_information: bool) {
        if gained_information {
            self.consecutive_no_gain_turns = 0;
        } else {
            self.consecutive_no_gain_turns = self.consecutive_no_gain_turns.saturating_add(1);
        }
    }

    fn delivery_complete(&self, contract: &TaskContract) -> bool {
        !contract.artifact_paths.is_empty()
            && contract
                .artifact_paths
                .iter()
                .all(|path| self.delivered_artifacts.contains(path))
    }
}

fn stable_text_hash(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn compile_task_contract(kind: model_router::TaskKind, user_message: &str) -> TaskContract {
    let lower = user_message.to_lowercase();
    let literature_task = ["文献", "pubmed", "综述", "证据", "检索"]
        .iter()
        .any(|needle| lower.contains(needle));
    let artifact_paths = extract_artifact_paths(user_message);
    let (class, allowed_tools, max_tool_turns, execution_policy) = if is_direct_answer_task(&lower)
    {
        (
            TaskClass::DirectAnswer,
            Some(NO_TOOLS),
            1,
            "\n\n## 快速回答契约\n这是无需检索或工作区操作的直接回答。禁止调用工具；用最短路径给出核心定义、用途和方向性解释，严格遵守用户字数要求。",
        )
    } else if is_explicit_rehab_query(&lower) {
        (
                TaskClass::RehabQuery,
                Some(REHAB_QUERY_TOOLS),
                12,
                "\n\n## 本任务数据边界\n仅查询用户明确要求的患者/量表数据；保持只读，返回最小必要字段。",
            )
    } else if is_local_data_task(&lower) && !literature_task {
        (TaskClass::LocalData, Some(DATA_TOOLS), 28, "")
    } else if literature_task {
        (TaskClass::Literature, Some(LITERATURE_TOOLS), 20, "")
    } else if is_focused_plan_artifact_task(&lower) {
        (
                TaskClass::FocusedPlanArtifact,
                Some(FOCUSED_ARTIFACT_TOOLS),
                7,
                "\n\n## 本任务执行预算\n这是边界明确的计划节点交付任务。最多使用 7 轮工具。若用户要求多个研究节点，第一轮必须调用 create_research_plan 写入结构化节点；随后直接调用 write_file 生成用户指定 Artifact。所有文件工具路径必须相对当前工作区，禁止传入工作区绝对路径。只有任务明确依赖某个现有文件时，才按已知相对路径读取一次，禁止先列根目录或用通配符探索。只允许写入用户指定的最终 Artifact，禁止创建辅助脚本或替代产物。若节点输入不足，必须把阻塞原因、已有证据和下一可执行动作写入目标 Artifact，禁止编造缺失数据。write_file 成功后下一轮直接总结，不得再次读取或搜索同一事实。",
            )
    } else if is_explicit_artifact_creation_task(&lower) {
        (
                TaskClass::ArtifactCreation,
                Some(WORKSPACE_TOOLS),
                5,
                "\n\n## 本任务交付契约\n用户已经明确要求创建工作区 Artifact，因此不得因缺少非关键背景而停下询问。若研究主题或细节未给出，使用中性占位内容或明确标注的合理假设，并在文档中列出待确认项；先直接调用 write_file 生成用户指定路径，再依据写入结果确认非空并立即总结。不要在写入前反复列目录、读取不存在的记忆文件或要求用户回复“用示例”。",
            )
    } else if is_workspace_artifact_task(&lower) {
        (TaskClass::Workspace, Some(WORKSPACE_TOOLS), 16, "")
    } else if matches!(kind, model_router::TaskKind::QuickLookup) {
        (TaskClass::QuickLookup, Some(LOOKUP_TOOLS), 8, "")
    } else {
        (TaskClass::OpenEnded, None, 28, "")
    };
    // Creating a bounded workspace artifact is an execution task, not an
    // open-ended reasoning task. Deep reasoning here delays the first tool
    // call and can consume the entire response budget before any write occurs.
    let disable_deep_reasoning = matches!(
        class,
        TaskClass::DirectAnswer | TaskClass::FocusedPlanArtifact | TaskClass::ArtifactCreation
    );
    let response_token_cap = matches!(class, TaskClass::DirectAnswer).then_some(768);
    TaskContract {
        class,
        allowed_tools,
        max_tool_turns,
        execution_policy,
        artifact_paths,
        disable_deep_reasoning,
        response_token_cap,
    }
}

fn task_execution_policy(user_message: &str) -> String {
    compile_task_contract(
        model_router::TaskKind::from_intent(user_message),
        user_message,
    )
    .execution_policy
    .to_string()
}

/// Keep tool evidence useful without replaying unbounded logs or entire data
/// files into every subsequent model request.
fn compact_tool_result(tool_name: &str, text: &str, is_error: bool) -> String {
    let (limit, head, tail) = tool_result_budget(tool_name, is_error);
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let head_text: String = text.chars().take(head).collect();
    let tail_text: String = text
        .chars()
        .rev()
        .take(tail)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!(
        "[工具结果已压缩] tool={tool_name} error={is_error} original_chars={}\n\
         --- 前部 ---\n{head_text}\n\
         --- 省略 {} 字符；如需细节请按文件路径或行范围读取 ---\n\
         --- 尾部 ---\n{tail_text}",
        text.chars().count(),
        text.chars().count().saturating_sub(head + tail),
    )
}

fn tool_result_budget(tool_name: &str, is_error: bool) -> (usize, usize, usize) {
    if is_error {
        return (3_000, 2_200, 500);
    }
    match tool_name {
        // File contents need enough room for local evidence, but should not
        // replay an entire document into every subsequent model request.
        "read_file" | "fetch_article" => (6_000, 4_500, 1_000),
        // Search/list/command output is usually repetitive and benefits from a
        // smaller envelope.
        "list_files" | "search_files" | "search_pubmed" | "execute_command" => (4_000, 2_800, 800),
        _ => (4_000, 2_800, 800),
    }
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

#[cfg(test)]
fn select_tools_for_task(
    tools: Vec<ToolDefinition>,
    kind: model_router::TaskKind,
    user_message: &str,
) -> Vec<ToolDefinition> {
    let contract = compile_task_contract(kind, user_message);
    select_tools_for_contract(tools, &contract)
}

fn select_tools_for_contract(
    mut tools: Vec<ToolDefinition>,
    contract: &TaskContract,
) -> Vec<ToolDefinition> {
    if let Some(allowed) = contract.allowed_tools {
        tools.retain(|tool| allowed.contains(&tool.name.as_str()));
    } else if contract.class != TaskClass::RehabQuery {
        // Patient/assessment database access is sensitive and highly specific.
        tools.retain(|tool| tool.name != "rehab_data");
    }
    tools
}

fn is_workspace_artifact_task(text: &str) -> bool {
    [
        "读取",
        "写入",
        "文件",
        "工作区",
        "output/",
        "output\\",
        "节点",
        "计划",
        "产物",
        "artifact",
        ".md",
        ".json",
        ".toml",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn is_direct_answer_task(text: &str) -> bool {
    let direct_cue = [
        "直接回答",
        "简要回答",
        "简短回答",
        "不超过",
        "用一句话",
        "无需检索",
        "不要创建文件",
    ]
    .iter()
    .any(|needle| text.contains(needle));
    let external_evidence_cue = [
        "检索",
        "搜索",
        "查找",
        "查一下",
        "最新",
        "文献",
        "证据",
        "pubmed",
        "读取",
        "工作区",
    ]
    .iter()
    .any(|needle| text.contains(needle));
    direct_cue && !external_evidence_cue
}

fn is_focused_plan_artifact_task(text: &str) -> bool {
    let plan_or_node = ["计划", "节点", "plan.json", "node"]
        .iter()
        .any(|needle| text.contains(needle));
    let explicit_delivery = ["写入", "生成", "保存", "output/", "output\\", "artifact"]
        .iter()
        .any(|needle| text.contains(needle));
    plan_or_node && explicit_delivery
}

fn is_explicit_artifact_creation_task(text: &str) -> bool {
    let create_intent = ["创建", "生成", "写入", "保存"]
        .iter()
        .any(|needle| text.contains(needle));
    let artifact_path = ["output/", "output\\", "artifact", ".md", ".json", ".csv"]
        .iter()
        .any(|needle| text.contains(needle));
    create_intent && artifact_path
}

fn extract_artifact_paths(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for (start, _) in text
        .match_indices("output/")
        .chain(text.match_indices("output\\"))
    {
        let tail = &text[start..];
        let end = tail
            .find(|ch: char| {
                ch.is_whitespace()
                    || matches!(ch, '，' | '。' | '；' | ';' | ',' | ')' | '）' | ']' | '】')
            })
            .unwrap_or(tail.len());
        let path = normalize_contract_path(
            tail[..end].trim_matches(|ch: char| matches!(ch, '`' | '"' | '\'' | ':' | '：')),
        );
        if !path.is_empty() && !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

fn normalize_contract_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

fn normalize_path_against_workspace(
    path: &str,
    workspace_root: &std::path::Path,
) -> Option<String> {
    let candidate = std::path::Path::new(path);
    if !candidate.is_absolute() {
        return None;
    }
    candidate.strip_prefix(workspace_root).ok().map(|relative| {
        relative
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches("./")
            .to_string()
    })
}

fn normalize_workspace_tool_input(input: &mut serde_json::Value, ctx: &ToolContext) {
    let Some(path) = input
        .get("path")
        .and_then(|value| value.as_str())
        .map(str::to_string)
    else {
        return;
    };
    let Some(root) = ctx
        .workspace_root
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
    else {
        return;
    };
    if let Some(relative) = normalize_path_against_workspace(&path, &root) {
        input["path"] = serde_json::Value::String(relative);
    }
}

fn validate_tool_call_against_contract(
    contract: &TaskContract,
    tool_name: &str,
    input: &serde_json::Value,
) -> Result<(), String> {
    if tool_name != "write_file" || contract.artifact_paths.is_empty() {
        return Ok(());
    }
    if !matches!(
        contract.class,
        TaskClass::FocusedPlanArtifact | TaskClass::ArtifactCreation
    ) {
        return Ok(());
    }
    let requested = normalize_contract_path(
        input
            .get("path")
            .and_then(|value| value.as_str())
            .unwrap_or_default(),
    );
    if contract.artifact_paths.contains(&requested) {
        Ok(())
    } else {
        Err(format!(
            "任务契约拒绝写入 `{requested}`。本任务只允许写入最终 Artifact：{}。不要创建辅助脚本或替代文件；输入不足时请把阻塞说明写入目标 Artifact。",
            contract.artifact_paths.join(", ")
        ))
    }
}

#[cfg(test)]
fn max_tool_turns_for_task(user_message: &str) -> u32 {
    compile_task_contract(
        model_router::TaskKind::from_intent(user_message),
        user_message,
    )
    .max_tool_turns
}

fn is_explicit_rehab_query(text: &str) -> bool {
    let mentions_subject_data = [
        "患者",
        "受试者",
        "量表",
        "评估记录",
        "测量记录",
        "视频资产",
        "语音资产",
        "康复数据库",
        "rehab_data",
    ]
    .iter()
    .any(|needle| text.contains(needle));
    let asks_to_query = ["查询", "查找", "读取", "检索", "列出", "统计"]
        .iter()
        .any(|needle| text.contains(needle));
    mentions_subject_data && asks_to_query
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
            当前工作区没有可用的 GALEN.md 项目记忆。不要尝试读取不存在的文件；\
            只有当当前任务明确要求持久化新发现时，才创建或追加 GALEN.md。"
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
    fn missing_memory_is_not_advertised_as_an_existing_file() {
        let ws = tmp_ws("missing_memory", &[]);
        let index = memory_index(&ws);
        assert!(index.contains("没有可用的 GALEN.md"));
        assert!(index.contains("不要尝试读取不存在的文件"));
        assert!(!index.contains("根目录下有 GALEN.md"));
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
    fn workspace_plan_task_hides_rehab_and_command_tools() {
        let defs = vec![
            ToolDefinition {
                name: "read_file".into(),
                description: None,
                input_schema: serde_json::json!({}),
            },
            ToolDefinition {
                name: "write_file".into(),
                description: None,
                input_schema: serde_json::json!({}),
            },
            ToolDefinition {
                name: "execute_command".into(),
                description: None,
                input_schema: serde_json::json!({}),
            },
            ToolDefinition {
                name: "rehab_data".into(),
                description: None,
                input_schema: serde_json::json!({}),
            },
        ];
        let selected = select_tools_for_task(
            defs,
            model_router::TaskKind::Chat,
            "读取现有研究计划，只执行 n3，写入 output/research-plan.md",
        );
        let names: Vec<&str> = selected.iter().map(|tool| tool.name.as_str()).collect();
        assert_eq!(names, vec!["read_file", "write_file"]);
    }

    #[test]
    fn focused_plan_delivery_has_a_bounded_execution_contract() {
        let prompt = "读取研究计划的当前节点并写入 output/result.md";
        assert!(is_focused_plan_artifact_task(prompt));
        assert_eq!(max_tool_turns_for_task(prompt), 7);
        let contract = compile_task_contract(model_router::TaskKind::Chat, prompt);
        assert_eq!(contract.class, TaskClass::FocusedPlanArtifact);
        assert_eq!(contract.artifact_paths, vec!["output/result.md"]);
        let policy = task_execution_policy(prompt);
        assert!(policy.contains("最多使用 7 轮工具"));
        assert!(policy.contains("禁止先列根目录"));
        assert!(policy.contains("write_file 成功"));
        assert!(policy.contains("禁止编造缺失数据"));
        assert!(contract.disable_deep_reasoning);
        assert!(validate_tool_call_against_contract(
            &contract,
            "write_file",
            &serde_json::json!({"path": "output/result.md", "content": "ok"})
        )
        .is_ok());
        assert!(validate_tool_call_against_contract(
            &contract,
            "write_file",
            &serde_json::json!({"path": "./output/result.md", "content": "ok"})
        )
        .is_ok());
        assert!(validate_tool_call_against_contract(
            &contract,
            "write_file",
            &serde_json::json!({"path": "output/helper.py", "content": "bad"})
        )
        .is_err());
    }

    #[test]
    fn open_ended_analysis_keeps_the_global_safety_ceiling() {
        assert_eq!(max_tool_turns_for_task("深入分析这批复杂数据"), 28);
    }

    #[test]
    fn explicit_artifact_creation_uses_assumptions_instead_of_refusing() {
        let prompt = "创建 output/delivery.md，包含研究问题、PICO、风险和下一步";
        assert!(is_explicit_artifact_creation_task(prompt));
        assert_eq!(max_tool_turns_for_task(prompt), 5);
        let policy = task_execution_policy(prompt);
        assert!(policy.contains("不得因缺少非关键背景而停下询问"));
        assert!(policy.contains("合理假设"));
        assert!(policy.contains("直接调用 write_file"));
    }

    #[test]
    fn direct_answer_contract_removes_tools_thinking_and_dynamic_context() {
        let prompt = "请用不超过 180 字解释 FMA-UE 的用途。直接回答，不要创建文件。";
        let contract = compile_task_contract(model_router::TaskKind::QuickLookup, prompt);
        assert_eq!(contract.class, TaskClass::DirectAnswer);
        assert_eq!(contract.allowed_tools, Some(NO_TOOLS));
        assert_eq!(contract.max_tool_turns, 1);
        assert!(contract.disable_deep_reasoning);
        assert_eq!(contract.response_token_cap, Some(768));

        let ws = tmp_ws("direct_answer", &[("GALEN.md", "不应注入的记忆")]);
        let context = build_turn_context(prompt, crate::modes::ChatMode::Auto, &ws, true);
        assert!(context.contains("快速回答契约"));
        assert!(!context.contains("当前工作区"));
        assert!(!context.contains("不应注入的记忆"));

        let persona = crate::personas::find_persona("medical");
        let compact =
            build_system_prompt_for_contract(&persona, crate::modes::ChatMode::Auto, &contract);
        assert!(!compact.contains("search_pubmed"));
        assert!(compact.len() < build_system_prompt(&persona, crate::modes::ChatMode::Auto).len());
    }

    #[test]
    fn explicit_search_request_does_not_use_direct_answer_contract() {
        let prompt = "请检索最新文献后简短回答";
        let contract = compile_task_contract(model_router::TaskKind::QuickLookup, prompt);
        assert_ne!(contract.class, TaskClass::DirectAnswer);
        assert!(!contract.disable_deep_reasoning);
    }

    #[test]
    fn working_memory_detects_no_gain_and_completed_delivery() {
        let contract =
            compile_task_contract(model_router::TaskKind::Chat, "创建 output/delivery.md");
        let mut memory = WorkingMemory::default();
        let listing = "[FILE] plan.json (20 bytes)\n[DIR] output (0 bytes)";
        let list_input = serde_json::json!({"path": ""});
        assert!(memory.observe_tool_result("list_files", &list_input, listing, false, false));
        assert!(!memory.observe_tool_result(
            "search_files",
            &serde_json::json!({"pattern": "*"}),
            listing,
            false,
            false
        ));
        memory.finish_turn(false);
        assert_eq!(memory.consecutive_no_gain_turns, 1);

        assert!(memory.observe_tool_result(
            "write_file",
            &serde_json::json!({"path": "output/delivery.md", "content": "done"}),
            "Wrote file",
            false,
            false
        ));
        assert!(memory.delivery_complete(&contract));
    }

    #[test]
    fn workspace_absolute_paths_are_normalized_only_when_inside_root() {
        let root = PathBuf::from(r"C:\tmp\galen-workspace");
        assert_eq!(
            normalize_path_against_workspace(r"C:\tmp\galen-workspace", &root),
            Some(String::new())
        );
        assert_eq!(
            normalize_path_against_workspace(r"C:\tmp\galen-workspace\output\x.md", &root),
            Some("output/x.md".to_string())
        );
        assert_eq!(
            normalize_path_against_workspace(r"C:\tmp\outside\x.md", &root),
            None
        );
    }

    #[test]
    fn rehab_data_requires_an_explicit_subject_data_query() {
        assert!(!is_explicit_rehab_query("生成康复研究计划"));
        assert!(is_explicit_rehab_query("查询患者的量表评估记录"));
    }

    #[test]
    fn tool_result_budgets_are_task_specific() {
        let read_budget = tool_result_budget("read_file", false);
        let search_budget = tool_result_budget("search_files", false);
        let error_budget = tool_result_budget("read_file", true);
        assert!(read_budget.0 > search_budget.0);
        assert!(error_budget.0 < read_budget.0);
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
    let run_started = Instant::now();
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
    let context_started = Instant::now();
    let task_kind = model_router::TaskKind::from_intent(&user_message);
    let task_contract = compile_task_contract(task_kind, &user_message);
    let system_prompt = build_system_prompt_for_contract(&persona, mode, &task_contract);
    // Dynamic state is refreshed every turn while the cache-stable prefix stays unchanged.
    let first_turn = history.is_empty();
    let turn_context = build_turn_context(&user_message, mode, &workspace_root, first_turn);
    let context_assembly_ms = context_started.elapsed().as_millis() as u64;
    timing_probe("run_chat:context_ready");

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
    let mcp_started = Instant::now();
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
    let mcp_setup_ms = mcp_started.elapsed().as_millis() as u64;
    let on_event: Arc<dyn Fn(ChatEvent) + Send + Sync> = Arc::new(on_event);
    let mut ctx = ToolContext::with_event_sender(medical.clone(), workspace_root, on_event.clone());
    ctx.mode = mode;

    // ── Auto-compaction knob ──
    const KEEP_HEAD: usize = 2; // keep first N messages (context)
    const KEEP_TAIL: usize = 6; // keep last N messages (recent)
    const MAX_COMPACTIONS: u32 = 3; // 长会话可多次压缩，旧摘要自动并入新摘要

    // Multi-turn loop: keep going until model responds with text (no tool calls)
    let mut turn = 0;
    let max_tool_turns = task_contract.max_tool_turns;
    let mut last_tool_name: Option<String> = None;
    let mut same_tool_streak: u32 = 0;
    let mut con_error_streak: u32 = 0;
    let mut final_chance_used = false;
    let mut final_turn = false;
    let mut empty_response_retried = false;
    let mut compaction_count: u32 = 0;
    // Reuse identical read-only calls within one run. This prevents repeated
    // file/search/database reads from consuming latency and replaying another
    // large result while never suppressing writes, commands or unknown MCP
    // side effects.
    let mut readonly_tool_cache: HashMap<String, (String, bool)> = HashMap::new();
    let mut working_memory = WorkingMemory::default();
    let mut run_summary = ChatRunSummary {
        context_assembly_ms,
        mcp_setup_ms,
        ..ChatRunSummary::default()
    };
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
        if turn > max_tool_turns {
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
                run_summary.model_request_count = run_summary.model_request_count.saturating_add(1);
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
                run_summary.compaction_count = compaction_count;
                on_event(ChatEvent::Delta("[上下文已自动压缩]\n".to_string()));
            }
        }

        let mut tools = if final_turn {
            // Force convergence: no tools available on the final turn.
            Vec::new()
        } else {
            select_tools_for_contract(
                registry.all_definitions_for_mode(ctx.mode).await,
                &task_contract,
            )
        };
        if working_memory.consecutive_no_gain_turns >= 2 && !task_contract.artifact_paths.is_empty()
        {
            tools.retain(|tool| tool.name == "write_file");
        }
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
        let (turn_reasoning_effort, turn_thinking) = if turn > 1
            || is_local_data_task(&user_message)
            || task_contract.disable_deep_reasoning
            || empty_response_retried
        {
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
        let request_max_tokens = task_contract
            .response_token_cap
            .map(|cap| request_max_tokens.min(cap))
            .unwrap_or(request_max_tokens);
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
        run_summary.model_request_count = run_summary.model_request_count.saturating_add(1);
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
                    run_summary
                        .ttft_ms
                        .get_or_insert(run_started.elapsed().as_millis() as u64);
                    timing_probe(&format!("turn:{turn}:first_content_block"));
                    match event.content_block {
                        OutputContentBlock::Text { text } => {
                            if !text.trim().is_empty() {
                                run_summary
                                    .ttfr_ms
                                    .get_or_insert(run_started.elapsed().as_millis() as u64);
                            }
                            // Only initialize the accumulator — don't emit.
                            // ContentBlockDelta events carry the actual streamed text.
                            current_text = text;
                        }
                        OutputContentBlock::ToolUse { id, name, .. } => {
                            run_summary
                                .ttfr_ms
                                .get_or_insert(run_started.elapsed().as_millis() as u64);
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
                        if !text.trim().is_empty() {
                            run_summary
                                .ttfr_ms
                                .get_or_insert(run_started.elapsed().as_millis() as u64);
                        }
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
                    run_summary.total_ms = run_started.elapsed().as_millis() as u64;
                    run_summary.compaction_count = compaction_count;
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
            let mut input: serde_json::Value =
                serde_json::from_str(&tool.input_json).unwrap_or(serde_json::Value::Null);
            normalize_workspace_tool_input(&mut input, &ctx);
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
            if full_text.trim().is_empty() && !empty_response_retried {
                empty_response_retried = true;
                history.push(InputMessage {
                    role: "user".to_string(),
                    content: vec![InputContentBlock::Text {
                        text: "[系统恢复指令] 上一轮只产生了内部推理，没有最终文本或工具调用。关闭深度思考，立即执行最小必要工具动作；若无需工具则直接给出完整结论。"
                            .to_string(),
                    }],
                });
                continue;
            }
            if full_text.trim().is_empty() {
                on_event(ChatEvent::Error(
                    "模型连续返回空响应：未生成文本或工具调用，请重试。".to_string(),
                ));
                break;
            }
            on_event(ChatEvent::Done(full_text));
            break;
        }

        // Execute tools and build result message
        let mut tool_results: Vec<InputContentBlock> = Vec::new();
        let mut turn_gained_information = false;
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

            let mut input: serde_json::Value =
                serde_json::from_str(&tool.input_json).unwrap_or(serde_json::Value::Null);
            normalize_workspace_tool_input(&mut input, &ctx);
            let cache_key = format!(
                "{}:{}",
                tool.name,
                serde_json::to_string(&input).unwrap_or_else(|_| tool.input_json.clone())
            );
            let cacheable = registry.is_write_tool(&tool.name) == Some(false);
            let cached = cacheable
                .then(|| readonly_tool_cache.get(&cache_key).cloned())
                .flatten();
            let (text, is_error, cache_hit) = if let Some((text, is_error)) = cached {
                (text, is_error, true)
            } else {
                let result =
                    match validate_tool_call_against_contract(&task_contract, &tool.name, &input) {
                        Ok(()) => {
                            registry
                                .execute_dynamic(&tool.name, input.clone(), &ctx)
                                .await
                        }
                        Err(error) => Err(error),
                    };
                let (text, is_error) = match result {
                    Ok(ok) => (ok, false),
                    Err(error) => (error, true),
                };
                if cacheable {
                    readonly_tool_cache.insert(cache_key, (text.clone(), is_error));
                }
                (text, is_error, false)
            };
            turn_gained_information |=
                working_memory.observe_tool_result(&tool.name, &input, &text, is_error, cache_hit);
            if registry.is_write_tool(&tool.name) != Some(false) {
                // A write/command may change previously cached file or search
                // results, so never let the read cache outlive a mutation.
                readonly_tool_cache.clear();
            }
            let input_preview: String = tool.input_json.chars().take(300).collect();
            if is_error {
                con_error_streak += 1;
            } else {
                con_error_streak = 0;
            }
            let trace_output = if cache_hit {
                format!("[只读缓存命中：未重复执行]\n{text}")
            } else {
                text.clone()
            };
            record_trace(
                &trace,
                turn,
                tool.name.clone(),
                input_preview,
                trace_output,
                is_error,
            );

            let model_text = if cache_hit {
                format!(
                    "[系统：相同只读调用已复用缓存，未重复执行。请使用已有结果继续任务，不要再次调用相同参数。]\n{text}"
                )
            } else {
                text
            };
            tool_results.push(InputContentBlock::ToolResult {
                tool_use_id: tool.id.clone(),
                content: vec![ToolResultContentBlock::Text {
                    text: compact_tool_result(&tool.name, &model_text, is_error),
                }],
                is_error,
            });
        }

        working_memory.finish_turn(turn_gained_information);
        if working_memory.delivery_complete(&task_contract) {
            final_turn = true;
            tool_results.push(InputContentBlock::ToolResult {
                tool_use_id: "__delivery_complete__".to_string(),
                content: vec![ToolResultContentBlock::Text {
                    text: "[系统：任务契约中的 Artifact 已全部成功写入。下一轮直接总结交付，不再调用工具。]"
                        .to_string(),
                }],
                is_error: false,
            });
        } else if working_memory.consecutive_no_gain_turns == 1 {
            tool_results.push(InputContentBlock::ToolResult {
                tool_use_id: "__no_gain_1__".to_string(),
                content: vec![ToolResultContentBlock::Text {
                    text: "[系统：本轮没有获得新文件、新事实或新 Artifact。请停止扩大搜索，基于已有信息执行下一项必要动作。]"
                        .to_string(),
                }],
                is_error: false,
            });
        } else if working_memory.consecutive_no_gain_turns >= 2
            && !task_contract.artifact_paths.is_empty()
        {
            tool_results.push(InputContentBlock::ToolResult {
                tool_use_id: "__no_gain_2__".to_string(),
                content: vec![ToolResultContentBlock::Text {
                    text: "[系统：连续两轮没有信息增益。探索型工具将被关闭；请立即写入目标 Artifact，明确标注假设或阻塞点。]"
                        .to_string(),
                }],
                is_error: false,
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

    run_summary.total_ms = run_started.elapsed().as_millis() as u64;
    run_summary.compaction_count = compaction_count;
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
