use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use api::{
    ContentBlockDelta, InputContentBlock, InputMessage, MessageRequest, OutputContentBlock,
    StreamEvent as ApiStreamEvent, ThinkingConfig, ToolChoice, ToolResultContentBlock, Usage,
};
use medical_core::MedicalCore;
use model_router::ModelRouter;

use crate::backend::{
    make_client, record_trace, timing_probe, ChatEvent, ChatRunSummary, ModelRequestTiming,
    PendingToolCall, TraceSink,
};
use crate::context_compaction::{archive_compaction, summarize_middle};
use crate::context_engine::{
    build_system_prompt_for_contract, build_turn_context, compact_tool_result,
    compact_trigger_bytes, normalize_workspace_tool_input, select_tools_for_contract,
    validate_tool_call_against_contract,
};
use crate::task_contract::{
    compile_task_contract, is_local_data_task, TaskContract, WorkingMemory,
};
use crate::tools::{ToolContext, ToolRegistry};

type McpServerHandle = Arc<tokio::sync::Mutex<crate::mcp_client::McpServer>>;

// Chat setup owns connection creation. Coverage refreshes can only inspect
// this cache, never spawn a second MCP connection set.
static MCP_CACHE: OnceLock<Vec<McpServerHandle>> = OnceLock::new();

pub(crate) fn cached_connected_mcp_server_names() -> Vec<String> {
    MCP_CACHE
        .get()
        .into_iter()
        .flatten()
        .filter_map(|server| {
            server.try_lock().ok().and_then(|server| {
                (server.status == crate::mcp_client::McpConnectionStatus::Connected)
                    .then(|| server.name.clone())
            })
        })
        .collect()
}

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

fn has_replayable_assistant_content(
    text_blocks: &[String],
    tool_calls: &[PendingToolCall],
) -> bool {
    !text_blocks.is_empty() || !tool_calls.is_empty()
}

fn is_output_limit_stop(stop_reason: Option<&str>) -> bool {
    matches!(
        stop_reason,
        Some("length" | "max_tokens" | "max_output_tokens")
    )
}

fn response_requires_continuation(text: &str) -> bool {
    let trimmed = text.trim_end();
    if trimmed.is_empty() || trimmed.matches("```").count() % 2 == 1 {
        return true;
    }
    !trimmed.chars().last().is_some_and(|last| {
        matches!(
            last,
            '。' | '！' | '？' | '.' | '!' | '?' | ')' | '）' | ']' | '】'
        )
    })
}

fn thinking_config_for_level(
    thinking_level: &str,
    model_id: &str,
) -> (Option<String>, Option<ThinkingConfig>) {
    let disabled = || {
        (
            None,
            Some(ThinkingConfig {
                thinking_type: "disabled".to_string(),
            }),
        )
    };
    // The current DeepSeek API maps Pro `low` back to high effort. Treat the
    // product's default low setting as its fast/non-thinking lane; users can
    // still select medium (high effort) or high (max effort) explicitly.
    if thinking_level == "off"
        || (thinking_level == "low" && model_id.to_ascii_lowercase().contains("v4-pro"))
    {
        return disabled();
    }
    let effort = match thinking_level {
        "low" => "low",
        "high" => "max",
        _ => "high",
    };
    (
        Some(effort.to_string()),
        Some(ThinkingConfig {
            thinking_type: "enabled".to_string(),
        }),
    )
}

