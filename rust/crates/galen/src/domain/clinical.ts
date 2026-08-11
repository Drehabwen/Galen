import type { FileEntry } from "../types";
import type { ArtifactKind, ClassifiedEntry, WorkflowStage } from "./types";
import { getExtension, DATA_EXTENSIONS, DOC_EXTENSIONS, ANALYSIS_EXTENSIONS } from "./types";

// ---------------------------------------------------------------------------
// Clinical-specific artifact types
// ---------------------------------------------------------------------------

// Clinical-specific subtypes beyond the generic ArtifactKind
type ClinicalExtra = "dictionary" | "clinical" | "scale" | "lab" | "followup" | "protocol" | "qc" | "analysis" | "manuscript";
export type ClinicalArtifactKind = ArtifactKind | ClinicalExtra;

export function classifyClinicalEntry(entry: FileEntry): ClinicalArtifactKind {
  const name = entry.name.toLowerCase();

  if (entry.is_dir) {
    if (/(follow|visit|随访|复诊)/.test(name)) return "followup";
    if (/(scale|score|questionnaire|量表|评分|问卷)/.test(name)) return "scale";
    if (/(lab|bio|blood|serum|生化|血液|检验)/.test(name)) return "lab";
    if (/(case|patient|subject|crf|ecrf|form|病例|患者|受试者|采集)/.test(name)) return "clinical";
    if (/(data|raw|clean|dataset|数据)/.test(name)) return "data";
    if (/(codebook|dictionary|变量|字段)/.test(name)) return "dictionary";
    if (/(analysis|stat|script|notebook|统计|分析)/.test(name)) return "analysis";
    if (/(manuscript|paper|writing|draft|论文|写作|投稿)/.test(name)) return "manuscript";
    if (/(protocol|方案|plan|sap|纳排|criteria)/.test(name)) return "protocol";
    if (/(qc|query|cleaning|质控|清洗|核查)/.test(name)) return "qc";
    return "directory";
  }

  const ext = getExtension(name);
  if (/(follow|visit|随访|复诊)/.test(name)) return "followup";
  if (/(scale|score|questionnaire|量表|评分|问卷)/.test(name)) return "scale";
  if (/(lab|bio|blood|serum|生化|血液|检验)/.test(name)) return "lab";
  if (/(case|patient|subject|crf|ecrf|form|病例|患者|受试者|采集)/.test(name)) return "clinical";
  if (/(codebook|dictionary|变量|字段)/.test(name)) return "dictionary";
  if (/(protocol|方案|sap|纳排|criteria)/.test(name)) return "protocol";
  if (/(qc|query|cleaning|质控|清洗|核查)/.test(name)) return "qc";
  if (ANALYSIS_EXTENSIONS.has(ext)) return "analysis";
  if (DATA_EXTENSIONS.has(ext)) return "data";
  if (DOC_EXTENSIONS.has(ext)) return "manuscript";
  return "other";
}

export function clinicalArtifactLabel(kind: ClinicalArtifactKind): string {
  const labels: Record<ClinicalArtifactKind, string> = {
    source: "源码",
    config: "配置",
    dependency: "依赖",
    test: "测试",
    doc: "文档",
    data: "数据",
    script: "脚本",
    output: "产物",
    directory: "目录",
    other: "其他",
    dictionary: "字典",
    clinical: "病例",
    scale: "量表",
    lab: "指标",
    followup: "随访",
    protocol: "方案",
    qc: "质控",
    analysis: "统计",
    manuscript: "写作",
  };
  return labels[kind];
}

// ---------------------------------------------------------------------------
// Clinical workflow stages
// ---------------------------------------------------------------------------

