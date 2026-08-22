use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use api::InputMessage;
use galen_lib::backend::{run_chat, ChatEvent, ChatRunSummary, ToolTrace};
use galen_lib::eval::{
    append_jsonl, compare_runs, discover_cases, load_jsonl, ComparisonDecision, EvalCase,
    RunObservation, RunRecord,
};
use galen_lib::modes::ChatMode;
use galen_lib::personas::medical_persona;
use medical_core::MedicalCore;
use model_router::ModelRouter;

fn usage() -> ! {
    eprintln!(
        "用法:\n  eval validate [--cases evals/cases]\n  eval run --case E01 [--model alias] [--repeat 1] [--output evals/runs/run.jsonl]\n  eval compare --baseline FILE --candidate FILE"
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
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
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

async fn run_once(
    case: &EvalCase,
    eval_root: &Path,
    model_alias: &str,
    run_index: u32,
) -> Result<RunRecord, String> {
    let router = ModelRouter::load().map_err(|error| format!("加载 models.toml 失败: {error}"))?;
    let model_id = router.resolve_model_id(model_alias);
    let workspace = prepare_workspace(case, eval_root, run_index)?;
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
            Vec::<InputMessage>::new(),
            ChatMode::Auto,
            medical_persona(),
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
        }
        println!("OK {} {} ({})", case.id, case.name, case.suite);
    }
    println!("已验证 {} 个评测案例", cases.len());
    Ok(())
}

async fn run_cases(args: &[String]) -> Result<(), String> {
    let case_id = option(args, "--case").unwrap_or_else(|| usage());
    let cases_dir = PathBuf::from(option(args, "--cases").unwrap_or_else(default_cases_dir));
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
    for run_index in 1..=repeat {
        let record = run_once(&case, eval_root, &model_alias, run_index).await?;
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
    Ok(())
}

fn compare(args: &[String]) -> Result<(), String> {
    let baseline = PathBuf::from(option(args, "--baseline").unwrap_or_else(|| usage()));
    let candidate = PathBuf::from(option(args, "--candidate").unwrap_or_else(|| usage()));
    let report = compare_runs(&load_jsonl(&baseline)?, &load_jsonl(&candidate)?);
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

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = args.first().map(String::as_str) else {
        usage();
    };
    let result = match command {
        "validate" => validate(&args[1..]),
        "compare" => compare(&args[1..]),
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
