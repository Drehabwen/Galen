use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use api::{InputContentBlock, InputMessage};
use galen_lib::backend::{run_chat, ChatEvent, ChatRunSummary, ToolTrace};
use galen_lib::eval::{
    append_jsonl, compare_runs, compare_runs_ignore_config, discover_cases, load_jsonl,
    reliability_report, summary_field_coverage, ComparisonDecision, ContextVariant, EvalCase,
    RunObservation, RunRecord,
};
use galen_lib::eval_report::{render_markdown_report, write_markdown_report};
use galen_lib::modes::ChatMode;
use galen_lib::personas::{medical_persona, Persona};
use galen_lib::rag_eval::{
    compare_rag_reports, load_report as load_rag_report, run_rag_benchmark,
    write_report as write_rag_report, RagBenchmarkSpec,
};
use medical_core::MedicalCore;
use model_router::ModelRouter;
use runtime::{
    compact_session, CompactionConfig, ContentBlock, ConversationMessage, MessageRole, Session,
};

/// 消融用的"裸 persona"：无科研人格、无系统提示，用于量化 medical_persona 的贡献。
#[must_use]
pub fn no_persona() -> Persona {
    Persona {
        id: "none".to_string(),
        label: "无人格".to_string(),
        description: "消融基线：无系统人格提示".to_string(),
        system_prompt: "",
    }
}

fn usage() -> ! {
    eprintln!(
        "用法:\n  eval validate [--cases evals/cases]\n  eval run --case E01 [--model alias] [--persona medical|none] [--variant none|full|compacted|skeleton|fullpack] [--repeat 1] [--output evals/runs/run.jsonl]\n  eval rescore --input FILE --output FILE [--cases evals/cases]\n  eval reliability --input FILE [--k 5]\n  eval compare --baseline FILE --candidate FILE [--ignore-config]\n  eval rag-validate --dataset evals/datasets/rag_ais.toml\n  eval rag --dataset evals/datasets/rag_ais.toml [--repeat 10] [--output evals/runs/rag.json]\n  eval rag-compare --baseline FILE --candidate FILE [--output FILE]\n  eval report [--agent FILE] [--rag FILE] --output evals/reports/report.md [--title TITLE]"
    );
    std::process::exit(2);
}

fn option(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn default_cases_dir() -> String {
    if Path::new("evals/cases").is_dir() {
        "evals/cases".to_string()
    } else {
        "../evals/cases".to_string()
    }
}

fn git_commit() -> String {
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let dirty = std::process::Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .ok()
        .is_some_and(|output| output.status.success() && !output.stdout.is_empty());
    if dirty {
        format!("{commit}-dirty")
    } else {
        commit
    }
}

fn copy_fixture(source: &Path, destination: &Path) -> Result<(), String> {
    std::fs::create_dir_all(destination).map_err(|error| format!("创建评测工作区失败: {error}"))?;
    for entry in std::fs::read_dir(source)
        .map_err(|error| format!("读取 fixture {} 失败: {error}", source.display()))?
    {
        let entry = entry.map_err(|error| format!("读取 fixture 条目失败: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("读取 fixture 类型失败: {error}"))?;
        if file_type.is_symlink() {
            return Err(format!(
                "fixture 不允许符号链接: {}",
                entry.path().display()
            ));
        }
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_fixture(&entry.path(), &target)?;
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), target)
                .map_err(|error| format!("复制 fixture 文件失败: {error}"))?;
        }
    }
    Ok(())
}

fn prepare_workspace(case: &EvalCase, eval_root: &Path, run_index: u32) -> Result<PathBuf, String> {
    let workspace = std::env::temp_dir().join("galen-evals").join(format!(
        "{}-{}-{run_index}",
        case.id,
        std::process::id()
    ));
    if workspace.exists() {
        return Err(format!(
            "拒绝覆盖已存在的评测工作区: {}",
            workspace.display()
        ));
    }
    std::fs::create_dir_all(workspace.join("output"))
        .map_err(|error| format!("创建评测工作区失败: {error}"))?;
    if let Some(fixture) = &case.fixture {
        let fixture_path = eval_root.join(fixture);
        copy_fixture(&fixture_path, &workspace)?;
    }
    Ok(workspace)
}

