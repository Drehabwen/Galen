use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use api::{InputContentBlock, InputMessage};
use galen_lib::backend::run_chat;
use galen_lib::modes::ChatMode;
use galen_lib::personas::medical_persona;
use galen_lib::tools::{ToolContext, ToolRegistry};
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

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let router = ModelRouter::load().unwrap_or_else(|e| {
            eprintln!("加载 models.toml 失败: {e}");
            std::process::exit(1);
        });
        let Some((alias, model_entry)) = router.all_models().iter().next() else {
            eprintln!("没有可用模型");
            std::process::exit(1);
        };
        let model_alias = alias.clone();
        let model_id = model_entry.model_id.clone();
        let has_key = router
            .to_provider_config(&model_alias)
            .and_then(|c| c.api_key().map(|s| s.to_string()))
            .is_some();
        println!("== 模型: {model_alias} / {model_id} | 密钥: {} ==", if has_key { "有" } else { "无" });

        let ws = std::env::temp_dir().join("galen-driver-ws");
        setup_workspace(&ws);
        println!("== 工作区: {} ==", ws.display());

        let user_message = std::env::args().nth(1).unwrap_or_else(|| {
            "继续研究计划：从 n3 开始，完成效应量合并与异质性分析".to_string()
        });

        let mode = ChatMode::Auto;
        let persona = medical_persona();
        let medical = Arc::new(MedicalCore::new(None));
        let ws_mutex: Mutex<Option<PathBuf>> = Mutex::new(Some(ws.clone()));

        let on_event = |ev: galen_lib::backend::ChatEvent| match ev {
            galen_lib::backend::ChatEvent::ThinkingDelta(t) => print!("[思考] {t}"),
            galen_lib::backend::ChatEvent::Delta(t) => print!("{t}"),
            galen_lib::backend::ChatEvent::ThinkingDone(_) => println!("\n--- 思考结束 ---"),
            galen_lib::backend::ChatEvent::Error(e) => println!("\n[错误] {e}"),
            galen_lib::backend::ChatEvent::Done(_) => println!("\n--- 完成 ---"),
            _ => {}
        };

        let history: Vec<InputMessage> = Vec::new();
        let res = run_chat(
            model_alias.clone(),
            model_id,
            user_message,
            history,
            mode,
            persona,
            "medium".to_string(),
            medical,
            router,
            ws_mutex,
            on_event,
        )
        .await;

        match res {
            Ok(()) => println!("== run_chat OK =="),
            Err(e) => println!("== run_chat 失败: {e} =="),
        }
    });
}
