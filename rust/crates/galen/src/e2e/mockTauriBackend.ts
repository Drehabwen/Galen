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

const cohortDeliveryArtifact: ArtifactRecord = {
  id: "artifact-sports-fatigue-paper-pdf",
  path: "output/pdf/Galen_高强度间歇负荷后多模态恢复_正式论文.pdf",
  kind: "document",
  mimeType: "application/pdf",
  size: 460983,
  contentHash: "galen-sports-fatigue-paper-v2",
  taskId: "task-rehab-cohort-fatigue",
  nodeId: null,
  createdAt: "2026-09-10T18:30:00+08:00",
  source: "agent",
};

const cohortDeliveryMarkdown = `# 多病例运动疲劳恢复的多模态纵向评估

## 摘要

本研究以 3 个脱敏 RehabID 的连续评估记录为例，观察一次负荷峰值后自主神经、神经肌肉表现与主观疲劳的变化，并验证 Galen 对多源康复数据的统一分析流程。每个对象包含基线、负荷峰值和 48 小时恢复 3 个时间点，共 9 个时间点、36 条核心数值观察。

结果显示，负荷峰值后 RMSSD 与 CMJ 同步下降，RPE 与 VAS 上升；48 小时后各对象均出现不同程度恢复，但恢复速度存在个体差异。单一指标无法完整描述恢复状态，RehabID 时间轴可以把指标变化与时间点、对象和数据来源保持在同一分析单元中。

## 1 研究问题

在运动康复场景中，将 HRV、CMJ、RPE 与 VAS 纳入同一 RehabID 纵向时间轴，是否能够更清晰地描述疲劳—恢复轨迹？

## 2 数据与方法

- **对象**：ATH-001、ATH-002、ATH-003（合成演示队列，数据已脱敏）。
- **时间点**：baseline、load_peak、recovery_48h。
- **指标**：HRV RMSSD（ms）、CMJ（cm）、RPE（0–10）、VAS（0–10）。
- **流程**：康复师工作台产生结构化记录 → Connector 预览并确认 → Galen 写入 RehabID → 按对象和时间点完成质控、纵向比较与论文生成。
- **分析**：对每个对象计算负荷峰值相对基线的变化，以及 48 小时恢复比例；结果保留原始时间点和指标单位，便于复核。

## 3 结果

| RehabID | RMSSD 基线→峰值→48 h | CMJ 基线→峰值→48 h | RPE 基线→峰值→48 h | VAS 基线→峰值→48 h |
| --- | --- | --- | --- | --- |
| ATH-001 | 62→38→55 ms | 38→29→35 cm | 3→8→5 | 1→5→2 |
| ATH-002 | 71→44→66 ms | 42→31→40 cm | 2→7→4 | 0→4→1 |
| ATH-003 | 55→33→48 ms | 35→24→31 cm | 4→9→6 | 2→6→3 |

三例在负荷峰值均出现 RMSSD、CMJ 下降以及 RPE、VAS 上升；48 小时后 RMSSD 恢复至基线的 77%–93%，CMJ 恢复至基线的 83%–95%。ATH-003 的主观疲劳恢复较慢，提示需要结合个体基线而非依赖统一阈值。

## 4 讨论

这组演示数据支持一个可检验的产品假设：康复科研的关键不只是增加指标，而是把不同来源的观测组织成可追踪的对象—时间点—指标关系。Galen 的价值在于完成数据接入、清洗、纵向比较和可复核产物交付，让研究者可以从一次性读数转向连续状态分析。

## 5 可复现性与数据出处

- 数据源：康复师工作台 Connector（live_bridge）。
- 导入回执：\`output/connector-imports/rehab-cohort-demo/import-receipt.json\`。
- 研究对象：\`ATH-001\`、\`ATH-002\`、\`ATH-003\`。
- 观察总数：36；时间点总数：9；对象总数：3。
- 本文为软件演示队列分析，不替代临床判断；后续实证研究应使用经伦理审批的真实队列并预注册统计方案。
`;

