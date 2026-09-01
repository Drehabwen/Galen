#[cfg(test)]
mod context_tests {
    use std::sync::Mutex;

    use api::ToolDefinition;

    use crate::context_engine::*;
    use crate::task_contract::*;
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

    fn literature_workspace(tag: &str) -> (Mutex<Option<PathBuf>>, PathBuf, String) {
        let workspace = tmp_ws(tag, &[]);
        let root = workspace.lock().unwrap().clone().unwrap();
        let task = crate::research_task::create_task(
            &root,
            "Stroke rehabilitation review".to_string(),
            "Synthesize the literature".to_string(),
            Vec::new(),
        )
        .unwrap();
        (workspace, root, task.task_id)
    }

    fn append_literature_run(root: &std::path::Path, run: crate::search_run::SearchRun) {
        crate::search_run::append_search_run(root, &run).unwrap();
    }

    const TEST_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

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
        let contract = compile_task_contract(model_router::TaskKind::Chat, prompt);
        assert_eq!(contract.allowed_tools, Some(WRITE_ONLY_TOOLS));
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
        let stable =
            build_system_prompt_for_contract(&persona, crate::modes::ChatMode::Auto, &contract);
        assert_eq!(
            stable,
            build_system_prompt(&persona, crate::modes::ChatMode::Discuss)
        );
        assert!(!stable.contains("快速回答契约"));
    }

    #[test]
    fn deep_discussion_keeps_reasoning_but_removes_tool_overhead() {
        let contract = compile_task_contract(
            model_router::TaskKind::DeepAnalysis,
            "不要创建文件；请深入讨论这个康复试验设计的风险和混杂因素。",
        );
        assert_eq!(contract.class, TaskClass::OpenEnded);
        assert_eq!(contract.allowed_tools, Some(NO_TOOLS));
        assert!(!contract.disable_deep_reasoning);
        assert_eq!(contract.response_token_cap, Some(2_400));

        let explicit = compile_task_contract(
            model_router::TaskKind::DeepAnalysis,
            "不要调用工具，请复盘前面的方案并分析利弊。",
        );
        assert_eq!(explicit.allowed_tools, Some(NO_TOOLS));
        assert!(!explicit.disable_deep_reasoning);
    }

    #[test]
    fn explicit_input_and_output_paths_only_expose_read_and_write() {
        let contract = compile_task_contract(
            model_router::TaskKind::Chat,
            "读取 inputs/eligibility.md，并把整合结果写入 output/context-protocol.md。",
        );
        assert_eq!(contract.class, TaskClass::ArtifactCreation);
        assert_eq!(contract.allowed_tools, Some(READ_WRITE_TOOLS));
        assert_eq!(contract.max_tool_turns, 4);
        assert_eq!(
            contract.ordered_read_paths,
            vec!["inputs/eligibility.md".to_string()]
        );
        assert!(contract.execution_policy.contains("禁止 list_files"));

        let recovery = compile_task_contract(
            model_router::TaskKind::Chat,
            "先读取 inputs/missing.md，失败后读取 inputs/brief.md，并写入 output/recovery.md。",
        );
        assert!(recovery.execution_policy.contains("不得重排或省略"));
        assert!(recovery.execution_policy.contains("必须且只读取一次"));
        assert_eq!(
            recovery.ordered_read_paths,
            vec![
                "inputs/missing.md".to_string(),
                "inputs/brief.md".to_string()
            ]
        );
        let stable = build_system_prompt(
            &crate::personas::find_persona("medical"),
            crate::modes::ChatMode::Auto,
        );
        assert!(stable.contains("禁止根据工作区清单推断结果"));
        let ws = tmp_ws("ordered_path_probe", &[("brief.md", "FMA-UE")]);
        let dynamic = build_turn_context(
            "先读取 inputs/missing.md，失败后读取 inputs/brief.md，并写入 output/recovery.md。",
            crate::modes::ChatMode::Auto,
            &ws,
            true,
        );
        assert!(dynamic.contains("不展开工作区清单"));
        assert!(!dynamic.contains("[FILE] brief.md"));

        let evidence_word_must_not_override_paths = compile_task_contract(
            model_router::TaskKind::DeepAnalysis,
            "读取 inputs/eligibility.md，把其中证据代码整合并写入 output/context-protocol.md。",
        );
        assert_eq!(
            evidence_word_must_not_override_paths.allowed_tools,
            Some(READ_WRITE_TOOLS)
        );
    }

    #[test]
    fn explicit_search_request_does_not_use_direct_answer_contract() {
        let prompt = "请检索最新文献后简短回答";
        let contract = compile_task_contract(model_router::TaskKind::QuickLookup, prompt);
        assert_ne!(contract.class, TaskClass::DirectAnswer);
        assert!(!contract.disable_deep_reasoning);
    }

    #[test]
    fn literature_coverage_renders_searched_zero_and_cnki_failure_truthfully() {
        // Conflating a zero-result success with failure, or a CNKI failure with
        // evidence absence, would make the synthesis boundary actively false.
        let providers = vec![
            crate::search_run::ProviderDescriptor::configured("pubmed", true, true),
            crate::search_run::ProviderDescriptor::configured("cnki", true, true),
        ];
        let pubmed = crate::search_run::SearchRun::succeeded(
            "task-1",
            "pubmed",
            "search_pubmed",
            "stroke rehabilitation",
            serde_json::json!({"query": "stroke rehabilitation"}),
            "100",
            "200",
            0,
            TEST_HASH,
        )
        .unwrap();
        let cnki = crate::search_run::SearchRun::failed(
            "task-1",
            "cnki",
            "cnki_search",
            "脑卒中康复",
            serde_json::json!({"query": "脑卒中康复"}),
            "100",
            "210",
            crate::search_run::SearchErrorClass::Unavailable,
            TEST_HASH,
        )
        .unwrap();
        let coverage = crate::commands::literature_coverage_from_runs(
            Some("task-1"),
            &providers,
            &[pubmed, cnki],
        );

        let prompt = render_literature_coverage_context(&coverage);

        assert!(prompt.contains("## Literature coverage"));
        assert!(prompt.contains("PubMed: searched (0 results)"));
        assert!(prompt.contains("CNKI: failed; do not infer absence of Chinese evidence"));
        assert!(prompt.contains("based on searched providers"));
    }

    #[test]
    fn complete_configured_coverage_does_not_add_a_limitation_claim() {
        // Applying the qualifier after every successful configured search
        // would obscure the meaningful incomplete-coverage boundary.
        let providers = vec![
            crate::search_run::ProviderDescriptor::configured("pubmed", true, true),
            crate::search_run::ProviderDescriptor::not_configured("cnki"),
        ];
        let pubmed = crate::search_run::SearchRun::succeeded(
            "task-1",
            "pubmed",
            "search_pubmed",
            "stroke rehabilitation",
            serde_json::Value::Null,
            "100",
            "200",
            2,
            TEST_HASH,
        )
        .unwrap();
        let coverage =
            crate::commands::literature_coverage_from_runs(Some("task-1"), &providers, &[pubmed]);

        let prompt = render_literature_coverage_context(&coverage);

        assert!(prompt.contains("PubMed: searched (2 results)"));
        assert!(!prompt.contains("based on searched providers"));
    }

    #[test]
    fn direct_answer_keeps_dynamic_coverage_out_but_explicit_literature_intent_gets_it() {
        // Removing the direct-answer early return would regress the fast path;
        // omitting coverage from explicit synthesis/search would hide limits.
        let (workspace, root, task_id) = literature_workspace("coverage_intent_boundary");
        append_literature_run(
            &root,
            crate::search_run::SearchRun::succeeded(
                &task_id,
                "pubmed",
                "search_pubmed",
                "stroke rehabilitation",
                serde_json::Value::Null,
                "100",
                "200",
                0,
                TEST_HASH,
            )
            .unwrap(),
        );

        let direct = build_turn_context(
            "请用不超过 180 字解释 FMA-UE 的用途。直接回答。",
            crate::modes::ChatMode::Auto,
            &workspace,
            true,
        );
        let synthesis = build_turn_context(
            "请基于现有文献证据简短综合脑卒中康复结论。",
            crate::modes::ChatMode::Auto,
            &workspace,
            true,
        );

        assert!(!direct.contains("## Literature coverage"));
        assert!(synthesis.contains("## Literature coverage"));
        assert!(synthesis.contains("PubMed: searched (0 results)"));
    }

    #[test]
    fn coverage_dto_is_task_scoped_and_secret_free_by_construction() {
        // Returning full configuration or SearchRun objects would expose MCP
        // environment values, tool arguments, hashes, and internal record IDs.
        let (_workspace, root, task_id) = literature_workspace("coverage_dto");
        append_literature_run(
            &root,
            crate::search_run::SearchRun::succeeded(
                &task_id,
                "pubmed",
                "search_pubmed",
                "stroke rehabilitation",
                serde_json::json!({"apiKey": "must-not-leak"}),
                "100",
                "200",
                0,
                TEST_HASH,
            )
            .unwrap(),
        );
        let providers = vec![crate::search_run::ProviderDescriptor::configured(
            "pubmed", true, true,
        )];

        let response =
            crate::commands::literature_coverage_for_workspace(&root, &providers).unwrap();
        let json = serde_json::to_string(&response).unwrap();

        assert_eq!(response.task_id.as_deref(), Some(task_id.as_str()));
        assert!(json.contains("\"displayName\":\"PubMed\""));
        assert!(json.contains("\"resultCount\":0"));
        for forbidden in [
            "must-not-leak",
            "arguments",
            "rawResultHash",
            "toolName",
            "command",
            "env",
        ] {
            assert!(!json.contains(forbidden), "coverage DTO leaked {forbidden}");
        }
    }

    #[test]
    fn unreadable_coverage_is_reported_as_unavailable_not_as_no_active_task() {
        // Swallowing a ledger error into the no-task fallback would falsely
        // describe a coverage failure as absence of task-scoped provenance.
        let (workspace, root, task_id) = literature_workspace("coverage_unreadable");
        let ledger = root
            .join(".galen")
            .join("tasks")
            .join(task_id)
            .join("search-runs.jsonl");
        std::fs::write(ledger, "not-json\n").unwrap();

        let context = build_turn_context(
            "请综合现有文献证据。",
            crate::modes::ChatMode::Auto,
            &workspace,
            true,
        );

        assert!(context.contains("Literature coverage is unavailable"));
        assert!(!context.contains("No active research task"));
        assert!(context.contains("based on searched providers"));
    }

    #[test]
    fn configuration_is_not_misreported_as_a_live_mcp_connection() {
        // Treating `enabled` as `connected` would make every inspector refresh
        // claim live availability without observing or probing a connection.
        let providers = crate::commands::configured_literature_providers();

        for provider in providers
            .iter()
            .filter(|provider| provider.provider_id != "pubmed")
        {
            assert!(!provider.connected, "{}", provider.provider_id);
        }
    }

    #[test]
    fn durable_terminal_run_remains_authoritative_without_a_live_status_probe() {
        // Requiring a fresh MCP probe before reading the ledger would either
        // hide a completed zero-result search or reconnect during UI refresh.
        let providers = vec![crate::search_run::ProviderDescriptor::configured(
            "crossref", true, false,
        )];
        let run = crate::search_run::SearchRun::succeeded(
            "task-1",
            "crossref",
            "crossref_search_works",
            "stroke rehabilitation",
            serde_json::Value::Null,
            "100",
            "200",
            0,
            TEST_HASH,
        )
        .unwrap();

        let coverage =
            crate::commands::literature_coverage_from_runs(Some("task-1"), &providers, &[run]);

        assert_eq!(
            coverage.providers[0].state,
            crate::search_run::CoverageState::Searched
        );
        assert_eq!(coverage.providers[0].result_count, Some(0));
    }

    #[test]
    fn coverage_query_disclosure_is_bounded() {
        // A provider can receive a very large query payload; reflecting it in
        // the inspector DTO would defeat the compact, safe summary boundary.
        let providers = vec![crate::search_run::ProviderDescriptor::configured(
            "pubmed", true, true,
        )];
        let run = crate::search_run::SearchRun::succeeded(
            "task-1",
            "pubmed",
            "search_pubmed",
            "x".repeat(2_000),
            serde_json::Value::Null,
            "100",
            "200",
            1,
            TEST_HASH,
        )
        .unwrap();

        let coverage =
            crate::commands::literature_coverage_from_runs(Some("task-1"), &providers, &[run]);
        let query = coverage.providers[0].latest_query.as_deref().unwrap();

        assert!(query.chars().count() <= 241);
        assert!(query.ends_with('…'));
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
        assert_eq!(
            prompt,
            build_system_prompt(&persona, crate::modes::ChatMode::Discuss)
        );
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
