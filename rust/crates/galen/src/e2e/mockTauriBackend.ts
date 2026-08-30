import type {
  RehabCaseBundle,
  RehabCaseSummary,
  RehabGoldenEvalReport,
} from "../domain/rehabContext";
import type { ArtifactRecord } from "../domain/artifact";

type Callback = (payload: unknown) => void;

declare global {
  interface Window {
    __TAURI_INTERNALS__?: {
      invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
      transformCallback: (callback: Callback, once?: boolean) => number;
      unregisterCallback: (id: number) => void;
    };
  }
}

const initialBundle: RehabCaseBundle = {
  revision: 1,
  case_record: {
    case_id: "AIS-C025",
    demographics: { age: 14, sex: "female" },
    condition: { diagnosis: "adolescent idiopathic scoliosis" },
    updated_at: "2026-08-29T10:00:00+08:00",
  },
  events: [
    { event_id: "baseline", event_type: "baseline", occurred_at: "2025-09-01", collection_context: "natural_standing", interventions: [] },
    { event_id: "follow-up-12w", event_type: "follow_up", occurred_at: "2025-11-24", collection_context: "out_of_brace_timed", interventions: ["brace", "PSSE"] },
  ],
  observations: [
    { observation_id: "cobb-baseline", event_id: "baseline", metric: "Cobb angle", region: "thoracic", value: 31, unit: "°", collection_context: "natural_standing", verification_status: "verified", source_locator: { pdf_page: 12, book_page: null, channel: "radiograph", figure: null } },
    { observation_id: "cobb-follow-up", event_id: "follow-up-12w", metric: "Cobb angle", region: "thoracic", value: 26, unit: "°", collection_context: "out_of_brace_timed", verification_status: "disputed", source_locator: { pdf_page: 19, book_page: null, channel: "table", figure: null } },
  ],
  review_decisions: [
    {
      decision_id: "review-cobb-follow-up",
      target_observation_id: "cobb-follow-up",
      question: "12 周 Cobb 角应采用影像读数还是表格值？",
      status: "open",
      selected_option_id: null,
      options: [
        { option_id: "radiograph-25", label: "影像复核", value: 25, channel: "radiograph" },
        { option_id: "table-26", label: "原表记录", value: 26, channel: "table" },
      ],
    },
  ],
  cohort_row: {
    status: "pending_review",
    reasons: ["follow-up Cobb angle disputed"],
    derived_values: { cobb_change_deg: -5, follow_up_weeks: 12 },
    source_coverage: 1,
    open_review_count: 1,
  },
};

const goldenReport: RehabGoldenEvalReport = {
  suite_id: "rehab-golden-v1",
  generated_at: "2026-08-29T10:02:00+08:00",
  passed: true,
  negative_optimization_detected: false,
  journeys: [
    ["G01", "首次导入", "新用户", 482],
    ["G02", "来源追溯", "康复科研人员", 311],
    ["G03", "争议裁决", "治疗师", 526],
    ["G04", "状态恢复", "回访研究者", 438],
    ["G05", "成果预览", "比赛评委", 362],
  ].map(([journey_id, title, persona, duration_ms]) => ({
    journey_id: String(journey_id), title: String(title), persona: String(persona),
    duration_ms: Number(duration_ms), passed: true, checks: [],
  })),
  metrics: [
    { id: "task_success", label: "任务成功率", value: 1, threshold: 0.9, passed: true, unit: "ratio" },
    { id: "memory", label: "上下文保持", value: 1, threshold: 0.9, passed: true, unit: "ratio" },
    { id: "delivery", label: "交付可验证", value: 1, threshold: 1, passed: true, unit: "ratio" },
    { id: "ui_flow", label: "界面闭环", value: 1, threshold: 0.9, passed: true, unit: "ratio" },
  ],
  recommendations: ["未检测到负优化；下一轮提高病例歧义与跨轮干扰强度。"],
};

const deliveryArtifact: ArtifactRecord = {
  id: "artifact-e09-release",
  path: "output/E09-scoliosis-evidence-brief.md",
  kind: "document",
  mimeType: "text/markdown",
  size: 1038,
  contentHash: "e09-release-preview-fixture",
  taskId: "E09",
  nodeId: null,
  createdAt: "2026-08-29T10:05:00+08:00",
  source: "agent",
};

const deliveryMarkdown = `# 青少年特发性脊柱侧弯证据简报

> 交付状态：已完成，关键结论仍需临床人员复核。

## 核心结论

支具联合脊柱侧弯特异性运动可能改善部分患者的 Cobb 角进展风险，但疗效取决于依从性、骨成熟度与随访窗口。

## 可核验结果

| 指标 | 基线 | 12 周 | 解释 |
| --- | ---: | ---: | --- |
| 胸弯 Cobb 角 | 31° | 25° | 改善 6°，来源已复核 |
| 开放争议 | 1 | 0 | 已完成人工裁决 |

## 下一步行动

1. 核验支具每日佩戴时长。
2. 在 24 周节点重复站立位全脊柱影像。
3. 保留原始影像与量角记录，支持审计追溯。
`;