export function getClinicalStages(
  entries: FileEntry[],
): WorkflowStage[] {
  const classified = entries.map((entry) => ({ entry, kind: classifyClinicalEntry(entry) }));
  const fileArtifacts = classified.filter(({ entry }) => !entry.is_dir);

  const protocolArtifacts = classified.filter(({ kind }) => kind === "protocol");
  const dictionaryArtifacts = classified.filter(({ kind }) => kind === "dictionary");
  const clinicalArtifacts = classified.filter(({ kind }) => kind === "clinical");
  const dataArtifacts = fileArtifacts.filter(({ kind }) => kind === "data");
  const scaleArtifacts = classified.filter(({ kind }) => kind === "scale");
  const labArtifacts = classified.filter(({ kind }) => kind === "lab");
  const followupArtifacts = classified.filter(({ kind }) => kind === "followup");
  const qcArtifacts = classified.filter(({ kind }) => kind === "qc");
  const analysisArtifacts = fileArtifacts.filter(({ kind }) => kind === "analysis");
  const manuscriptArtifacts = fileArtifacts.filter(({ kind }) => kind === "manuscript");

  const hasDesign = protocolArtifacts.length > 0 || dictionaryArtifacts.length > 0 || clinicalArtifacts.length > 0;
  const hasCapture = dataArtifacts.length > 0 || clinicalArtifacts.length > 0 || scaleArtifacts.length > 0 || labArtifacts.length > 0;
  const hasFollowup = followupArtifacts.length > 0 || qcArtifacts.length > 0;
  const hasStats = analysisArtifacts.length > 0 || (dataArtifacts.length > 0 && dictionaryArtifacts.length > 0);
  const hasWriting = manuscriptArtifacts.length > 0;

  return [
    {
      title: "课题设计",
      state: hasDesign ? "ready" : "incomplete",
      detail: hasDesign ? "已有方案、CRF 或变量定义基础" : "先把临床问题、PICO、纳排标准和结局指标定下来",
      prompt: "请把当前课题包转成临床课题方案骨架：研究问题、PICO/PECO、纳排标准、主要/次要结局、时间点和变量表。",
    },
    {
      title: "病例采集",
      state: hasCapture ? "ready" : "incomplete",
      detail: hasCapture ? "已发现病例、CRF、量表或指标文件" : "需要病例登记表、就诊时间点和采集字段",
      prompt: "请基于当前课题包生成临床采集表结构，包括患者编号、时间点、量表、检查指标、治疗/康复方案和备注字段。",
    },
    {
      title: "随访质控",
      state: hasFollowup ? "ready" : "incomplete",
      detail: hasFollowup ? "已有随访或质控相关材料" : "需要随访窗口、缺失项规则和 query 处理方式",
      prompt: "请为当前临床课题生成随访质控清单：每个时间点应收集什么、哪些缺失必须追访、哪些异常需要 query。",
    },
    {
      title: "统计分析",
      state: hasStats ? "ready" : "incomplete",
      detail: hasStats ? "已有数据/字典或分析脚本基础" : "数据源和变量定义齐备后再进入统计",
      prompt: "请根据当前课题包状态生成统计分析路径：Table 1、组间比较、随访趋势、回归/敏感性分析和图表清单。",
    },
    {
      title: "写作发表",
      state: hasWriting ? "ready" : "incomplete",
      detail: hasWriting ? "已有手稿或写作材料" : "方法学、结果段、图表说明和投稿清单仍需生成",
      prompt: "请基于当前课题包生成论文写作骨架：标题、摘要结构、方法学段落、结果表图、讨论要点和投稿前检查项。",
    },
  ];
}

// ---------------------------------------------------------------------------
// Clinical agent tasks
// ---------------------------------------------------------------------------

export function getClinicalAgentTasks(
  entries: FileEntry[],
): { label: string; prompt: string }[] {
  const classified = entries.map((entry) => ({ entry, kind: classifyClinicalEntry(entry) }));
  const fileArtifacts = classified.filter(({ entry }) => !entry.is_dir);

  const dataArtifacts = fileArtifacts.filter(({ kind }) => kind === "data");
  const dictionaryArtifacts = classified.filter(({ kind }) => kind === "dictionary");
  const clinicalArtifacts = classified.filter(({ kind }) => kind === "clinical");
  const scaleArtifacts = classified.filter(({ kind }) => kind === "scale");
  const labArtifacts = classified.filter(({ kind }) => kind === "lab");
  const followupArtifacts = classified.filter(({ kind }) => kind === "followup");
  const analysisArtifacts = fileArtifacts.filter(({ kind }) => kind === "analysis");
  const manuscriptArtifacts = fileArtifacts.filter(({ kind }) => kind === "manuscript");

  const dataName = dataArtifacts[0]?.entry.path || dataArtifacts[0]?.entry.name;
  const dictionaryName = dictionaryArtifacts[0]?.entry.path || dictionaryArtifacts[0]?.entry.name;
  const clinicalName = clinicalArtifacts[0]?.entry.path || clinicalArtifacts[0]?.entry.name;
  const scaleName = scaleArtifacts[0]?.entry.path || scaleArtifacts[0]?.entry.name;
  const labName = labArtifacts[0]?.entry.path || labArtifacts[0]?.entry.name;
  const followupName = followupArtifacts[0]?.entry.path || followupArtifacts[0]?.entry.name;
  const analysisName = analysisArtifacts[0]?.entry.path || analysisArtifacts[0]?.entry.name;
  const manuscriptName = manuscriptArtifacts[0]?.entry.path || manuscriptArtifacts[0]?.entry.name;

  return [
    {
      label: clinicalName || dictionaryName ? "完善课题方案" : "生成课题方案",
      prompt: clinicalName || dictionaryName
        ? `请优先读取 ${clinicalName || dictionaryName}，把当前材料整理成临床课题方案：研究问题、病例来源、纳排标准、结局指标和随访时间点。`
        : "请基于当前课题包生成一个临床课题方案模板，要求医生能直接补充病种、病例来源、纳排标准和结局指标。",
    },
    {
      label: dataName ? "检查病例采集" : "建立采集表",
      prompt: dataName
        ? `请优先读取 ${dataName}，检查病例采集字段是否支持临床课题设计，并指出缺少的患者信息、时间点、量表或检查指标。`
        : "请生成临床病例采集表结构，覆盖患者编号、入组信息、就诊时间点、干预/治疗、量表、检查指标和随访备注。",
    },
    {
      label: scaleName || labName ? "核对量表指标" : "规划量表指标",
      prompt: scaleName || labName
        ? `请优先读取 ${scaleName || labName}，检查量表/指标是否和主要结局、次要结局、随访时间点一致。`
        : "请根据当前课题包状态，规划临床量表、体征、影像/检查和可选生化指标的采集原则。",
    },
    {
      label: followupName ? "检查随访质控" : "生成随访质控",
      prompt: followupName
        ? `请优先读取 ${followupName}，整理随访窗口、缺失项、异常值和 query 处理清单。`
        : "请生成随访质控规则：时间点、允许窗口、必填项、缺失追访、异常值复核和 query 关闭标准。",
    },
    {
      label: analysisName ? "审查统计路径" : "生成统计路径",
      prompt: analysisName
        ? `请优先读取 ${analysisName}，审查统计路径是否匹配临床问题、结局指标、分组和随访设计。`
        : "请生成统计分析路径：Table 1、组间比较、随访趋势、回归模型、图表清单和结果解释边界。",
    },
    {
      label: manuscriptName ? "整理发表材料" : "生成论文骨架",
      prompt: manuscriptName
        ? `请优先读取 ${manuscriptName}，提取当前论文材料还缺哪些临床信息、统计结果和图表。`
        : "请生成论文写作骨架：标题、摘要、方法、结果、讨论、图表说明、参考文献策略和投稿前检查。",
    },
  ];
}

