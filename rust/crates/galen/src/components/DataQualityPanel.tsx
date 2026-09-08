import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { readSheet } from "read-excel-file/browser";
import type { ArtifactRecord } from "../domain/artifact";
import type { AnalysisResult, InsightDirection, InsightMetric, RehabTimelineImportPayload } from "../domain/analysisResult";

type Matrix = string[][];
type FieldProfile = { raw: string; canonical: string | null; missing: number; numeric: number; min?: number; max?: number; flags: number };
type Profile = { headers: string[]; rows: Matrix; fields: FieldProfile[]; duplicates: number; anomalies: string[] };
type CleaningOutput = { dataset: ArtifactRecord; qualityReport: ArtifactRecord };
type DataQualityPanelProps = {
  workspaceSelected: boolean;
  onOpenArtifact: (artifact: ArtifactRecord) => void;
  onArtifactsCreated: (artifacts: ArtifactRecord[]) => void;
  onInsightReady: (result: AnalysisResult) => void;
};

const FIELD_ALIASES: Record<string, string[]> = {
  rehab_id: ["rehab id", "rehab_id", "病例id", "病例编号"],
  subject_id: ["subject", "subject id", "subject_id", "participant", "受试者", "受试者编号", "受试者id", "编号"],
  timepoint: ["timepoint", "time point", "visit", "时间点", "测试时间", "测量时间", "阶段"],
  heart_rate_bpm: ["heart rate", "heart_rate", "hr", "心率"],
  hrv_rmssd_ms: ["rmssd", "hrv", "心率变异", "心率变异性"],
  rpe: ["rpe", "borg", "主观疲劳", "自觉疲劳"],
  pain_vas: ["vas", "疼痛", "疼痛评分"],
  lactate_mmol_l: ["lactate", "blood lactate", "血乳酸", "乳酸"],
  cmj_height_cm: ["cmj", "jump height", "垂直跳", "反向跳"],
};

const RANGES: Record<string, [number, number]> = {
  heart_rate_bpm: [25, 240], hrv_rmssd_ms: [0, 300], rpe: [0, 20], pain_vas: [0, 10], lactate_mmol_l: [0, 30], cmj_height_cm: [0, 150],
};

function normal(value: string): string { return value.trim().toLowerCase().replace(/[＿_]/g, "_").replace(/\s+/g, " "); }
function canonicalOf(header: string): string | null {
  const input = normal(header);
  return Object.entries(FIELD_ALIASES).find(([, aliases]) => aliases.some((alias) => input === normal(alias) || input.includes(normal(alias))))?.[0] ?? null;
}

function parseDelimited(content: string): Matrix {
  const delimiter = content.split(/\r?\n/, 1)[0]?.includes("\t") ? "\t" : ",";
  const rows: Matrix = [];
  let row: string[] = [], value = "", quoted = false;
  for (let index = 0; index < content.length; index += 1) {
    const char = content[index];
    if (char === '"') {
      if (quoted && content[index + 1] === '"') { value += '"'; index += 1; } else quoted = !quoted;
    } else if (char === delimiter && !quoted) { row.push(value); value = ""; }
    else if ((char === "\n" || char === "\r") && !quoted) {
      if (char === "\r" && content[index + 1] === "\n") index += 1;
      row.push(value); if (row.some((cell) => cell.trim())) rows.push(row); row = []; value = "";
    } else value += char;
  }
  row.push(value); if (row.some((cell) => cell.trim())) rows.push(row);
  return rows;
}

function profileMatrix(matrix: Matrix): Profile {
  const headers = (matrix[0] ?? []).map((header, index) => header.trim() || `column_${index + 1}`);
  const rows = matrix.slice(1).map((row) => headers.map((_, index) => String(row[index] ?? "")));
  const duplicates = rows.length - new Set(rows.map((row) => JSON.stringify(row))).size;
  const anomalies: string[] = [];
  const fields = headers.map((raw, index) => {
    const canonical = canonicalOf(raw);
    const values = rows.map((row) => row[index].trim());
    const present = values.filter((value) => !isMissing(value));
    const numbers = present.map(asNumber).filter((value): value is number => value !== null);
    const [min, max] = numbers.length ? [Math.min(...numbers), Math.max(...numbers)] : [undefined, undefined];
    const range = canonical ? RANGES[canonical] : undefined;
    const flags = range ? numbers.filter((value) => value < range[0] || value > range[1]).length : 0;
    if (flags) anomalies.push(`${raw} 有 ${flags} 条超出 ${range?.[0]}–${range?.[1]} 的记录`);
    return { raw, canonical, missing: values.length - present.length, numeric: numbers.length, min, max, flags };
  });
  if (duplicates) anomalies.push(`发现 ${duplicates} 条完全重复记录`);
  return { headers, rows, fields, duplicates, anomalies };
}