function summary(bundle: RehabCaseBundle): RehabCaseSummary {
  return {
    case_id: bundle.case_record.case_id,
    revision: bundle.revision,
    status: bundle.cohort_row.status,
    event_count: bundle.events.length,
    observation_count: bundle.observations.length,
    open_review_count: bundle.cohort_row.open_review_count,
  };
}

export function installE2eTauriBackend(): void {
  if (window.__TAURI_INTERNALS__ || new URLSearchParams(window.location.search).get("e2e") !== "1") return;

  let bundle: RehabCaseBundle | null = null;
  let callbackId = 0;
  const callbacks = new Map<number, Callback>();

  window.__TAURI_INTERNALS__ = {
    transformCallback(callback, once = false) {
      const id = ++callbackId;
      callbacks.set(id, once ? (payload) => { callback(payload); callbacks.delete(id); } : callback);
      return id;
    },
    unregisterCallback(id) { callbacks.delete(id); },
    async invoke(command, args = {}) {
      if (command.startsWith("plugin:event|")) return command.endsWith("listen") ? callbackId : null;
      switch (command) {
        case "get_workspace_root": return "D:\\DEV\\Galen-new";
        case "get_models": return [{ name: "DeepSeek V4 Flash", provider: "openai_compat", model_id: "deepseek-v4-flash" }];
        case "get_model_status": return [{ name: "DeepSeek V4 Flash", api_key_present: true, available: true, error: null }];
        case "get_runtime_status": return {
          python: { installed: true, version: "3.10", path: "python", install_guide: null },
          r: { installed: false, version: null, path: null, install_guide: null },
          typst: { installed: true, version: "0.13", path: "typst", install_guide: null },
          deno: { installed: false, version: null, path: null, install_guide: null },
          uvx: { installed: true, version: "0.8", path: "uvx", install_guide: null },
        };
        case "get_mcp_status": return [];
        case "get_capabilities": return [];
        case "get_modes": return [
          { id: "discuss", label: "讨论", description: "澄清与推理" },
          { id: "plan", label: "规划", description: "形成研究计划" },
          { id: "auto", label: "自动", description: "执行并交付" },
        ];
        case "get_mode": return "discuss";
        case "get_chat_session": return [];
        case "get_artifacts": return [deliveryArtifact];
        case "read_workspace_file": {
          if (String(args.path ?? "") !== deliveryArtifact.path) throw new Error("artifact not found");
          return deliveryMarkdown;
        }
        case "read_artifact_bytes": {
          if (String(args.path ?? "") !== deliveryArtifact.path) throw new Error("artifact not found");
          return new TextEncoder().encode(deliveryMarkdown).buffer;
        }
        case "get_memory_status": return { exists: true, size: 3, preview: "AIS cohort context" };
        case "get_conversation_decisions": return [];
        case "get_active_research_task": return null;
        case "get_agent_benchmark_report": return {
          case_id: "E01",
          runs: [
            { profile: "自动路由", model: "deepseek-v4-flash", samples: 5, pass_rate: 1, mean_ttfr_ms: 675, p95_ttfr_ms: 812, mean_total_ms: 1898, p95_total_ms: 2168, mean_input_tokens: 1069, mean_output_tokens: 93 },
            { profile: "Flash", model: "deepseek-v4-flash", samples: 5, pass_rate: 1, mean_ttfr_ms: 607, p95_ttfr_ms: 701, mean_total_ms: 2055, p95_total_ms: 2585, mean_input_tokens: 1069, mean_output_tokens: 108 },
            { profile: "Pro", model: "deepseek-v4-pro", samples: 5, pass_rate: 1, mean_ttfr_ms: 802, p95_ttfr_ms: 919, mean_total_ms: 3929, p95_total_ms: 6288, mean_input_tokens: 1069, mean_output_tokens: 107 },
          ],
        };
        case "list_rehab_cases": return bundle ? [summary(bundle)] : [];
        case "get_rehab_case": return bundle;
        case "import_rehab_case": bundle = structuredClone(initialBundle); return bundle;
        case "resolve_rehab_review": {
          if (!bundle) throw new Error("case not imported");
          const optionId = String(args.optionId ?? "");
          bundle = structuredClone(bundle);
          bundle.revision += 1;
          bundle.review_decisions[0].status = "resolved";
          bundle.review_decisions[0].selected_option_id = optionId;
          bundle.observations[1].verification_status = "verified";
          bundle.observations[1].value = optionId === "radiograph-25" ? 25 : 26;
          bundle.cohort_row.status = "included";
          bundle.cohort_row.open_review_count = 0;
          bundle.cohort_row.derived_values.cobb_change_deg = optionId === "radiograph-25" ? -6 : -5;
          return bundle;
        }
        case "run_rehab_golden_journeys": return goldenReport;
        case "set_mode":
        case "clear_chat_session":
        case "append_memory": return null;
        default: throw new Error(`E2E backend has no fixture for ${command}`);
      }
    },
  };
}
