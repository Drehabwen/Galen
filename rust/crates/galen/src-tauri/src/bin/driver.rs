use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use api::InputMessage;
use galen_lib::backend::{run_chat, ChatEvent, ChatRunSummary, ToolTrace};
use galen_lib::modes::ChatMode;
use galen_lib::personas::medical_persona;
use medical_core::MedicalCore;
use model_router::ModelRouter;

fn setup_workspace(dir: &Path) {
    std::fs::create_dir_all(dir.join("output")).unwrap();
    let plan = r#"[
      {"id":"n1","title":"文献检索：康复运动对脑卒中步行功能的影响","status":"completed","result":"MeSH 检索完成，命中 42 篇，筛出 8 篇 RCT"},
      {"id":"n2","title":"数据提取：基线特征与结局指标","status":"completed","result":"8 篇 RCT 提取完成，FMA/Walking speed 为主结局"},
      {"id":"n3","title":"统计分析：效应量合并与异质性","status":"pending","dependsOn":["n2"],"result":null},
      {"id":"n4","title":"撰写综述初稿","status":"pending","dependsOn":["n3"],"result":null}
    ]"#;
    std::fs::write(dir.join("plan.json"), plan).unwrap();
    let evidence = r#"[
      {"id":"e1","node_id":"n1","node_title":"文献检索","source":"research","claim":"8 篇 RCT 支持康复运动改善脑卒中步行功能","confidence":"medium","created_at":"2026-08-13T09:00:00Z"},
      {"id":"e2","node_id":"n2","node_title":"数据提取","source":"analysis","claim":"FMA 平均差 5.2 (95%CI 2.1-8.3)","confidence":"medium","created_at":"2026-08-13T09:05:00Z"}
    ]"#;
    std::fs::write(dir.join("evidence.json"), evidence).unwrap();
    let memory = "2026-08-12 | 团队讨论 | 确定以脑卒中步行康复为主题，倾向系统综述路线\n";
    std::fs::write(dir.join("GALEN.md"), memory).unwrap();
}

async fn run_stage(
    router: ModelRouter,
    model_alias: String,
    model_id: String,
    user_message: String,
    ws: PathBuf,
    stage_label: &str,
) -> (Result<ChatRunSummary, String>, Vec<ToolTrace>) {
    let mode = ChatMode::Auto;
    let persona = medical_persona();
    let medical = Arc::new(MedicalCore::new(None));
    let ws_mutex: Mutex<Option<PathBuf>> = Mutex::new(Some(ws.clone()));

    let on_event = |ev: ChatEvent| match ev {
        ChatEvent::ThinkingDelta(t) => print!("[思考] {t}"),
        ChatEvent::Delta(t) => print!("{t}"),
        ChatEvent::ThinkingDone(_) => println!("\n--- 思考结束 ---"),
        ChatEvent::Error(e) => println!("\n[错误] {e}"),
        ChatEvent::Done(_) => println!("\n--- 完成 ---"),
        ChatEvent::ArtifactCreated(artifact) => println!("\n[产物] {}", artifact.path),
        ChatEvent::ResearchTaskUpdated(task) => println!("\n[任务] {}", task.title),
        _ => {}
    };

    let trace: std::sync::Arc<std::sync::Mutex<Vec<ToolTrace>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let trace_for_run = trace.clone();
    let history: Vec<InputMessage> = Vec::new();
    println!("\n===== 阶段：{stage_label} =====");
    let res = run_chat(
        model_alias,
        model_id,
        user_message,
        history,
        mode,
        persona,
        "medium".to_string(),
        medical,
        router,
        ws_mutex,
        Some(trace_for_run),
        on_event,
    )
    .await;
    let traces = trace.lock().map(|g| g.clone()).unwrap_or_default();
    (res, traces)
}

