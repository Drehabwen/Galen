use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use api::InputMessage;
use galen_lib::backend::{run_chat, ChatEvent, ChatRunSummary, ToolTrace};
use galen_lib::modes::ChatMode;
use galen_lib::personas::medical_persona;
use galen_lib::probe::{evaluate_closed_loop, ClosedLoopObservation, ProbeEventCounts};
use medical_core::MedicalCore;
use model_router::ModelRouter;

const EXPECTED_ARTIFACT: &str = "output/e2e-closed-loop.md";
const PROMPT: &str = "请为“脑卒中上肢康复依从性提升”创建一个三节点科研计划：01 明确研究问题，02 设计干预与结局，03 评估风险与下一步。立即执行节点 01，并将包含研究问题、PICO、主要结局、风险和下一步的简报写入 output/e2e-closed-loop.md。";

fn usage() -> ! {
    eprintln!(
        "用法: probe closed-loop [--model ALIAS] [--workspace DIR] [--output FILE] [--timeout SECONDS]"
    );
    std::process::exit(2);
}

fn option(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|argument| argument == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn default_output() -> PathBuf {
    let root = if Path::new("evals").is_dir() {
        PathBuf::from("evals")
    } else {
        PathBuf::from("../../../evals")
    };
    root.join("runs")
        .join(format!("probe-closed-loop-{}.json", now_millis()))
}

fn prepare_workspace(requested: Option<String>) -> Result<PathBuf, String> {
    let workspace = requested.map(PathBuf::from).unwrap_or_else(|| {
        std::env::temp_dir().join("galen-probes").join(format!(
            "closed-loop-{}-{}",
            std::process::id(),
            now_millis()
        ))
    });
    if workspace.exists() {
        let non_empty = std::fs::read_dir(&workspace)
            .map_err(|error| format!("读取探针工作区失败: {error}"))?
            .next()
            .is_some();
        if non_empty {
            return Err(format!("探针拒绝复用非空工作区: {}", workspace.display()));
        }
    }
    std::fs::create_dir_all(workspace.join("output"))
        .map_err(|error| format!("创建探针工作区失败: {error}"))?;
    Ok(workspace)
}

async fn run(args: &[String]) -> Result<bool, String> {
    let router = ModelRouter::load().map_err(|error| format!("加载 models.toml 失败: {error}"))?;
    let model = option(args, "--model").unwrap_or_else(|| router.default_alias().to_string());
    let model_id = router.resolve_model_id(&model);
    let workspace = prepare_workspace(option(args, "--workspace"))?;
    let output = option(args, "--output")
        .map(PathBuf::from)
        .unwrap_or_else(default_output);
    let timeout_seconds = option(args, "--timeout")
        .unwrap_or_else(|| "240".to_string())
        .parse::<u64>()
        .map_err(|_| "--timeout 必须是正整数".to_string())?;

    let response = Arc::new(Mutex::new(String::new()));
    let response_sink = response.clone();
    let traces: Arc<Mutex<Vec<ToolTrace>>> = Arc::new(Mutex::new(Vec::new()));
    let trace_sink = traces.clone();
    let events = Arc::new(Mutex::new(ProbeEventCounts::default()));
    let event_sink = events.clone();

    println!(
        "probe=closed-loop model={model} workspace={}",
        workspace.display()
    );
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_seconds),
        run_chat(
            model.clone(),
            model_id,
            PROMPT.to_string(),
            Vec::<InputMessage>::new(),
            ChatMode::Auto,
            medical_persona(),
            "off".to_string(),
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
                ChatEvent::ArtifactCreated(_) => {
                    if let Ok(mut value) = event_sink.lock() {
                        value.artifact_created += 1;
                    }
                }
                ChatEvent::ResearchTaskUpdated(_) => {
                    if let Ok(mut value) = event_sink.lock() {
                        value.research_task_updated += 1;
                    }
                }
                ChatEvent::Error(_) => {
                    if let Ok(mut value) = event_sink.lock() {
                        value.errors += 1;
                    }
                }
                _ => {}
            },
        ),
    )
    .await;
    let (summary, run_error) = match result {
        Ok(Ok(summary)) => (summary, None),
        Ok(Err(error)) => (ChatRunSummary::default(), Some(error)),
        Err(_) => (
            ChatRunSummary::default(),
            Some(format!("探针运行超过 {timeout_seconds} 秒")),
        ),
    };
    let response = response
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default();
    let traces = traces.lock().map(|value| value.clone()).unwrap_or_default();
    let events = events.lock().map(|value| value.clone()).unwrap_or_default();
    let report = evaluate_closed_loop(ClosedLoopObservation {
        workspace: &workspace,
        model: &model,
        expected_artifact: EXPECTED_ARTIFACT,
        response: &response,
        run_error,
        summary: &summary,
        traces: &traces,
        events,
    });
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("创建报告目录失败: {error}"))?;
    }
    let json = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("序列化探针报告失败: {error}"))?;
    std::fs::write(&output, json).map_err(|error| format!("写入探针报告失败: {error}"))?;

    println!(
        "{} closed-loop ttfr_ms={:?} total_ms={} tokens={}/{} tools={} nodes={} completed={} artifacts={} report={}",
        if report.passed { "PASS" } else { "FAIL" },
        report.metrics.ttfr_ms,
        report.metrics.total_ms,
        report.metrics.input_tokens,
        report.metrics.output_tokens,
        report.tool_names.join(","),
        report.task.as_ref().map_or(0, |task| task.node_count),
        report.task.as_ref().map_or(0, |task| task.completed_nodes),
        report.artifacts.len(),
        output.display(),
    );
    for assertion in report.assertions.iter().filter(|assertion| !assertion.pass) {
        eprintln!("  FAIL {}: {}", assertion.name, assertion.detail);
    }
    Ok(report.passed)
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) != Some("closed-loop") {
        usage();
    }
    let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|error| {
        eprintln!("创建 Tokio runtime 失败: {error}");
        std::process::exit(2);
    });
    match runtime.block_on(run(&args[1..])) {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}