fn request_token_budget(
    model_max_tokens: u32,
    final_turn: bool,
    has_tools: bool,
    contract: &TaskContract,
    output_continuation_count: u32,
    fast_pro_lane: bool,
) -> u32 {
    let base = if final_turn {
        model_max_tokens.min(8_192).max(4_096)
    } else if has_tools && !contract.artifact_paths.is_empty() {
        model_max_tokens.min(4_096)
    } else if has_tools {
        model_max_tokens.min(2_048)
    } else {
        model_max_tokens.min(4_096)
    };
    let mut budget = if has_tools {
        base
    } else {
        contract
            .response_token_cap
            .map(|cap| base.min(cap))
            .unwrap_or(base)
    };
    if output_continuation_count > 0 {
        budget = budget.min(1_024);
    }
    if fast_pro_lane && !has_tools {
        budget = budget.min(1_200);
    }
    budget
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
    // Map the product's four levels onto provider-specific thinking controls.
    let (reasoning_effort, thinking) = thinking_config_for_level(&thinking_level, &model_id);
    let fast_pro_lane = model_id.to_ascii_lowercase().contains("v4-pro")
        && thinking
            .as_ref()
            .is_some_and(|config| config.thinking_type == "disabled");

    // Build cache-stable prefix
    let context_started = Instant::now();
    let task_kind = model_router::TaskKind::from_intent(&user_message);
    let task_contract = compile_task_contract(task_kind, &user_message);
    let mut system_prompt = build_system_prompt_for_contract(&persona, mode, &task_contract);
    // Dynamic state is refreshed every turn while the cache-stable prefix stays unchanged.
    let first_turn = history.is_empty();
    let turn_context = build_turn_context(&user_message, mode, &workspace_root, first_turn);
    let decision_context = {
        let root = workspace_root
            .lock()
            .map_err(|_| "工作区状态锁已损坏".to_string())?
            .clone();
        root.map(|root| crate::conversation_memory::render_decision_context(&root))
            .transpose()?
            .flatten()
            .unwrap_or_default()
    };
    let context_assembly_ms = context_started.elapsed().as_millis() as u64;
    timing_probe("run_chat:context_ready");

    let mut history = history; // mutable copy
                               // The frontend stores the user's raw text, so adding a fresh context envelope
                               // here does not accumulate duplicate snapshots in later requests.
    let consensus_tail = if decision_context.is_empty() {
        String::new()
    } else {
        format!("\n\n{decision_context}")
    };
    let advertised_response_cap = if fast_pro_lane {
        1_200
    } else {
        task_contract.response_token_cap.unwrap_or(3_072)
    };
    let first_user_text = format!(
        "{turn_context}{consensus_tail}\n\n## 本轮回答预算\n最终文字回复请在约 {advertised_response_cap} 个输出 Token 内完整收敛，先给结论，删去重复铺垫；不得依赖达到长度上限后续写。工具参数中的 Artifact 正文不受最终回复篇幅限制，必须保证 JSON 完整可解析。\n\n---\n\n用户: {user_message}"
    );
    history.push(InputMessage {
        role: "user".to_string(),
        content: vec![InputContentBlock::Text {
            text: first_user_text,
        }],
    });

    // Build tool registry and shared context
    let mut registry = ToolRegistry::configured();
    let mcp_started = Instant::now();
    // Cache MCP connections globally — connect once, reuse across turns.
    {
        timing_probe("run_chat:mcp_start");
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
    // Long research tasks need a generous internal runway. The ceiling remains
    // a safety mechanism, but users receive a final synthesis rather than an
    // exposed "max tool calls" failure.
    let max_tool_turns = task_contract.max_tool_turns.max(36);
    let mut last_tool_name: Option<String> = None;
    let mut same_tool_streak: u32 = 0;
    let mut con_error_streak: u32 = 0;
    let mut final_chance_used = false;
    let mut final_turn = false;
    let mut empty_response_retried = false;
    let mut stream_retry_count = 0_u32;
    let mut output_continuation_count = 0_u32;
    let mut accumulated_final_text = String::new();
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
    // Exact-path read/write tasks are deterministic workflows, not open-ended
    // exploration. Execute the declared reads in order so a model cannot infer
    // a missing file from the workspace listing and falsely claim it ran the
    // requested probe. The real results are replayed through normal tool-use
    // messages and remain visible in the immutable trace.
    for (index, path) in task_contract.ordered_read_paths.iter().enumerate() {
        let tool_use_id = format!("contract-read-{}", index + 1);
        let input = serde_json::json!({ "path": path });
        let result = registry
            .execute_dynamic("read_file", input.clone(), &ctx)
            .await;
        let (text, is_error) = match result {
            Ok(value) => (value, false),
            Err(error) => (error, true),
        };
        run_summary.tool_call_count = run_summary.tool_call_count.saturating_add(1);
        record_trace(
            &trace,
            (index + 1) as u32,
            "read_file".to_string(),
            serde_json::to_string(&input).unwrap_or_default(),
            text.clone(),
            is_error,
        );
        history.push(InputMessage {
            role: "assistant".to_string(),
            content: vec![InputContentBlock::ToolUse {
                id: tool_use_id.clone(),
                name: "read_file".to_string(),
                input,
            }],
        });
        history.push(InputMessage {
            role: "user".to_string(),
            content: vec![InputContentBlock::ToolResult {
                tool_use_id,
                content: vec![ToolResultContentBlock::Text {
                    text: compact_tool_result("read_file", &text, is_error),
                }],
                is_error,
            }],
        });
    }
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
                on_event(ChatEvent::Delta(
                    "[执行收束] 已达到本轮工具预算，正在基于已获得的证据整理可交付结论。\n".into(),
                ));
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
        if !task_contract.ordered_read_paths.is_empty() {
            tools.retain(|tool| tool.name != "read_file");
        }
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
        // Tool payloads may contain the full Artifact and need a larger budget
        // than the concise final response. Keeping them separate prevents
        // truncated JSON and duplicate write retries.
        let request_max_tokens = request_token_budget(
            max_tokens,
            final_turn,
            !tools.is_empty(),
            &task_contract,
            output_continuation_count,
            fast_pro_lane,
        );
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
        let request_started = Instant::now();
        run_summary.model_request_count = run_summary.model_request_count.saturating_add(1);
        let mut stream = client
            .stream_message(&request)
            .await
            .map_err(|e| format_api_error(&e))?;
        let attempt_count = stream.attempt_count();
        let stream_connect_ms = request_started.elapsed().as_millis() as u64;
        timing_probe(&format!("turn:{turn}:stream_connected"));

        // Collect content blocks for this response
        let mut text_blocks: Vec<String> = Vec::new();
        let mut tool_calls: Vec<PendingToolCall> = Vec::new();
        let mut current_tool: Option<PendingToolCall> = None;
        let mut current_text: String = String::new();
        let mut current_thinking: String = String::new();
        let mut request_usage = Usage::default();
        let mut stop_reason: Option<String> = None;
        let mut first_reasoning_token_ms = None;
        let mut first_visible_token_ms = None;

        let mut stream_failure = None;
        loop {
            let next_event =
                tokio::time::timeout(Duration::from_secs(30), stream.next_event()).await;
            let event = match next_event {
                Ok(event) => event,
                Err(_) => {
                    stream_failure = Some("stream idle timeout after 30s".to_string());
                    break;
                }
            };
            match event {
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
                    if event.delta.stop_reason.is_some() {
                        stop_reason = event.delta.stop_reason;
                    }
                }
                Ok(Some(ApiStreamEvent::ContentBlockStart(event))) => {
                    run_summary
                        .ttft_ms
                        .get_or_insert(run_started.elapsed().as_millis() as u64);
                    timing_probe(&format!("turn:{turn}:first_content_block"));
                    match event.content_block {
                        OutputContentBlock::Text { text } => {
                            if !text.trim().is_empty() {
                                first_visible_token_ms
                                    .get_or_insert(request_started.elapsed().as_millis() as u64);
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
                            if !thinking.trim().is_empty() {
                                first_reasoning_token_ms
                                    .get_or_insert(request_started.elapsed().as_millis() as u64);
                            }
                            on_event(ChatEvent::ThinkingDelta(thinking.clone()));
                            current_thinking = thinking;
                        }
                        _ => {}
                    }
                }
                Ok(Some(ApiStreamEvent::ContentBlockDelta(event))) => match event.delta {
                    ContentBlockDelta::TextDelta { text } => {
                        if !text.trim().is_empty() {
                            first_visible_token_ms
                                .get_or_insert(request_started.elapsed().as_millis() as u64);
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
                        if !thinking.trim().is_empty() {
                            first_reasoning_token_ms
                                .get_or_insert(request_started.elapsed().as_millis() as u64);
                        }
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
                    stream_failure = Some(format!("stream error: {e}"));
                    break;
                }
            }
        }
        run_summary.requests.push(ModelRequestTiming {
            turn,
            attempt_count,
            stream_connect_ms,
            first_reasoning_token_ms,
            first_visible_token_ms,
            total_ms: request_started.elapsed().as_millis() as u64,
            input_tokens: u64::from(request_usage.input_tokens),
            output_tokens: u64::from(request_usage.output_tokens),
            cache_hit_tokens: u64::from(request_usage.cache_read_input_tokens),
            cache_miss_tokens: u64::from(request_usage.cache_creation_input_tokens),
            stop_reason: stop_reason.clone(),
        });
        run_summary.absorb_usage(&request_usage);
        run_summary.iterations = turn;
        if let Some(error) = stream_failure {
            if stream_retry_count == 0 {
                stream_retry_count += 1;
                record_trace(
                    &trace,
                    turn,
                    "__stream_retry__".to_string(),
                    String::new(),
                    error.clone(),
                    true,
                );
                history.push(InputMessage {
                    role: "user".to_string(),
                    content: vec![InputContentBlock::Text {
                        text: "[系统恢复指令] 上一轮流式响应中断。不要重复读取已有输入，立即继续任务并完成所需产物。"
                            .to_string(),
                    }],
                });
                continue;
            }
            on_event(ChatEvent::Error(error.clone()));
            return Err(error);
        }
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
            on_event(ChatEvent::ToolProgress {
                turn,
                max_turns: max_tool_turns,
                tool: tool.name.clone(),
                phase: "running".to_string(),
            });
            let mut input: serde_json::Value =
                serde_json::from_str(&tool.input_json).unwrap_or(serde_json::Value::Null);
            normalize_workspace_tool_input(&mut input, &ctx);
            assistant_content.push(InputContentBlock::ToolUse {
                id: tool.id.clone(),
                name: tool.name.clone(),
                input,
            });
        }
        // A reasoning-only assistant turn is not valid replay history for
        // DeepSeek/OpenAI-compatible APIs: `content` and `tool_calls` would
        // both be empty. This happens when thinking consumes the whole output
        // budget. Skip it and let the recovery instruction retry cleanly.
        if !assistant_content.is_empty()
            && has_replayable_assistant_content(&text_blocks, &tool_calls)
        {
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
            let hit_output_limit = is_output_limit_stop(stop_reason.as_deref());
            if hit_output_limit
                && response_requires_continuation(&full_text)
                && output_continuation_count < 1
            {
                output_continuation_count += 1;
                accumulated_final_text.push_str(&full_text);
                history.push(InputMessage {
                    role: "user".to_string(),
                    content: vec![InputContentBlock::Text {
                        text: "[系统续写指令] 上一段回答因输出长度上限被截断。请从中断处直接续写，不要重复已有内容；压缩剩余论证，并确保用完整结论正常结束。"
                            .to_string(),
                    }],
                });
                continue;
            }
            accumulated_final_text.push_str(&full_text);
            on_event(ChatEvent::Done(std::mem::take(&mut accumulated_final_text)));
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
            let cacheable = registry.is_write_tool(&tool.name) == Some(false)
                && crate::tools::research::recognized_builtin_search(&tool.name).is_none();
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
            on_event(ChatEvent::ToolProgress {
                turn,
                max_turns: max_tool_turns,
                tool: tool.name.clone(),
                phase: if is_error { "failed" } else { "completed" }.to_string(),
            });
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

        // Search tools persist their terminal outcome while executing. Rebuild
        // the dynamic workspace-backed prompt before the next model request so
        // the final synthesis sees this turn's actual provider coverage.
        system_prompt = build_system_prompt_for_contract(&persona, mode, &task_contract);
    }

    run_summary.total_ms = run_started.elapsed().as_millis() as u64;
    run_summary.compaction_count = compaction_count;
    run_summary.stream_retry_count = stream_retry_count;
    run_summary.output_continuation_count = output_continuation_count;
    Ok(run_summary)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasoning_only_turn_is_not_replayed_as_an_empty_assistant_message() {
        assert!(!has_replayable_assistant_content(&[], &[]));
        assert!(has_replayable_assistant_content(
            &["最终回答".to_string()],
            &[]
        ));
        assert!(has_replayable_assistant_content(
            &[],
            &[PendingToolCall {
                id: "call-1".to_string(),
                name: "read_file".to_string(),
                input_json: "{}".to_string(),
            }]
        ));
    }

    #[test]
    fn provider_length_stops_trigger_bounded_continuation() {
        assert!(is_output_limit_stop(Some("length")));
        assert!(is_output_limit_stop(Some("max_tokens")));
        assert!(is_output_limit_stop(Some("max_output_tokens")));
        assert!(!is_output_limit_stop(Some("end_turn")));
        assert!(!is_output_limit_stop(None));
    }

    #[test]
    fn continuation_requires_an_incomplete_ending() {
        assert!(response_requires_continuation("结论尚未写完，下一步是"));
        assert!(response_requires_continuation("```text\n未闭合"));
        assert!(!response_requires_continuation("结论已经完整。"));
        assert!(!response_requires_continuation("最终建议见上表】"));
    }

    #[test]
    fn default_low_uses_flash_low_but_pro_fast_lane() {
        let (flash_effort, flash_thinking) = thinking_config_for_level("low", "deepseek-v4-flash");
        assert_eq!(flash_effort.as_deref(), Some("low"));
        assert_eq!(flash_thinking.unwrap().thinking_type, "enabled".to_string());

        let (pro_effort, pro_thinking) = thinking_config_for_level("low", "deepseek-v4-pro");
        assert_eq!(pro_effort, None);
        assert_eq!(pro_thinking.unwrap().thinking_type, "disabled".to_string());
        assert_eq!(
            thinking_config_for_level("medium", "deepseek-v4-pro")
                .0
                .as_deref(),
            Some("high")
        );
        assert_eq!(
            thinking_config_for_level("high", "deepseek-v4-pro")
                .0
                .as_deref(),
            Some("max")
        );
    }

    #[test]
    fn artifact_tool_payload_is_not_capped_like_final_prose() {
        let contract = compile_task_contract(
            model_router::TaskKind::Chat,
            "创建 output/delivery.md，包含研究问题、PICO、风险和下一步。",
        );
        assert_eq!(contract.response_token_cap, Some(1_200));
        assert_eq!(
            request_token_budget(64_000, false, true, &contract, 0, true),
            4_096
        );
        assert_eq!(
            request_token_budget(64_000, false, false, &contract, 0, true),
            1_200
        );
    }
}
