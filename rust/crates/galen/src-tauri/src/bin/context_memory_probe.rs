use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use galen_lib::backend::{run_chat, ChatEvent, ChatRunSummary, ToolTrace};
use galen_lib::chat_session;
use galen_lib::modes::ChatMode;
use galen_lib::personas::medical_persona;
use medical_core::MedicalCore;
use model_router::ModelRouter;
use serde::Serialize;

const TURN_PROMPTS: [&str; 3] = [
    "我们正在设计一个脑卒中上肢康复先导随机试验。请记住以下项目约束，并深入讨论为什么它们彼此匹配：协议号 GALEN-CONTEXT-73，样本量 48，主要结局 FMA-UE，随访 12 周。不要创建文件；重点讨论设计逻辑、风险和可能的混杂因素。",
    "承接上一轮，不要让我重复项目约束。请读取 inputs/eligibility.md，把其中的资格标准与上一轮试验设计整合，使用工具生成 output/context-protocol.md。文件必须包含协议号、样本量、主要结局、随访时间，以及资格标准中的内部证据代码。为了测试工具记忆隔离，最终聊天只回复“已完成文件写入”，不要在聊天回答中复述资格标准、证据代码或项目约束。",
    "现在做一次深入的方案复盘，不要调用工具：把随访从原来的 12 周修订为 16 周，但其他核心约束不变。请明确列出协议号、样本量、主要结局、旧随访、新随访，并回忆上一轮工具读取到的排除标准和内部证据代码；最后分析这次修订的利弊。",
];