// ---------------------------------------------------------------------------
// Clinical metrics
// ---------------------------------------------------------------------------

export interface ClinicalMetric {
  label: string;
  value: string;
  sub: string;
}

export function getClinicalMetrics(entries: FileEntry[]): ClinicalMetric[] {
  const classified = entries.map((entry) => ({ entry, kind: classifyClinicalEntry(entry) }));
  const fileArtifacts = classified.filter(({ entry }) => !entry.is_dir);
  const totalSize = fileArtifacts.reduce((sum, c) => sum + c.entry.size, 0);

  const protocolArtifacts = classified.filter(({ kind }) => kind === "protocol");
  const dictionaryArtifacts = classified.filter(({ kind }) => kind === "dictionary");
  const clinicalArtifacts = classified.filter(({ kind }) => kind === "clinical");

  const dataArtifacts = fileArtifacts.filter(({ kind }) => kind === "data");
  const scaleArtifacts = classified.filter(({ kind }) => kind === "scale");
  const labArtifacts = classified.filter(({ kind }) => kind === "lab");
  const followupArtifacts = classified.filter(({ kind }) => kind === "followup");
  const qcArtifacts = classified.filter(({ kind }) => kind === "qc");
  const analysisArtifacts = fileArtifacts.filter(({ kind }) => kind === "analysis");
  const manuscriptArtifacts = fileArtifacts.filter(({ kind }) => kind === "manuscript");

  const hasDesign = protocolArtifacts.length > 0 || dictionaryArtifacts.length > 0 || clinicalArtifacts.length > 0;
  const hasFollowup = followupArtifacts.length > 0 || qcArtifacts.length > 0;

  return [
    {
      label: "课题设计",
      value: hasDesign ? "有" : "缺",
      sub: hasDesign
        ? summarizeNames([...protocolArtifacts, ...dictionaryArtifacts, ...clinicalArtifacts])
        : "待生成方案/CRF/变量表",
    },
    {
      label: "病例资料",
      value: String(dataArtifacts.length + clinicalArtifacts.length + scaleArtifacts.length + labArtifacts.length),
      sub: totalSize > 0 ? `数据文件总计 ${formatSize(totalSize)}` : "尚未识别病例数据",
    },
    {
      label: "随访质控",
      value: String(followupArtifacts.length + qcArtifacts.length),
      sub: hasFollowup ? summarizeNames([...followupArtifacts, ...qcArtifacts]) : "待配置随访窗口和 query",
    },
    {
      label: "统计写作",
      value: String(analysisArtifacts.length + manuscriptArtifacts.length),
      sub: "等待分析脚本或手稿材料",
    },
  ];
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type ClinicalClassified = { entry: FileEntry; kind: ClinicalArtifactKind };

function summarizeNames(items: ClinicalClassified[], max = 3): string {
  return items.length > 0
    ? items.slice(0, max).map(({ entry }) => entry.name).join("、")
    : "未发现";
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