/// 构造超阈值 seed 会话（可复现）：约 60K tokens 的长历史，模拟真实大会话，
/// 使压缩引擎的经济性（token 节省率）可被量化评估。
fn build_seed_session(case: &EvalCase) -> Session {
    let facts = if case.required.facts.is_empty() {
        "样本量 48、主要结局 FMA-UE、随机 12 周".to_string()
    } else {
        case.required.facts.join("、")
    };
    // 填充块：放大会话体积到真实长会话量级（约 1500 字符/条）
    const FILL: &str = "本研究属于运动康复与神经康复交叉领域，重点关注功能结局的纵向变化轨迹。\
        受试者入组标准、排除标准、知情同意流程、随机化隐藏方案、盲法实施细节、数据监查委员会章程\
        与中期分析计划均已在前序讨论中逐步确定。每次随访窗口需控制在 ±2 天内，数据采集员需通过一致性培训。";
    let mut messages = Vec::new();
    for index in 0..60 {
        let detail = format!(
            "历史请求 {index}：讨论研究方案设计与数据采集流程，关键约束 {facts}。{FILL}\
             下一步（pending）包括完成方案附录、补充随机化方法说明与统计分析计划。\
             参考 docs/protocol/s01_study_protocol.md 与 scripts/analyze.py，\
             涉及 6MWT、FMA-UE 量表与不良事件记录表。"
        );
        messages.push(ConversationMessage {
            role: MessageRole::User,
            blocks: vec![ContentBlock::Text {
                text: detail.clone(),
            }],
            usage: None,
        });
        messages.push(ConversationMessage {
            role: MessageRole::Assistant,
            blocks: vec![ContentBlock::Text {
                text: format!("回应 {index}：已确认 {facts}；待办：附录、随机化说明与统计计划；引用协议文档与脚本。"),
            }],
            usage: None,
        });
        // 每 5 轮注入一次模拟工具调用，使压缩摘要出现 Tools mentioned 字段
        if index % 5 == 0 {
            messages.push(ConversationMessage {
                role: MessageRole::Tool,
                blocks: vec![
                    ContentBlock::ToolUse {
                        id: format!("tu-{index}"),
                        name: "read_file".to_string(),
                        input: "docs/protocol/s01_study_protocol.md".to_string(),
                    },
                    ContentBlock::ToolResult {
                        tool_use_id: format!("tu-{index}"),
                        tool_name: "read_file".to_string(),
                        output: "协议文档内容：样本量 48、FMA-UE、随机 12 周。".to_string(),
                        is_error: false,
                    },
                ],
                usage: None,
            });
        }
    }
    let mut session = Session::new();
    session.messages = messages;
    session
}

