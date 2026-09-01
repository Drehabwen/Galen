use std::path::PathBuf;
use std::sync::Mutex;

use api::ToolDefinition;

use crate::task_contract::{
    compile_task_contract, normalize_contract_path, task_execution_policy, TaskClass, TaskContract,
    READ_WRITE_TOOLS,
};
use crate::tools::ToolContext;

pub(crate) fn build_system_prompt(
    persona: &crate::personas::Persona,
    _mode: crate::modes::ChatMode,
) -> String {
    // L0 常驻核心：人格 + 科研品味。模式和任务契约属于动态尾部，
    // 不能放进 system prefix，否则切换任务/模式会击穿 provider 前缀缓存。
    let taste = if persona.id == "medical" {
        crate::skills::RESEARCH_TASTE
    } else {
        ""
    };
    format!(
        "{}\n\n{}\n\n## 回复要求\n\
         优先行动并报告可验证结果。不要复述内部推理；需要说明决策时只给简短依据。\
         达到当前任务的验收条件后立即收敛输出。\
         如果用户明确要求验证工具失败或指定工具步骤顺序，必须实际执行且严格按序；\
         禁止根据工作区清单推断结果后声称已经执行。",
        persona.system_prompt, taste,
    )
}

pub(crate) fn build_system_prompt_for_contract(
    persona: &crate::personas::Persona,
    mode: crate::modes::ChatMode,
    _contract: &TaskContract,
) -> String {
    build_system_prompt(persona, mode)
}

/// Assemble the dynamic context for the current turn.
///
/// L1 skills follow the current intent. L2 workspace/task/evidence state is
/// refreshed on every turn so a durable flow-back becomes visible immediately,
/// even when the frontend still has conversational history. Session-opening
/// instructions are included only once.
pub(crate) fn build_turn_context(
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
    let mode_policy = crate::modes::mode_prompt(mode);
    let skills = crate::skills::assemble_skills_for_intent(task_kind, user_message);
    // L2：项目画像
    let plan = plan_progress_summary(workspace_root);
    let memory = memory_index(workspace_root);
    let evidence = workspace_root_path(workspace_root)
        .map(|root| crate::evidence::evidence_chain_summary(&root, 8))
        .unwrap_or_default();
    let literature_coverage = match workspace_root_path(workspace_root) {
        Some(root) => {
            let provider_source = crate::commands::configured_literature_providers();
            match crate::commands::literature_coverage_for_workspace_from_provider_source(
                &root,
                &provider_source,
            ) {
                Ok(coverage) => render_literature_coverage_context(&coverage),
                Err(_) => render_literature_coverage_unavailable_context(),
            }
        }
        None => {
            let provider_source = crate::commands::configured_literature_providers();
            render_literature_coverage_context(
                &crate::commands::literature_coverage_from_provider_source(
                    None,
                    &provider_source,
                    &[],
                ),
            )
        }
    };
    let resume = if first_turn {
        resume_protocol(workspace_root)
    } else {
        String::new()
    };
    let status = crate::runtime_manager::detect_all();
    let env_summary = crate::runtime_manager::status_summary(&status);
    let workspace = if contract.allowed_tools == Some(READ_WRITE_TOOLS) {
        // The user already supplied exact paths. Listing which files exist lets
        // a model infer a missing-path result and falsely claim it executed the
        // requested failure probe. Keep the contract authoritative instead.
        "定点路径任务：不展开工作区清单；必须通过实际 read_file 结果判断文件状态。".to_string()
    } else {
        workspace_summary(workspace_root)
    };
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
        "{mode_policy}\n\n{opening}{skills}{execution_policy}\n\n## 当前工作区\n{workspace}\n\n## 当前科研环境\n{env_summary}\n\n{plan}\n\n{memory}{evidence}\n\n{literature_coverage}\n\n## 证据引用纪律\n凡是推荐量表、纳排标准、统计方法、疗效判断或安全性判断，必须在同一条建议后给出已检索来源的可核验标识（PMID、DOI、数据库记录或明确标注“当前无直接来源”）。不得用未检索来源把推测写成事实。{resume}{plan_format}"
    )
}