const cohortCases = Array.from({ length: 12 }, (_, index) => {
  const code = `ATH-${String(index + 1).padStart(3, "0")}`;
  return {
    caseId: code,
    displayName: `${code} · 演示队列`,
    sessionCount: 4,
    assessmentCount: 16,
    timepointCount: 4,
    measurementCount: 17,
  };
});

function isCohortDemo(): boolean {
  return new URLSearchParams(window.location.search).get("e2e") === "cohort";
}

function emitEvent(
  callbacks: Map<number, Callback>,
  listeners: Map<string, Set<number>>,
  event: string,
  payload: unknown,
): void {
  for (const id of listeners.get(event) ?? []) {
    callbacks.get(id)?.({ event, id, payload });
  }
}

function cohortTaskSnapshot(nodes: Array<Record<string, unknown>>): Record<string, unknown> {
  const completedNodes = nodes.map((node, index) => ({
    ...node,
    status: "completed",
    result: [
      "已完成：",
      index === 0 ? "12 个 RehabID、48 个评估节点和 204 条数值观察通过结构化质控。" :
        index === 1 ? "T0 时 RMSSD、CMJ 下降、静息心率上升；T24 DOMS 达峰，T72 保留个体恢复差异。" :
          "已形成摘要、方法、结果、讨论、图表与可预览 PDF 正式论文。",
    ].join(""),
    evidence: index === 0
      ? ["12 个脱敏 RehabID", "48 个纵向评估节点", "204 条带单位数值观察"]
      : index === 1
        ? ["RMSSD 与 CMJ 在 T0 急性下移", "DOMS 在 T24 达到峰值", "T72 恢复比例按对象保留"]
        : ["研究方法与数据字典已生成", "纵向图表已嵌入论文", "PDF 已写入产物库"],
    outputs: index === 2
      ? [cohortDeliveryArtifact.path]
      : index === 0
        ? ["output/analysis/cohort-data-quality.json"]
        : ["output/analysis/fatigue-longitudinal-summary.csv"],
  }));
  return {
    schemaVersion: 1,
    revision: 2,
    taskId: "task-rehab-cohort-fatigue",
    title: "高强度间歇负荷后 72 小时多模态恢复研究",
    goal: "比较 12 个 RehabID 在基线、T0、T24 与 T72 的多模态变化，并交付正式论文 PDF",
    status: "verifying",
    createdAt: "2026-09-10T18:20:00+08:00",
    updatedAt: "2026-09-10T18:30:00+08:00",
    nodes: completedNodes,
    evidenceIds: ["ev-cohort-quality", "ev-fatigue-trend", "ev-paper-draft"],
    artifactIds: [cohortDeliveryArtifact.id],
  };
}

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
  const e2eMode = new URLSearchParams(window.location.search).get("e2e");
  if (window.__TAURI_INTERNALS__ || (e2eMode !== "1" && e2eMode !== "cohort")) return;
  const cohortDemo = e2eMode === "cohort";

  let bundle: RehabCaseBundle | null = null;
  let callbackId = 0;
  const callbacks = new Map<number, Callback>();
  const listeners = new Map<string, Set<number>>();
  let activeTask: Record<string, unknown> | null = null;
  let paperReady = false;

  const emit = (event: string, payload: unknown) => {
    emitEvent(callbacks, listeners, event, payload);
  };

  const emitChat = (content: string, tag?: string) => {
    const suffix = tag ? `:${tag}` : "";
    window.setTimeout(() => {
      emit(`chat-done${suffix}`, content);
    }, 80);
  };

  const deliverCohortPaper = (tag?: string) => {
    if (!cohortDemo || paperReady) return;
    paperReady = true;
    if (activeTask) {
      activeTask = { ...activeTask, status: "deliverable", updatedAt: new Date().toISOString() };
      window.setTimeout(() => emit("research-task-updated", activeTask), 20);
    }
    window.setTimeout(() => emit("artifact-created", cohortDeliveryArtifact), 30);
    emitChat(
      "研究已完成。12 个 RehabID、48 个评估节点和 204 条数值观察已完成质控与纵向分析；正式论文 PDF 已写入产物库。\n\n" +
        `[预览 ${cohortDeliveryArtifact.path.split("/").pop()}](galen-artifact://${encodeURIComponent(cohortDeliveryArtifact.id)})`,
      tag,
    );
  };

  window.__TAURI_INTERNALS__ = {
    transformCallback(callback, once = false) {
      const id = ++callbackId;
      callbacks.set(id, once ? (payload) => { callback(payload); callbacks.delete(id); } : callback);
      return id;
    },
    unregisterCallback(id) { callbacks.delete(id); },
    async invoke(command, args = {}) {
      if (command === "plugin:event|listen") {
        const event = String(args.event ?? "");
        const handler = Number(args.handler ?? 0);
        if (!listeners.has(event)) listeners.set(event, new Set());
        listeners.get(event)?.add(handler);
        return handler;
      }
      if (command === "plugin:event|unlisten") {
        const event = String(args.event ?? "");
        const handler = Number(args.eventId ?? 0);
        listeners.get(event)?.delete(handler);
        return null;
      }
      if (command.startsWith("plugin:event|")) return null;
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
          { id: "auto", label: "自动", description: "自主执行并交付结果" },
          { id: "plan", label: "规划", description: "形成研究计划" },
        ];
        case "get_mode": return "auto";
        case "get_chat_session": return [];
        case "get_artifacts": return cohortDemo ? (paperReady ? [cohortDeliveryArtifact] : []) : [deliveryArtifact];
        case "read_workspace_file": {
          const path = String(args.path ?? "");
          if (cohortDemo && path === cohortDeliveryArtifact.path) return "正式论文以 PDF 产物形式交付，请通过 read_artifact_bytes 预览。";
          if (path !== deliveryArtifact.path) throw new Error("artifact not found");
          return deliveryMarkdown;
        }
        case "read_artifact_bytes": {
          const path = String(args.path ?? "");
          if (cohortDemo && path === cohortDeliveryArtifact.path) {
            const response = await fetch("/demo/galen-sports-fatigue-paper.pdf");
            if (!response.ok) throw new Error("demo paper PDF not found");
            return response.arrayBuffer();
          }
          if (path !== deliveryArtifact.path) throw new Error("artifact not found");
          return new TextEncoder().encode(deliveryMarkdown).buffer;
        }
        case "get_memory_status": return { exists: true, size: 3, preview: "AIS cohort context" };
        case "get_conversation_decisions": return [];
        case "get_active_research_task": return activeTask;
        case "discover_research_data_source": return cohortDemo ? {
          sourceId: "rehab-workbench",
          sourceLabel: "康复师工作台",
          exportPath: "C:\\Users\\labops\\AppData\\Local\\Rehab\\GalenConnector\\latest.json",
          exportedAt: 1789000000000,
          connectionMode: "live_bridge",
          patientCount: 12,
          sessionCount: 48,
          assessmentCount: 192,
          timepointCount: 48,
          measurementCount: 204,
          cases: cohortCases,
          canImport: true,
          message: "已发现 12 个 RehabID 候选、48 个评估时间点和 204 条数值观察。",
        } : {
          sourceId: "rehab-workbench",
          sourceLabel: "康复师工作台",
          exportPath: "C:\\Users\\labops\\Downloads\\rehab-backup-2026-09-10.json",
          exportedAt: 1789000000000,
          connectionMode: "live_bridge",
          patientCount: 1,
          sessionCount: 3,
          assessmentCount: 12,
          timepointCount: 3,
          measurementCount: 12,
          cases: [{ caseId: "ATH-001", displayName: "ATH-001", sessionCount: 3, assessmentCount: 12, timepointCount: 3, measurementCount: 12 }],
          canImport: true,
          message: "已发现 1 个 RehabID 候选、3 个评估时间点和 12 条数值观察。",
        };
        case "import_research_data_source": return cohortDemo ? {
          caseIds: cohortCases.map((item) => item.caseId),
          importedEventCount: 48,
          importedObservationCount: 204,
          skippedObservationCount: 0,
          receipt: {
            id: "artifact-connector-cohort-receipt",
            path: "output/connector-imports/rehab-cohort-demo/import-receipt.json",
            kind: "file",
            mimeType: "application/json",
            size: 4286,
            contentHash: "connector-cohort-v2-receipt",
            taskId: null,
            nodeId: null,
            createdAt: "2026-09-10T18:10:00+08:00",
            source: "agent",
          },
        } : {
          caseIds: ["ATH-001"],
          importedEventCount: 3,
          importedObservationCount: 12,
          skippedObservationCount: 0,
          receipt: {
            id: "artifact-connector-receipt",
            path: "output/connector-imports/rehab-workbench-demo/import-receipt.json",
            kind: "file",
            mimeType: "application/json",
            size: 842,
            contentHash: "connector-demo-receipt",
            taskId: null,
            nodeId: null,
            createdAt: "2026-09-10T12:00:00+08:00",
            source: "agent",
          },
        };
        case "create_research_task": {
          const nodes = Array.isArray(args.nodes) ? args.nodes as Array<Record<string, unknown>> : [];
          activeTask = cohortDemo ? cohortTaskSnapshot(nodes) : {
            schemaVersion: 1,
            revision: 1,
            taskId: "task-e2e",
            title: String(args.title ?? "研究任务"),
            goal: String(args.goal ?? "完成研究任务"),
            status: "running",
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
            nodes,
            evidenceIds: [],
            artifactIds: [],
          };
          return activeTask;
        }
        case "pi_start_node":
        case "pi_complete_node":
        case "pi_approve_node":
        case "pi_assign_node":
        case "append_evidence": {
          if (!activeTask) throw new Error("research task not found");
          return { task: activeTask, readyNodeIds: [], activeNodeIds: [], blockedNodeIds: [], awaitingDecisionNodeIds: [], lastEventSequence: 1 };
        }
        case "send_message": {
          const message = String(args.message ?? "");
          const tag = args.tag ? String(args.tag) : undefined;
          if (cohortDemo && /运动疲劳|RehabID/.test(message) && /论文|研究|比较|分析/.test(message) && !message.includes("计划已确认") && !message.includes("[计划完成]")) {
            emitChat(
                "已读取 12 个 RehabID 的连续恢复时间轴，并生成可执行研究计划。\n\n" +
                "<!-- PLAN_START -->\n" +
                "01. 队列数据质控 — 检查 12 个 RehabID、48 个评估节点和 204 条数值观察的完整性与单位一致性\n" +
                "02. 72 小时纵向比较 — 比较 HRV、CMJ、静息心率和 DOMS 在 T0、T24、T72 的变化\n" +
                "03. 结果整合与论文生成 — 汇总摘要、方法、结果、讨论、图表与可预览 PDF 正式论文\n" +
                "<!-- PLAN_END -->",
              tag,
            );
            return null;
          }
          if (cohortDemo && message.includes("计划已确认")) {
            emitChat("研究任务已建立：3 个节点将按证据链执行。12 个对象的质控与纵向比较已完成，正在汇总正式论文 PDF。", tag);
            // The fixture completes the evidence loop after the confirmation
            // response, so the recording demonstrates a real delivery handoff
            // without requiring a human to type three repetitive summaries.
            window.setTimeout(() => deliverCohortPaper(tag), 420);
            return null;
          }
          if (cohortDemo && (message.includes("[计划完成]") || message.includes("整合最终成果"))) {
            deliverCohortPaper(tag);
            return null;
          }
          emitChat(cohortDemo ? "已将本轮结果写入研究记录。" : "已完成本轮演示操作。", tag);
          return null;
        }
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