function isMissing(value: string): boolean { return ["", "na", "n/a", "null", "none", "-", "无"].includes(normal(value)); }
function asNumber(value: string): number | null { const parsed = Number(value.trim().replace(/，/g, ".")); return Number.isFinite(parsed) ? parsed : null; }
function csvEscape(value: string): string { return /[",\r\n]/.test(value) ? `"${value.replace(/"/g, '""')}"` : value; }

function cleanProfile(profile: Profile, removeDuplicates: boolean): { csv: string; report: Record<string, unknown>; preview: Matrix } {
  const used = new Map<string, number>();
  const headers = profile.fields.map((field) => {
    const base = field.canonical ?? field.raw.trim(); const current = used.get(base) ?? 0; used.set(base, current + 1); return current ? `${base}_${current + 1}` : base;
  });
  const seen = new Set<string>();
  const cleanedRows = profile.rows.filter((row) => {
    const normalized = row.map((value) => isMissing(value) ? "" : value.trim().replace(/，/g, "."));
    const key = JSON.stringify(normalized);
    if (removeDuplicates && seen.has(key)) return false;
    seen.add(key); return true;
  }).map((row) => row.map((value) => isMissing(value) ? "" : value.trim().replace(/，/g, ".")));
  return {
    csv: [headers, ...cleanedRows].map((row) => row.map(csvEscape).join(",")).join("\n") + "\n",
    preview: [headers, ...cleanedRows.slice(0, 7)],
    report: {
      schemaVersion: 1, generatedAt: new Date().toISOString(), sourceRows: profile.rows.length, outputRows: cleanedRows.length,
      operations: { normalizedMissingValues: true, trimmedWhitespace: true, normalizedDecimalComma: true, exactDuplicatesRemoved: removeDuplicates ? profile.duplicates : 0 },
      fields: profile.fields, anomalies: profile.anomalies,
    },
  };
}

const METRIC_LABELS: Record<string, { label: string; unit: string; favorable: "up" | "down" }> = {
  heart_rate_bpm: { label: "心率", unit: "bpm", favorable: "down" },
  hrv_rmssd_ms: { label: "HRV · RMSSD", unit: "ms", favorable: "up" },
  rpe: { label: "主观疲劳 · RPE", unit: "分", favorable: "down" },
  pain_vas: { label: "疼痛 · VAS", unit: "分", favorable: "down" },
  lactate_mmol_l: { label: "血乳酸", unit: "mmol/L", favorable: "down" },
  cmj_height_cm: { label: "CMJ 高度", unit: "cm", favorable: "up" },
};

function makeInsight(profile: Profile, output: CleaningOutput): AnalysisResult {
  const timepointIndex = profile.fields.findIndex((field) => field.canonical === "timepoint");
  const rehabIdIndex = profile.fields.findIndex((field) => field.canonical === "rehab_id");
  const subjectIdIndex = profile.fields.findIndex((field) => field.canonical === "subject_id");
  const metrics: InsightMetric[] = profile.fields.flatMap((field, index) => {
    const config = field.canonical ? METRIC_LABELS[field.canonical] : undefined;
    if (!config) return [];
    const points = profile.rows.flatMap((row, rowIndex) => {
      const value = asNumber(row[index]);
      return value === null ? [] : [{ value, label: row[timepointIndex]?.trim() || `记录 ${rowIndex + 1}` }];
    });
    if (points.length === 0) return [];
    const baseline = points[0].value;
    const current = points[points.length - 1]?.value ?? baseline;
    const deltaPercent = baseline === 0 ? null : ((current - baseline) / Math.abs(baseline)) * 100;
    const direction: InsightDirection = Math.abs(current - baseline) < Math.max(Math.abs(baseline) * 0.02, 0.01) ? "flat" : current > baseline ? "up" : "down";
    return [{ id: field.canonical!, ...config, baseline, current, deltaPercent, direction, values: points.map((point) => point.value), labels: points.map((point) => point.label) }];
  });
  const byId = new Map(metrics.map((metric) => [metric.id, metric]));
  const hrv = byId.get("hrv_rmssd_ms");
  const cmj = byId.get("cmj_height_cm");
  const rpe = byId.get("rpe");
  const pain = byId.get("pain_vas");
  const findings: string[] = [];
  if (hrv && cmj) {
    const sameFavorableDirection = hrv.direction === "up" && cmj.direction === "up";
    const sameUnfavorableDirection = hrv.direction === "down" && cmj.direction === "down";
    if (sameFavorableDirection) findings.push("HRV 与 CMJ 均较首个记录改善，恢复相关指标呈现一致的向好趋势。");
    else if (sameUnfavorableDirection) findings.push("HRV 与 CMJ 同步低于首个记录，生理恢复与运动表现的变化方向一致，建议结合训练负荷继续追踪。");
    else findings.push("HRV 与 CMJ 的变化方向并不一致；结果页保留两条原始轨迹，便于区分自主神经与运动表现的变化。");
  }
  [rpe, pain].filter((metric): metric is InsightMetric => Boolean(metric)).forEach((metric) => {
    findings.push(`${metric.label}${metric.direction === "flat" ? "保持稳定" : metric.favorable === metric.direction ? "较首个记录下降" : "较首个记录上升"}，可与生理和运动表现指标同步查看。`);
  });
  if (findings.length === 0 && metrics.length > 0) findings.push(`已从清洗数据中识别 ${metrics.length} 个可计算指标；每项指标均保留首个记录、最新记录和完整轨迹。`);
  if (findings.length === 0) findings.push("本次数据已完成字段识别与质量记录，但尚未匹配到可计算的康复指标。");
  const safeCaseId = (value: string, fallback: string) => {
    const normalized = value.trim().replace(/[^a-zA-Z0-9_-]+/g, "-").replace(/^-+|-+$/g, "");
    return normalized || fallback;
  };
  const timelineMeasurements = profile.rows.flatMap((row, rowIndex) => {
    const rawCaseId = rehabIdIndex >= 0 ? row[rehabIdIndex] : subjectIdIndex >= 0 ? row[subjectIdIndex] : "";
    const caseId = safeCaseId(rawCaseId, "rehab-001");
    const timepoint = row[timepointIndex]?.trim() || `记录 ${rowIndex + 1}`;
    return profile.fields.flatMap((field, fieldIndex) => {
      const config = field.canonical ? METRIC_LABELS[field.canonical] : undefined;
      const value = asNumber(row[fieldIndex]);
      return config && value !== null ? [{ caseId, timepoint, metric: field.canonical!, value, unit: config.unit }] : [];
    });
  });
  const timelineImport: RehabTimelineImportPayload = {
    datasetPath: output.dataset.path,
    qualityReportPath: output.qualityReport.path,
    suggestedCaseId: [...new Set(timelineMeasurements.map((measurement) => measurement.caseId))][0] ?? "rehab-001",
    measurements: timelineMeasurements,
  };
  return {
    title: "本次数据分析结果",
    generatedAt: new Date().toISOString(),
    recordCount: profile.rows.length,
    fieldCount: profile.headers.length,
    dataSource: output.dataset.path,
    qualityReport: output.qualityReport.path,
    metrics,
    findings,
    dataNotes: profile.anomalies,
    timelineImport,
  };
}

export function DataQualityPanel({ workspaceSelected, onOpenArtifact, onArtifactsCreated, onInsightReady }: DataQualityPanelProps) {
  const [path, setPath] = useState("");
  const [profile, setProfile] = useState<Profile | null>(null);
  const [loading, setLoading] = useState(false);
  const [removeDuplicates, setRemoveDuplicates] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<CleaningOutput | null>(null);

  const load = async () => {
    if (!path.trim()) return;
    setLoading(true); setError(null); setSaved(null);
    try {
      const isExcel = /\.(xlsx|xls)$/i.test(path);
      const matrix = isExcel
        ? (await readSheet(new Blob([await invoke<ArrayBuffer>("read_artifact_bytes", { path })]))).map((row) => row.map((cell) => cell instanceof Date ? cell.toISOString() : String(cell ?? "")))
        : parseDelimited(await invoke<string>("read_workspace_file", { path }));
      if (matrix.length < 2) throw new Error("数据至少需要一行表头和一行记录。");
      setProfile(profileMatrix(matrix));
    } catch (cause) { setProfile(null); setError(`无法读取数据：${String(cause)}`); }
    finally { setLoading(false); }
  };

  const cleaning = useMemo(() => profile ? cleanProfile(profile, removeDuplicates) : null, [profile, removeDuplicates]);
  const save = async () => {
    if (!cleaning || !profile) return;
    setLoading(true); setError(null);
    try {
      const result = await invoke<CleaningOutput>("write_cleaned_dataset", { input: { sourcePath: path, csv: cleaning.csv, reportJson: JSON.stringify({ ...cleaning.report, sourcePath: path }) } });
      setSaved(result);
      onArtifactsCreated([result.dataset, result.qualityReport]);
      onInsightReady(makeInsight(profile, result));
    } catch (cause) { setError(`保存清洗版本失败：${String(cause)}`); }
    finally { setLoading(false); }
  };

  if (!workspaceSelected) return <section className="data-quality-panel data-quality-empty"><span className="plan-canvas-kicker">DATA QUALITY LAB</span><h2>选择工作区后开始数据体检</h2><p>原始文件留在工作区；Galen 只生成新的清洗版本和质量报告。</p></section>;
  return <section className="data-quality-panel" aria-label="数据质量实验室">
    <header><div><span className="plan-canvas-kicker">DATA QUALITY LAB</span><h2>数据体检与清洗</h2><p>支持 CSV、TSV、XLSX。先看问题，再生成新的可分析版本。</p></div></header>
    <div className="data-quality-load"><input value={path} onChange={(event) => setPath(event.target.value)} placeholder="工作区相对路径，例如 data/fatigue.xlsx" /><button className="btn btn-primary" type="button" disabled={loading || !path.trim()} onClick={() => void load()}>{loading ? "处理中…" : "开始体检"}</button></div>
    {error && <div className="evidence-trail__error">{error}</div>}
    {profile && cleaning && <>
      <div className="data-quality-stats"><span><strong>{profile.rows.length}</strong> 条记录</span><span><strong>{profile.headers.length}</strong> 个字段</span><span><strong>{profile.duplicates}</strong> 条完全重复</span><span><strong>{profile.anomalies.length}</strong> 项需关注</span></div>
      <section className="data-quality-section"><h3>字段映射与质量画像</h3><div className="data-quality-fields">{profile.fields.map((field) => <article key={field.raw}><strong>{field.raw}</strong><span>{field.canonical ? `→ ${field.canonical}` : "未映射"}</span><small>缺失 {field.missing} · 数值 {field.numeric}{field.min !== undefined ? ` · ${field.min}–${field.max}` : ""}{field.flags ? ` · 异常 ${field.flags}` : ""}</small></article>)}</div></section>
      <section className="data-quality-section"><h3>清洗方案</h3><p>统一空值标记、清除字段首尾空格、规范小数点；异常值不会删除，只保留在质量报告中。</p><label className="data-quality-toggle"><input type="checkbox" checked={removeDuplicates} onChange={(event) => setRemoveDuplicates(event.target.checked)} />移除完全重复的行（{profile.duplicates} 条）</label><button className="btn btn-primary" type="button" onClick={() => void save()} disabled={loading}>生成清洗版本与质量报告</button></section>
      {profile.anomalies.length > 0 && <section className="data-quality-section data-quality-alert"><h3>待确认事项</h3><ul>{profile.anomalies.map((item) => <li key={item}>{item}</li>)}</ul></section>}
      <section className="data-quality-section"><h3>清洗预览</h3><div className="artifact-table-wrap"><table className="artifact-table"><tbody>{cleaning.preview.map((row, rowIndex) => <tr key={rowIndex}>{row.map((cell, cellIndex) => <td key={cellIndex}>{cell}</td>)}</tr>)}</tbody></table></div></section>
      {saved && <div className="data-quality-saved">已生成 <button type="button" onClick={() => onOpenArtifact(saved.dataset)}>{saved.dataset.path}</button> 与 <button type="button" onClick={() => onOpenArtifact(saved.qualityReport)}>{saved.qualityReport.path}</button></div>}
    </>}
  </section>;
}