/// 把压缩后的会话消息转成 run_chat 的历史输入。
fn to_input_messages(messages: &[ConversationMessage]) -> Vec<InputMessage> {
    messages
        .iter()
        .map(|message| {
            let role = match message.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
            };
            let text = message
                .blocks
                .iter()
                .map(|block| match block {
                    ContentBlock::Text { text } => text.clone(),
                    ContentBlock::ToolUse { name, input, .. } => {
                        format!("tool_use {name}({input})")
                    }
                    ContentBlock::ToolResult {
                        tool_name, output, ..
                    } => {
                        format!("tool_result {tool_name}: {output}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            InputMessage {
                role: role.to_string(),
                content: vec![InputContentBlock::Text { text }],
            }
        })
        .collect()
}

/// 按 case 的上下文变体构造历史消息，返回 (history, 摘要骨架字段覆盖率)。
fn build_context_history(case: &EvalCase) -> (Vec<InputMessage>, Option<(usize, usize)>) {
    match case.context.variant {
        ContextVariant::None => (Vec::new(), None),
        // 完整 seed 上下文（不压缩）：作为压缩引擎的真实对照基线
        ContextVariant::Full => (to_input_messages(&build_seed_session(case).messages), None),
        // SkeletonOnly：仅注入摘要（System 消息，无保留尾部）；
        // Compacted：摘要 System 消息 + 保留尾部；FullPack 暂同 Compacted（ResearchContextPack 后续接入）。
        ContextVariant::Compacted | ContextVariant::SkeletonOnly | ContextVariant::FullPack => {
            let session = build_seed_session(case);
            let config = CompactionConfig {
                preserve_recent_messages: case.context.preserve_recent.unwrap_or(8),
                max_estimated_tokens: case.context.max_tokens.unwrap_or(50_000),
            };
            let result = compact_session(&session, config);
            let coverage = Some(summary_field_coverage(
                &result.summary,
                &case.context.require_fields,
            ));
            let messages = if matches!(case.context.variant, ContextVariant::SkeletonOnly) {
                vec![ConversationMessage {
                    role: MessageRole::System,
                    blocks: vec![ContentBlock::Text {
                        text: result.summary.clone(),
                    }],
                    usage: None,
                }]
            } else {
                result.compacted_session.messages
            };
            (to_input_messages(&messages), coverage)
        }
    }
}

async fn run_once(
    case: &EvalCase,
    eval_root: &Path,
    model_alias: &str,
    run_index: u32,
    persona: &Persona,
) -> Result<RunRecord, String> {
    let router = ModelRouter::load().map_err(|error| format!("加载 models.toml 失败: {error}"))?;
    let model_id = router.resolve_model_id(model_alias);
    let workspace = prepare_workspace(case, eval_root, run_index)?;
    let (history, summary_field_coverage) = build_context_history(case);
    let response = Arc::new(Mutex::new(String::new()));
    let response_sink = response.clone();
    let traces: Arc<Mutex<Vec<ToolTrace>>> = Arc::new(Mutex::new(Vec::new()));
    let trace_sink = traces.clone();
    let timeout = std::time::Duration::from_secs(case.timeout_seconds);
    let result = tokio::time::timeout(
        timeout,
        run_chat(
            model_alias.to_string(),
            model_id,
            case.prompt.clone(),
            history,
            ChatMode::Auto,
            persona.clone(),
            "medium".to_string(),
            Arc::new(MedicalCore::new(None)),
            router,
            Mutex::new(Some(workspace.clone())),
            Some(trace_sink),
            move |event| match event {
                ChatEvent::Delta(text) => {
                    if let Ok(mut value) = response_sink.lock() {
                        value.push_str(&text);
                    }
                }
                ChatEvent::Done(text) => {
                    if let Ok(mut value) = response_sink.lock() {
                        *value = text;
                    }
                }
                _ => {}
            },
        ),
    )
    .await;
    let (run_ok, summary) = match result {
        Ok(Ok(summary)) => (true, summary),
        Ok(Err(error)) => {
            eprintln!("{} 第 {run_index} 次运行失败: {error}", case.id);
            (false, ChatRunSummary::default())
        }
        Err(_) => {
            eprintln!("{} 第 {run_index} 次运行超时", case.id);
            (false, ChatRunSummary::default())
        }
    };
    let response = response
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default();
    let traces = traces.lock().map(|value| value.clone()).unwrap_or_default();
    Ok(RunRecord::evaluate(
        case,
        RunObservation {
            commit: &git_commit(),
            model: model_alias,
            run_index,
            run_ok,
            response: &response,
            summary: &summary,
            traces: &traces,
            workspace: &workspace,
            summary_field_coverage,
        },
    ))
}

fn validate(args: &[String]) -> Result<(), String> {
    let cases_dir = PathBuf::from(option(args, "--cases").unwrap_or_else(default_cases_dir));
    let cases = discover_cases(&cases_dir)?;
    if cases.is_empty() {
        return Err(format!("{} 中没有 TOML 案例", cases_dir.display()));
    }
    let eval_root = cases_dir.parent().unwrap_or_else(|| Path::new("."));
    for (path, case) in &cases {
        if let Some(fixture) = &case.fixture {
            let fixture_path = eval_root.join(fixture);
            if !fixture_path.is_dir() {
                return Err(format!(
                    "{} 引用的 fixture 不存在: {}",
                    path.display(),
                    fixture_path.display()
                ));
            }
            if fixture_path.join(".galen/active-task.json").is_file() {
                galen_lib::research_task::load_active_task(&fixture_path)
                    .map_err(|error| format!("{} 的 ResearchTask fixture 无效: {error}", case.id))?
                    .ok_or_else(|| format!("{} 未能加载 active ResearchTask", case.id))?;
            }
        }
        println!("OK {} {} ({})", case.id, case.name, case.suite);
    }
    println!("已验证 {} 个评测案例", cases.len());
    Ok(())
}

async fn run_cases(args: &[String]) -> Result<(), String> {
    let case_id = option(args, "--case").unwrap_or_else(|| usage());
    let cases_dir = PathBuf::from(option(args, "--cases").unwrap_or_else(default_cases_dir));
    let variant_override = option(args, "--variant");
    let repeat = option(args, "--repeat")
        .unwrap_or_else(|| "1".to_string())
        .parse::<u32>()
        .map_err(|_| "--repeat 必须是正整数".to_string())?;
    if repeat == 0 {
        return Err("--repeat 必须大于 0".to_string());
    }
    let cases = discover_cases(&cases_dir)?;
    let (_, case) = cases
        .into_iter()
        .find(|(_, case)| case.id.eq_ignore_ascii_case(&case_id))
        .ok_or_else(|| format!("没有找到案例 {case_id}"))?;
    let mut case = case;
    if let Some(variant) = &variant_override {
        case.context.variant = match variant.as_str() {
            "none" => ContextVariant::None,
            "full" => ContextVariant::Full,
            "compacted" => ContextVariant::Compacted,
            "skeleton" => ContextVariant::SkeletonOnly,
            "fullpack" => ContextVariant::FullPack,
            other => {
                return Err(format!(
                    "--variant 必须是 none/full/compacted/skeleton/fullpack，收到: {other}"
                ))
            }
        };
    }
    let eval_root = cases_dir.parent().unwrap_or_else(|| Path::new("."));
    let output = PathBuf::from(option(args, "--output").unwrap_or_else(|| {
        eval_root
            .join("runs")
            .join(format!("{}-{}.jsonl", case_id, git_commit()))
            .display()
            .to_string()
    }));
    let router = ModelRouter::load().map_err(|error| format!("加载 models.toml 失败: {error}"))?;
    let model_alias = option(args, "--model").unwrap_or_else(|| router.default_alias().to_string());
    let persona = match option(args, "--persona").as_deref() {
        Some("none") | Some("off") => no_persona(),
        Some(other) => {
            return Err(format!("--persona 必须是 medical 或 none，收到: {other}"));
        }
        _ => medical_persona(),
    };
    for run_index in 1..=repeat {
        let record = run_once(&case, eval_root, &model_alias, run_index, &persona).await?;
        append_jsonl(&output, &record)?;
        println!(
            "{} run={} pass={} quality={:.3} ttfr_ms={:?} total_ms={}",
            record.case_id,
            run_index,
            record.hard_gates_passed,
            record.quality_score,
            record.latency.ttfr_ms,
            record.latency.total_ms
        );
    }
    println!("评测记录: {}", output.display());
    let report = reliability_report(&load_jsonl(&output)?, repeat.min(5) as usize);
    println!(
        "可靠性: success={}/{} lower95={:.3} pass^{}={:?} GAI={:.1} qualified={}",
        report.successes,
        report.runs,
        report.wilson_lower_95,
        report.pass_k_k,
        report.pass_k,
        report.galen_agent_index,
        report.qualified
    );
    Ok(())
}

fn reliability(args: &[String]) -> Result<(), String> {
    let input = PathBuf::from(option(args, "--input").unwrap_or_else(|| usage()));
    let k = option(args, "--k")
        .unwrap_or_else(|| "5".to_string())
        .parse::<usize>()
        .map_err(|_| "--k 必须是正整数".to_string())?;
    if k == 0 {
        return Err("--k 必须大于 0".to_string());
    }
    let report = reliability_report(&load_jsonl(&input)?, k);
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|error| format!("序列化可靠性报告失败: {error}"))?
    );
    Ok(())
}