pub(crate) fn render_literature_coverage_context(
    coverage: &crate::commands::LiteratureCoverageResponse,
) -> String {
    let mut lines = vec!["## Literature coverage".to_string()];
    if coverage.task_id.is_none() {
        lines.push(
            "- No active research task; task-scoped literature coverage is unavailable."
                .to_string(),
        );
    }
    for provider in &coverage.providers {
        let detail = match provider.state {
            crate::search_run::CoverageState::Searched => match provider.result_count {
                Some(count) => format!("searched ({count} results)"),
                None => "searched (result count unavailable)".to_string(),
            },
            crate::search_run::CoverageState::Failed if provider.provider_id == "cnki" => {
                "failed; do not infer absence of Chinese evidence".to_string()
            }
            crate::search_run::CoverageState::Failed => "failed".to_string(),
            crate::search_run::CoverageState::ConnectedNotSearched => "not searched".to_string(),
            crate::search_run::CoverageState::ConfiguredDisabled => "disabled".to_string(),
            crate::search_run::CoverageState::Unavailable if provider.provider_id == "cnki" => {
                "unavailable; do not infer absence of Chinese evidence".to_string()
            }
            crate::search_run::CoverageState::Unavailable => "unavailable".to_string(),
            crate::search_run::CoverageState::NotConfigured => "not configured".to_string(),
        };
        lines.push(format!("- {}: {detail}", provider.display_name));
    }
    if let Some(limitation) = &coverage.limitation {
        lines.push(format!("- Coverage limitation: {limitation}"));
    }
    lines.join("\n")
}

fn render_literature_coverage_unavailable_context() -> String {
    "## Literature coverage\n\
     - Literature coverage is unavailable; task-scoped search history could not be read.\n\
     - Coverage limitation: Final claims must say \"based on searched providers\" and must not imply comprehensive coverage."
        .to_string()
}

/// Keep tool evidence useful without replaying unbounded logs or entire data
/// files into every subsequent model request.
pub(crate) fn compact_tool_result(tool_name: &str, text: &str, is_error: bool) -> String {
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

pub(crate) fn tool_result_budget(tool_name: &str, is_error: bool) -> (usize, usize, usize) {
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
pub(crate) fn compact_trigger_bytes() -> usize {
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
pub(crate) fn select_tools_for_task(
    tools: Vec<ToolDefinition>,
    kind: model_router::TaskKind,
    user_message: &str,
) -> Vec<ToolDefinition> {
    let contract = compile_task_contract(kind, user_message);
    select_tools_for_contract(tools, &contract)
}

pub(crate) fn select_tools_for_contract(
    mut tools: Vec<ToolDefinition>,
    contract: &TaskContract,
) -> Vec<ToolDefinition> {
    if contract.allowed_tools.is_some() {
        tools.retain(|tool| contract.allows_tool(&tool.name));
    } else if contract.class != TaskClass::RehabQuery {
        // Patient/assessment database access is sensitive and highly specific.
        tools.retain(|tool| tool.name != "rehab_data");
    }
    tools
}

pub(crate) fn normalize_path_against_workspace(
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

pub(crate) fn normalize_workspace_tool_input(input: &mut serde_json::Value, ctx: &ToolContext) {
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

pub(crate) fn validate_tool_call_against_contract(
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

/// 读取工作区根目录；无工作区返回 None。
pub(crate) fn workspace_root_path(workspace_root: &Mutex<Option<PathBuf>>) -> Option<PathBuf> {
    workspace_root.lock().ok().and_then(|g| g.clone())
}

/// 记忆索引：短记忆全文注入；长记忆只注入最近记录 + 总量（全文按需读取）。
pub(crate) fn memory_index(workspace_root: &Mutex<Option<PathBuf>>) -> String {
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
pub(crate) fn plan_progress_summary(workspace_root: &Mutex<Option<PathBuf>>) -> String {
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
pub(crate) fn resume_protocol(workspace_root: &Mutex<Option<PathBuf>>) -> String {
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
pub(crate) fn workspace_summary(workspace_root: &Mutex<Option<PathBuf>>) -> String {
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