const FATIGUE_SCOPE_TURN_PROMPTS: [&str; 3] = [
    "我们最初曾讨论过脑卒中上肢康复与中西医结合方向。现在先记录这段旧讨论：它不是当前研究范围。不要创建文件；只说明旧方向已经归档，等待新的项目定义。",
    "现在正式切换到新项目：运动疲劳恢复研究，协议号 GALEN-FATIGUE-01。研究对象为 12 名健康受试者，连续采集 48 个训练后评估时点；核心变量为 HRV、反向跳 CMJ 和主观用力程度 RPE，观察窗为训练后 0、24、48、72 小时。此前对话中的全部历史方向均已归档，不属于当前项目，也不得出现在新文件或最终聊天中。请读取 inputs/fatigue-cohort.md，并据此使用工具生成 output/fatigue-protocol.md。文件必须包含协议号、样本与时点、三类核心变量、72 小时观察窗及数据字典中的证据代码。最终聊天只回复“已完成文件写入”。",
    "承接当前项目，不要让我重复研究范围，也不要调用工具。请给出 GALEN-FATIGUE-01 的分析方案：明确研究对象、时点、HRV/CMJ/RPE 的纵向分析逻辑及数据字典证据代码。历史方向已经退出，最终回答不得提及它们。",
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TurnProbe {
    turn: usize,
    prompt: String,
    history_message_count: usize,
    response: String,
    traces: Vec<ToolTrace>,
    summary: ChatRunSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Assertion {
    name: String,
    pass: bool,
    detail: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ContextMemoryReport {
    probe: &'static str,
    scenario: String,
    model: String,
    workspace: String,
    passed: bool,
    assertions: Vec<Assertion>,
    turns: Vec<TurnProbe>,
    session_message_count: usize,
    total_input_tokens: u64,
    total_output_tokens: u64,
    total_duration_ms: u64,
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn option(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|argument| argument == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn prepare_workspace(requested: Option<String>, scenario: &str) -> Result<PathBuf, String> {
    let workspace = requested.map(PathBuf::from).unwrap_or_else(|| {
        std::env::temp_dir().join("galen-probes").join(format!(
            "context-memory-{}-{}",
            std::process::id(),
            now_millis()
        ))
    });
    if workspace.exists()
        && std::fs::read_dir(&workspace)
            .map_err(|error| format!("读取探针工作区失败: {error}"))?
            .next()
            .is_some()
    {
        return Err(format!("探针拒绝复用非空工作区: {}", workspace.display()));
    }
    std::fs::create_dir_all(workspace.join("inputs"))
        .map_err(|error| format!("创建探针输入目录失败: {error}"))?;
    std::fs::create_dir_all(workspace.join("output"))
        .map_err(|error| format!("创建探针输出目录失败: {error}"))?;
    std::fs::write(
        workspace.join("inputs/eligibility.md"),
        "# 资格标准\n\n- 排除：近 3 个月内接受过肉毒毒素注射。\n- 内部证据代码：E-TOOL-29。\n",
    )
    .map_err(|error| format!("写入探针输入失败: {error}"))?;
    if scenario == "fatigue_scope" {
        std::fs::write(
            workspace.join("inputs/fatigue-cohort.md"),
            "# GALEN-FATIGUE-01 数据字典\n\n- 受试者：12 名健康受试者。\n- 评估：48 个训练后评估时点；0、24、48、72 小时。\n- 核心变量：HRV、CMJ、RPE。\n- 数据字典证据代码：REHABID-FATIGUE-48。\n",
        )
        .map_err(|error| format!("写入疲劳探针输入失败: {error}"))?;
    }
    Ok(workspace)
}

fn default_output() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../evals");
    root.join("runs")
        .join(format!("context-memory-probe-{}.json", now_millis()))
}

fn contains_all(text: &str, facts: &[&str]) -> bool {
    let normalized = text.to_lowercase();
    facts
        .iter()
        .all(|fact| normalized.contains(&fact.to_lowercase()))
}

fn assertion(name: &str, pass: bool, detail: impl Into<String>) -> Assertion {
    Assertion {
        name: name.to_string(),
        pass,
        detail: detail.into(),
    }
}

fn response_looks_complete(text: &str) -> bool {
    text.trim_end()
        .chars()
        .last()
        .is_some_and(|last| matches!(last, '。' | '！' | '？' | '.' | '!' | '?' | ')' | '）'))
}

async fn execute_turn(
    workspace: &Path,
    router: ModelRouter,
    model_alias: &str,
    model_id: &str,
    turn: usize,
    prompt: &str,
    timeout_seconds: u64,
    discussion_thinking: &str,
) -> Result<TurnProbe, String> {
    let history = chat_session::prepare_model_history(workspace, None, model_id, Vec::new())?;
    let history_message_count = history.len();
    let response = Arc::new(Mutex::new(String::new()));
    let response_sink = response.clone();
    let traces = Arc::new(Mutex::new(Vec::<ToolTrace>::new()));
    let trace_sink = traces.clone();
    let prompt = prompt.to_string();
    let mode = if turn == 2 {
        ChatMode::Auto
    } else {
        ChatMode::Discuss
    };
    let thinking_level = if turn == 2 {
        "off"
    } else {
        discussion_thinking
    };
    let started_at_ms = u64::try_from(now_millis()).unwrap_or(u64::MAX);

    println!(
        "turn={turn} history_messages={history_message_count} mode={mode:?} thinking={thinking_level}"
    );
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_seconds),
        run_chat(
            model_alias.to_string(),
            model_id.to_string(),
            prompt.clone(),
            history,
            mode,
            medical_persona(),
            thinking_level.to_string(),
            Arc::new(MedicalCore::new(None)),
            router,
            Mutex::new(Some(workspace.to_path_buf())),
            Some(trace_sink),
            move |event| {
                if let ChatEvent::Done(text) = event {
                    if let Ok(mut value) = response_sink.lock() {
                        *value = text;
                    }
                }
            },
        ),
    )
    .await
    .map_err(|_| format!("第 {turn} 轮超过 {timeout_seconds} 秒"))??;

    let response = response
        .lock()
        .map_err(|_| "响应记录锁已损坏".to_string())?
        .clone();
    if response.trim().is_empty() {
        return Err(format!("第 {turn} 轮没有最终回答"));
    }
    let traces = traces
        .lock()
        .map_err(|_| "工具轨迹锁已损坏".to_string())?
        .clone();
    chat_session::append_exchange(
        workspace,
        None,
        model_alias,
        &prompt,
        &response,
        &traces,
        started_at_ms,
        &result,
    )?;

    println!(
        "turn={turn} done total_ms={} tokens={}/{} tools={}",
        result.total_ms,
        result.input_tokens,
        result.output_tokens,
        traces
            .iter()
            .map(|trace| trace.tool.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );
    Ok(TurnProbe {
        turn,
        prompt,
        history_message_count,
        response,
        traces,
        summary: result,
    })
}

async fn run(args: &[String]) -> Result<ContextMemoryReport, String> {
    let router = ModelRouter::load().map_err(|error| format!("加载 models.toml 失败: {error}"))?;
    let scenario = option(args, "--scenario").unwrap_or_else(|| "constraint_revision".to_string());
    let turn_prompts = match scenario.as_str() {
        "constraint_revision" => &TURN_PROMPTS,
        "fatigue_scope" => &FATIGUE_SCOPE_TURN_PROMPTS,
        _ => return Err("--scenario 仅支持 constraint_revision 或 fatigue_scope".to_string()),
    };
    let model_alias = option(args, "--model").unwrap_or_else(|| router.default_alias().to_string());
    let model_id = router.resolve_model_id(&model_alias);
    let timeout_seconds = option(args, "--timeout")
        .unwrap_or_else(|| "300".to_string())
        .parse::<u64>()
        .map_err(|_| "--timeout 必须是正整数".to_string())?;
    let discussion_thinking =
        option(args, "--discussion-thinking").unwrap_or_else(|| "high".to_string());
    if !matches!(
        discussion_thinking.as_str(),
        "off" | "low" | "medium" | "high"
    ) {
        return Err("--discussion-thinking 必须是 off、low、medium 或 high".to_string());
    }
    let workspace = prepare_workspace(option(args, "--workspace"), &scenario)?;
    let output = option(args, "--output")
        .map(PathBuf::from)
        .unwrap_or_else(default_output);

    println!(
        "probe=context-memory scenario={scenario} model={model_alias} workspace={}",
        workspace.display()
    );
    let mut turns = Vec::new();
    for turn in 1..=3 {
        turns.push(
            execute_turn(
                &workspace,
                router.clone(),
                &model_alias,
                &model_id,
                turn,
                turn_prompts[turn - 1],
                timeout_seconds,
                &discussion_thinking,
            )
            .await?,
        );
    }

    let artifact_path = workspace.join(if scenario == "fatigue_scope" {
        "output/fatigue-protocol.md"
    } else {
        "output/context-protocol.md"
    });
    let artifact = std::fs::read_to_string(&artifact_path).unwrap_or_default();
    let session_messages = chat_session::load_messages(&workspace, None)?;
    let turn2_tools = turns[1]
        .traces
        .iter()
        .map(|trace| trace.tool.as_str())
        .collect::<Vec<_>>();
    let final_response = &turns[2].response;
    let common_assertions = vec![
        assertion(
            "turn2_received_turn1_exchange",
            turns[1].history_message_count >= 2,
            format!("history_messages={}", turns[1].history_message_count),
        ),
        assertion(
            "turn3_received_two_exchanges",
            turns[2].history_message_count >= 4,
            format!("history_messages={}", turns[2].history_message_count),
        ),
        assertion(
            "tool_round_read_and_write",
            turn2_tools.contains(&"read_file") && turn2_tools.contains(&"write_file"),
            format!("tools={}", turn2_tools.join(",")),
        ),
    ];
    let scenario_assertions = if scenario == "fatigue_scope" {
        vec![
            assertion(
                "tool_artifact_integrates_current_fatigue_scope",
                contains_all(
                    &artifact,
                    &[
                        "GALEN-FATIGUE-01",
                        "12",
                        "48",
                        "HRV",
                        "CMJ",
                        "RPE",
                        "72",
                        "REHABID-FATIGUE-48",
                    ],
                ) && !["脑卒中", "中西医", "FMA-UE"]
                    .iter()
                    .any(|term| artifact.contains(term)),
                format!(
                    "artifact={} bytes={}",
                    artifact_path.display(),
                    artifact.len()
                ),
            ),
            assertion(
                "final_turn_retains_current_project_constraints",
                contains_all(
                    final_response,
                    &[
                        "GALEN-FATIGUE-01",
                        "12",
                        "48",
                        "HRV",
                        "CMJ",
                        "RPE",
                        "72",
                        "REHABID-FATIGUE-48",
                    ],
                ),
                "required=protocol,participants,assessments,variables,window,evidence_code",
            ),
            assertion(
                "final_turn_excludes_retired_scope",
                !["脑卒中", "中西医", "FMA-UE"]
                    .iter()
                    .any(|term| final_response.contains(term)),
                "forbidden=脑卒中,中西医,FMA-UE",
            ),
        ]
    } else {
        vec![
            assertion(
                "tool_artifact_integrates_discussion_and_file",
                contains_all(
                    &artifact,
                    &["GALEN-CONTEXT-73", "48", "FMA-UE", "12 周", "E-TOOL-29"],
                ),
                format!(
                    "artifact={} bytes={}",
                    artifact_path.display(),
                    artifact.len()
                ),
            ),
            assertion(
                "final_turn_retains_original_constraints",
                contains_all(
                    final_response,
                    &["GALEN-CONTEXT-73", "48", "FMA-UE", "12 周"],
                ),
                "required=protocol,sample,outcome,old_followup",
            ),
            assertion(
                "final_turn_applies_revision",
                contains_all(final_response, &["16 周"]),
                "required=new_followup",
            ),
            assertion(
                "final_turn_retains_tool_acquired_fact",
                contains_all(final_response, &["3 个月", "肉毒毒素", "E-TOOL-29"]),
                "required=exclusion_and_internal_evidence_code",
            ),
        ]
    };
    let mut assertions = common_assertions;
    assertions.extend(scenario_assertions);
    assertions.push(assertion(
        "final_turn_response_is_complete",
        response_looks_complete(final_response),
        format!(
            "last_character={:?}",
            final_response.trim_end().chars().last()
        ),
    ));
    assertions.push(assertion(
        "durable_session_has_three_exchanges",
        session_messages.len() == 6,
        format!("session_messages={}", session_messages.len()),
    ));
    let passed = assertions.iter().all(|item| item.pass);
    let total_input_tokens = turns.iter().map(|turn| turn.summary.input_tokens).sum();
    let total_output_tokens = turns.iter().map(|turn| turn.summary.output_tokens).sum();
    let total_duration_ms = turns.iter().map(|turn| turn.summary.total_ms).sum();
    let report = ContextMemoryReport {
        probe: "context-memory",
        scenario,
        model: model_alias,
        workspace: workspace.display().to_string(),
        passed,
        assertions,
        turns,
        session_message_count: session_messages.len(),
        total_input_tokens,
        total_output_tokens,
        total_duration_ms,
    };

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("创建报告目录失败: {error}"))?;
    }
    std::fs::write(
        &output,
        serde_json::to_string_pretty(&report)
            .map_err(|error| format!("序列化探针报告失败: {error}"))?,
    )
    .map_err(|error| format!("写入探针报告失败: {error}"))?;
    println!(
        "{} context-memory total_ms={} tokens={}/{} report={}",
        if report.passed { "PASS" } else { "FAIL" },
        report.total_duration_ms,
        report.total_input_tokens,
        report.total_output_tokens,
        output.display()
    );
    for item in report.assertions.iter().filter(|item| !item.pass) {
        eprintln!("  FAIL {}: {}", item.name, item.detail);
    }
    Ok(report)
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|error| {
        eprintln!("创建 Tokio runtime 失败: {error}");
        std::process::exit(2);
    });
    match runtime.block_on(run(&args)) {
        Ok(report) if report.passed => {}
        Ok(_) => std::process::exit(1),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}