fn rescore(args: &[String]) -> Result<(), String> {
    let input = PathBuf::from(option(args, "--input").unwrap_or_else(|| usage()));
    let output = PathBuf::from(option(args, "--output").unwrap_or_else(|| usage()));
    if output.exists() {
        return Err(format!("拒绝覆盖已存在的重评分文件: {}", output.display()));
    }
    let cases_dir = PathBuf::from(option(args, "--cases").unwrap_or_else(default_cases_dir));
    let cases = discover_cases(&cases_dir)?
        .into_iter()
        .map(|(_, case)| (case.id.clone(), case))
        .collect::<std::collections::HashMap<_, _>>();
    let records = load_jsonl(&input)?;
    for old in &records {
        let case = cases
            .get(&old.case_id)
            .ok_or_else(|| format!("重评分找不到案例 {}", old.case_id))?;
        let summary = ChatRunSummary {
            iterations: old
                .tool_trace
                .iter()
                .map(|trace| trace.turn)
                .max()
                .unwrap_or(0),
            tool_call_count: old.tools.calls,
            model_request_count: old.model_requests,
            input_tokens: old.usage.input,
            output_tokens: old.usage.output,
            cache_creation_input_tokens: old.usage.cache_create,
            cache_read_input_tokens: old.usage.cache_read,
            context_assembly_ms: old.latency.context_ms,
            mcp_setup_ms: old.latency.mcp_ms,
            ttft_ms: old.latency.ttft_ms,
            ttfr_ms: old.latency.ttfr_ms,
            total_ms: old.latency.total_ms,
            compaction_count: old.context.compactions,
            ..ChatRunSummary::default()
        };
        let run_ok = old
            .assertions
            .iter()
            .find(|assertion| assertion.name == "run_completed")
            .is_some_and(|assertion| assertion.pass);
        let rescored = RunRecord::evaluate(
            case,
            RunObservation {
                commit: &old.commit,
                model: &old.model,
                run_index: old.run_index,
                run_ok,
                response: &old.final_response,
                summary: &summary,
                traces: &old.tool_trace,
                workspace: Path::new(&old.workspace),
                summary_field_coverage: old.context.summary_field_coverage,
            },
        );
        append_jsonl(&output, &rescored)?;
    }
    let report = reliability_report(&load_jsonl(&output)?, 1);
    println!(
        "重评分完成: success={}/{} lower95={:.3} GAI={:.1} output={}",
        report.successes,
        report.runs,
        report.wilson_lower_95,
        report.galen_agent_index,
        output.display()
    );
    Ok(())
}