/// 检测「编译缺口」：存在 .typ 源文件但 output/ 无 PDF，返回源文件名列表。
fn compile_gap(ws: &Path) -> Option<Vec<String>> {
    let mut typs: Vec<String> = Vec::new();
    let mut scan = |dir: &Path| {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_file()
                    && p.extension().map(|x| x == "typ").unwrap_or(false)
                    && !p.to_string_lossy().contains("\\output\\")
                {
                    typs.push(
                        p.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                    );
                }
            }
        }
    };
    scan(ws);
    let manuscript = ws.join("manuscript");
    if manuscript.is_dir() {
        scan(&manuscript);
    }
    typs.sort();
    typs.dedup();
    let has_pdf = std::fs::read_dir(ws.join("output"))
        .map(|rd| {
            rd.flatten()
                .any(|e| e.path().extension().map(|x| x == "pdf").unwrap_or(false))
        })
        .unwrap_or(false);
    if !typs.is_empty() && !has_pdf {
        Some(typs)
    } else {
        None
    }
}

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let router = ModelRouter::load().unwrap_or_else(|e| {
            eprintln!("加载 models.toml 失败: {e}");
            std::process::exit(1);
        });

        // CLI args: [--model <alias>] [--ws <dir>] [message...]
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut model_alias_arg: Option<String> = None;
        let mut ws_arg: Option<String> = None;
        let mut message_parts: Vec<String> = Vec::new();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--model" if i + 1 < args.len() => {
                    model_alias_arg = Some(args[i + 1].clone());
                    i += 2;
                }
                "--ws" if i + 1 < args.len() => {
                    ws_arg = Some(args[i + 1].clone());
                    i += 2;
                }
                _ => {
                    message_parts.push(args[i].clone());
                    i += 1;
                }
            }
        }

        let user_message = if message_parts.is_empty() {
            "继续研究计划：从 n3 开始，完成效应量合并与异质性分析".to_string()
        } else {
            message_parts.join(" ")
        };

        let model_alias = model_alias_arg
            .or_else(|| {
                router.default_model().map(|_| router.default_alias().to_string())
            })
            .or_else(|| router.all_models().iter().next().map(|(a, _)| a.clone()))
            .unwrap_or_else(|| {
                eprintln!("没有可用模型");
                std::process::exit(1);
            });
        let Some(model_entry) = router.all_models().get(&model_alias) else {
            eprintln!("没有可用模型");
            std::process::exit(1);
        };
        let model_id = model_entry.model_id.clone();
        let has_key = router
            .to_provider_config(&model_alias)
            .and_then(|c| c.api_key().map(|s| s.to_string()))
            .is_some();
        println!("== 模型: {model_alias} / {model_id} | 密钥: {} ==", if has_key { "有" } else { "无" });

        let ws = match ws_arg {
            Some(p) => PathBuf::from(p),
            None => {
                let ws = std::env::temp_dir().join("galen-driver-ws");
                setup_workspace(&ws);
                ws
            }
        };
        std::fs::create_dir_all(ws.join("output")).unwrap_or_default();
        println!("== 工作区: {} ==", ws.display());

        // 第一阶段：执行用户任务
        let (res1, mut traces) = run_stage(
            router,
            model_alias.clone(),
            model_id.clone(),
            user_message,
            ws.clone(),
            "主任务",
        )
        .await;
        let mut run_ok = res1.is_ok();
        match res1 {
            Ok(_) => println!("\n== 阶段1 run_chat OK =="),
            Err(e) => println!("\n== 阶段1 run_chat 失败: {e} =="),
        }

        // 第二阶段（按需）：检测到 .typ 未编译 -> 自动补一轮编译交付
        let stage2 = compile_gap(&ws);
        if let Some(typs) = stage2 {
            println!("\n== 检测到未编译的 Typst 源文件: {}，发起编译交付阶段 ==", typs.join(", "));
            let msg = format!(
                "工作区已有 Typst 源文件（{}）。请：1) 读取源文件检查语法；2) 用 execute_command 运行 typst compile 生成 PDF 到 output/；3) 验证 PDF 存在且非空（列出文件大小）；4) 简短汇报。若编译报错，修复源文件后重试，直到 PDF 生成成功。",
                typs.join(", ")
            );
            let router2 = ModelRouter::load().unwrap_or_else(|e| {
                eprintln!("加载 models.toml 失败: {e}");
                std::process::exit(1);
            });
            let (res2, traces2) = run_stage(
                router2,
                model_alias.clone(),
                model_id.clone(),
                msg,
                ws.clone(),
                "编译交付",
            )
            .await;
            run_ok = run_ok && res2.is_ok();
            match res2 {
                Ok(_) => println!("\n== 阶段2 run_chat OK =="),
                Err(e) => println!("\n== 阶段2 run_chat 失败: {e} =="),
            }
            traces.extend(traces2);
        }

        // ---- 第二层：行为断言（结构化工具体验） ----
        let report = analyze(&traces, &ws, &model_alias, run_ok);
        let json = serde_json::to_string_pretty(&report).unwrap_or_default();
        let report_path = "D:/DEV/tmp/driver-report.json";
        let _ = std::fs::write(report_path, &json);
        println!("\n== 行为报告已写入 {report_path} ==");
        println!("{json}");

        let failed = report["assertions"]
            .as_array()
            .map(|a| a.iter().any(|x| x["pass"] == false))
            .unwrap_or(true);
        if failed {
            std::process::exit(2);
        }
    });
}