fn compare(args: &[String]) -> Result<(), String> {
    let baseline = PathBuf::from(option(args, "--baseline").unwrap_or_else(|| usage()));
    let candidate = PathBuf::from(option(args, "--candidate").unwrap_or_else(|| usage()));
    let ignore_config = args.iter().any(|arg| arg == "--ignore-config");
    let report = if ignore_config {
        compare_runs_ignore_config(&load_jsonl(&baseline)?, &load_jsonl(&candidate)?)
    } else {
        compare_runs(&load_jsonl(&baseline)?, &load_jsonl(&candidate)?)
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|error| format!("序列化比较报告失败: {error}"))?
    );
    if matches!(report.decision, ComparisonDecision::Accept) {
        Ok(())
    } else {
        Err(format!("候选版本未获准升级基线: {:?}", report.decision))
    }
}

fn rag_validate(args: &[String]) -> Result<(), String> {
    let dataset = PathBuf::from(option(args, "--dataset").unwrap_or_else(|| usage()));
    let spec = RagBenchmarkSpec::from_path(&dataset)?;
    let eval_root = dataset
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| format!("无法从 {} 推导 evals 根目录", dataset.display()))?;
    let fixture = eval_root.join(&spec.fixture);
    if !fixture.is_dir() {
        return Err(format!("RAG fixture 不存在: {}", fixture.display()));
    }
    galen_lib::research_task::load_active_task(&fixture)
        .map_err(|error| format!("RAG fixture 的 ResearchTask 无效: {error}"))?
        .ok_or_else(|| "RAG fixture 没有活动 ResearchTask".to_string())?;
    let evidence = galen_lib::evidence::load_evidence(&fixture)?;
    let evidence_ids = evidence
        .iter()
        .map(|item| item.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    for query in &spec.queries {
        for id in query.relevant.iter().chain(&query.forbidden) {
            if !evidence_ids.contains(id.as_str()) {
                return Err(format!(
                    "查询 {} 引用了 fixture 中不存在的证据 {id}",
                    query.id
                ));
            }
        }
    }
    println!(
        "OK RAG dataset={} queries={} evidence={} top_k={}",
        spec.id,
        spec.queries.len(),
        evidence.len(),
        spec.top_k
    );
    Ok(())
}

fn rag_run(args: &[String]) -> Result<(), String> {
    let dataset = PathBuf::from(option(args, "--dataset").unwrap_or_else(|| usage()));
    let repeat = option(args, "--repeat")
        .unwrap_or_else(|| "10".to_string())
        .parse::<usize>()
        .map_err(|_| "--repeat 必须是正整数".to_string())?;
    if repeat == 0 {
        return Err("--repeat 必须大于 0".to_string());
    }
    rag_validate(args)?;
    let spec = RagBenchmarkSpec::from_path(&dataset)?;
    let eval_root = dataset
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| format!("无法从 {} 推导 evals 根目录", dataset.display()))?;
    let fixture = eval_root.join(&spec.fixture);
    let workspace = std::env::temp_dir().join("galen-rag-evals").join(format!(
        "{}-{}-{}",
        spec.id,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or_default()
    ));
    copy_fixture(&fixture, &workspace)?;
    let report = run_rag_benchmark(&spec, &workspace, &git_commit(), repeat);
    let _ = std::fs::remove_dir_all(&workspace);
    let report = report?;
    let output = PathBuf::from(option(args, "--output").unwrap_or_else(|| {
        eval_root
            .join("runs")
            .join(format!(
                "rag-{}-{}-{}.json",
                spec.id,
                git_commit(),
                report.started_at_ms
            ))
            .display()
            .to_string()
    }));
    write_rag_report(&output, &report)?;
    println!(
        "RAG dataset={} pass={} Recall@{}={:.3} MRR={:.3} nDCG@{}={:.3} negative_accuracy={:.3} P95={:.1}ms cold={}ms forbidden={}",
        report.dataset_id,
        report.hard_gates_passed,
        report.top_k,
        report.aggregate.recall_at_k,
        report.aggregate.mrr,
        report.top_k,
        report.aggregate.ndcg_at_k,
        report.aggregate.negative_query_accuracy,
        report.aggregate.latency_p95_ms,
        report.aggregate.cold_index_ms,
        report.aggregate.forbidden_hits
    );
    println!("RAG 评测报告: {}", output.display());
    if report.hard_gates_passed {
        Ok(())
    } else {
        Err("RAG benchmark 未通过硬门禁".to_string())
    }
}

fn rag_compare(args: &[String]) -> Result<(), String> {
    let baseline_path = PathBuf::from(option(args, "--baseline").unwrap_or_else(|| usage()));
    let candidate_path = PathBuf::from(option(args, "--candidate").unwrap_or_else(|| usage()));
    let report = compare_rag_reports(
        &load_rag_report(&baseline_path)?,
        &load_rag_report(&candidate_path)?,
    );
    if let Some(output) = option(args, "--output") {
        write_rag_report(Path::new(&output), &report)?;
        println!("RAG 对比报告: {output}");
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|error| format!("序列化 RAG 对比报告失败: {error}"))?
        );
    }
    if matches!(report.decision, ComparisonDecision::Accept) {
        Ok(())
    } else {
        Err(format!("候选 RAG 未获准升级基线: {:?}", report.decision))
    }
}

fn report(args: &[String]) -> Result<(), String> {
    let agent_path = option(args, "--agent").map(PathBuf::from);
    let rag_path = option(args, "--rag").map(PathBuf::from);
    if agent_path.is_none() && rag_path.is_none() {
        return Err("report 至少需要 --agent 或 --rag".to_string());
    }
    let output = PathBuf::from(option(args, "--output").unwrap_or_else(|| usage()));
    let title = option(args, "--title").unwrap_or_else(|| "Galen 测评报告".to_string());
    let agent_records = agent_path
        .as_deref()
        .map(load_jsonl)
        .transpose()?
        .unwrap_or_default();
    let rag = rag_path.as_deref().map(load_rag_report).transpose()?;
    let markdown = render_markdown_report(&title, &agent_records, rag.as_ref());
    write_markdown_report(&output, &markdown)?;
    println!("测评报告: {}", output.display());
    Ok(())
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = args.first().map(String::as_str) else {
        usage();
    };
    let result = match command {
        "validate" => validate(&args[1..]),
        "rescore" => rescore(&args[1..]),
        "reliability" => reliability(&args[1..]),
        "compare" => compare(&args[1..]),
        "rag-validate" => rag_validate(&args[1..]),
        "rag" => rag_run(&args[1..]),
        "rag-compare" => rag_compare(&args[1..]),
        "report" => report(&args[1..]),
        "run" => tokio::runtime::Runtime::new()
            .map_err(|error| format!("创建 Tokio runtime 失败: {error}"))
            .and_then(|runtime| runtime.block_on(run_cases(&args[1..]))),
        _ => usage(),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