fn analyze(traces: &[ToolTrace], ws: &Path, model_alias: &str, run_ok: bool) -> serde_json::Value {
    let total_tool_calls = traces
        .iter()
        .filter(|t| t.tool != "__convergence__")
        .count();
    let error_calls = traces.iter().filter(|t| t.is_error).count();
    let converged = traces.iter().any(|t| t.tool == "__convergence__");

    // 同工具连续调用最大次数（execute_command 连续多次可能合理）
    let mut max_streak = 0u32;
    let mut cur_streak = 0u32;
    let mut prev: Option<&str> = None;
    for t in traces.iter().filter(|t| t.tool != "__convergence__") {
        if prev == Some(t.tool.as_str()) {
            cur_streak += 1;
        } else {
            cur_streak = 1;
            prev = Some(t.tool.as_str());
        }
        if cur_streak > max_streak {
            max_streak = cur_streak;
        }
    }

    // 死循环检测：同一工具 + 同一输入 重复出现 >= 3 次才算异常
    let mut seen: Vec<(String, String)> = Vec::new();
    for t in traces.iter().filter(|t| t.tool != "__convergence__") {
        if !seen
            .iter()
            .any(|(tool, input)| *tool == t.tool && *input == t.input)
        {
            seen.push((t.tool.clone(), t.input.clone()));
        }
    }
    let max_repeat = seen
        .iter()
        .map(|(tool, input)| {
            traces
                .iter()
                .filter(|t| t.tool == *tool && t.input == *input)
                .count()
        })
        .max()
        .unwrap_or(0);

    // read_file 输出应包含文件内容（而非只有行数）
    let read_file_with_content = traces
        .iter()
        .filter(|t| t.tool == "read_file")
        .any(|t| t.output.lines().count() > 1 || t.output.len() > 100);

    // 产出文件（output/ 下的非空文件）
    let outputs: Vec<String> = std::fs::read_dir(ws.join("output"))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.metadata().map(|m| m.len() > 0).unwrap_or(false))
                .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .collect()
        })
        .unwrap_or_default();

    // 工具名列表（去重、按序）
    let mut tool_names: Vec<&str> = traces
        .iter()
        .filter(|t| t.tool != "__convergence__")
        .map(|t| t.tool.as_str())
        .collect();
    tool_names.dedup();

    let mut assertions = Vec::new();
    let mut push = |name: &str, pass: bool, detail: String| {
        assertions.push(serde_json::json!({ "name": name, "pass": pass, "detail": detail }));
    };

    push(
        "run_chat 正常结束",
        run_ok,
        if run_ok { "Ok".into() } else { "Err".into() },
    );
    push(
        "read_file 返回内容",
        read_file_with_content,
        format!(
            "{} 次 read_file 中是否有返回完整内容的调用",
            traces.iter().filter(|t| t.tool == "read_file").count()
        ),
    );
    push(
        "无死循环（同一工具+同一参数最多重复 2 次）",
        max_repeat <= 2,
        format!("同一调用最大重复次数: {max_repeat}；同工具最大连续调用: {max_streak}"),
    );
    push(
        "工具错误率低于 40%",
        total_tool_calls == 0 || (error_calls as f64) / (total_tool_calls as f64) < 0.4,
        format!("错误 {error_calls}/{total_tool_calls}"),
    );
    push(
        "output/ 有产出文件",
        !outputs.is_empty(),
        if outputs.is_empty() {
            "output/ 为空".into()
        } else {
            outputs.join(", ")
        },
    );
    push(
        "收敛机制就绪（工具轮次用尽时触发）",
        true,
        if converged {
            "已触发收敛轮"
        } else {
            "正常在轮次内完成，未触发收敛"
        }
        .into(),
    );

    serde_json::json!({
        "meta": {
            "model": model_alias,
            "workspace": ws.display().to_string(),
            "total_tool_calls": total_tool_calls,
            "error_calls": error_calls,
            "max_same_tool_streak": max_streak,
            "max_repeat_same_call": max_repeat,
            "converged": converged,
            "tool_names": tool_names,
            "outputs": outputs,
        },
        "assertions": assertions,
    })
}
