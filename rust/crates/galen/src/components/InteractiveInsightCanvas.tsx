import { useEffect, useMemo, useState } from "react";
import type { AnalysisResult, InsightMetric } from "../domain/analysisResult";

const number = new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 1 });

function TrendSvg({ metric }: { metric: InsightMetric }) {
  const values = metric.values;
  const safeValues = values.length > 1 ? values : [metric.baseline, metric.current];
  const min = Math.min(...safeValues);
  const max = Math.max(...safeValues);
  const span = max - min || Math.max(Math.abs(max) * 0.16, 1);
  const points = safeValues.map((value, index) => {
    const x = 10 + (index * 280) / Math.max(safeValues.length - 1, 1);
    const y = 72 - ((value - min) / span) * 52;
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  }).join(" ");
  const last = points.split(" ")[points.split(" ").length - 1]?.split(",") ?? ["290", "45"];
  const better = metric.favorable === "up" ? metric.direction === "up" : metric.direction === "down";
  return (
    <svg className="insight-trend-svg" viewBox="0 0 300 88" role="img" aria-label={`${metric.label}趋势`}>
      <defs>
        <linearGradient id={`fill-${metric.id}`} x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stopColor={better ? "#15a38d" : "#e18445"} stopOpacity=".2" />
          <stop offset="100%" stopColor={better ? "#15a38d" : "#e18445"} stopOpacity="0" />
        </linearGradient>
      </defs>
      <path d="M10 72H290" className="insight-trend-baseline" />
      <path d={`M ${points.split(" ").join(" L ")} L 290 80 L 10 80 Z`} fill={`url(#fill-${metric.id})`} />
      <polyline points={points} className={`insight-trend-line ${better ? "good" : "watch"}`} />
      <circle cx={last[0]} cy={last[1]} r="4.5" className={`insight-trend-point ${better ? "good" : "watch"}`} />
    </svg>
  );
}

function MetricCard({ metric }: { metric: InsightMetric }) {
  const signedDelta = metric.deltaPercent === null ? "—" : `${metric.deltaPercent > 0 ? "+" : ""}${number.format(metric.deltaPercent)}%`;
  const directionText = metric.direction === "up" ? "上升" : metric.direction === "down" ? "下降" : "稳定";
  const better = metric.favorable === "up" ? metric.direction === "up" : metric.direction === "down";
  return (
    <article className="insight-metric-card">
      <div className="insight-metric-heading"><span>{metric.label}</span><em className={better ? "good" : metric.direction === "flat" ? "flat" : "watch"}>{directionText}</em></div>
      <div className="insight-metric-value"><strong>{number.format(metric.current)}</strong><small>{metric.unit}</small></div>
      <div className="insight-metric-delta"><span>相对首个记录</span><b className={better ? "good" : metric.direction === "flat" ? "flat" : "watch"}>{signedDelta}</b></div>
      <TrendSvg metric={metric} />
      <div className="insight-metric-range"><span>{metric.labels[0] ?? "首个记录"}</span><span>{metric.labels[metric.labels.length - 1] ?? "最新记录"}</span></div>
    </article>
  );
}

export function InteractiveInsightCanvas({ result, onOpenArtifact, onImportToTimeline }: {
  result: AnalysisResult | null;
  onOpenArtifact: (path: string) => void;
  onImportToTimeline: (result: AnalysisResult) => Promise<void>;
}) {
  const [importing, setImporting] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const [caseId, setCaseId] = useState("");
  useEffect(() => setCaseId(result?.timelineImport.suggestedCaseId ?? ""), [result?.timelineImport.suggestedCaseId]);
  const caseIds = useMemo(() => [...new Set(result?.timelineImport.measurements.map((item) => item.caseId) ?? [])], [result]);
  if (!result) {
    return <section className="insight-empty">
      <div className="insight-empty-mark"><i /><i /><i /></div>
      <span className="plan-canvas-kicker">INTERACTIVE REHAB INSIGHT</span>
      <h2>结果将在这里呈现</h2>
      <p>导入一份数据并完成体检后，Galen 会直接生成可交互的趋势、发现与依据；报告文件只作为导出版本。</p>
    </section>;
  }

  const primaryMetric = result.metrics[0];
  const recovered = result.metrics.filter((metric) => metric.direction !== "flat" && metric.favorable === metric.direction).length;
  const status = result.metrics.length === 0 ? "待分析" : recovered >= Math.ceil(result.metrics.length / 2) ? "趋势向好" : "需要关注";
  const canImport = result.timelineImport.measurements.length > 0;
  const importTimeline = async () => {
    if (!canImport) return;
    setImporting(true); setImportError(null);
    try {
      const normalizedCaseId = caseId.trim();
      const timelineImport = caseIds.length === 1 && normalizedCaseId
        ? { ...result.timelineImport, suggestedCaseId: normalizedCaseId, measurements: result.timelineImport.measurements.map((item) => ({ ...item, caseId: normalizedCaseId })) }
        : result.timelineImport;
      await onImportToTimeline({ ...result, timelineImport });
    } catch (cause) {
      setImportError(String(cause));
    } finally { setImporting(false); }
  };
  return <main className="insight-canvas" aria-label="交互分析结果">
    <header className="insight-header">
      <div>
        <span className="plan-canvas-kicker">INTERACTIVE REHAB INSIGHT · 已生成</span>
        <h1>{result.title}</h1>
        <p>{result.recordCount} 条记录 · {result.fieldCount} 个字段 · {new Date(result.generatedAt).toLocaleString("zh-CN", { hour12: false })}</p>
      </div>
      <div className="insight-header-actions">
        <button type="button" onClick={() => onOpenArtifact(result.dataSource)}>查看清洗数据</button>
        <button type="button" onClick={() => onOpenArtifact(result.qualityReport)}>质量记录</button>
      </div>
    </header>

    <section className={`insight-status ${status === "趋势向好" ? "positive" : status === "需要关注" ? "caution" : "neutral"}`}>
      <div><span>当前数据画像</span><strong>{status}</strong></div>
      <p>{primaryMetric ? `最新记录以 ${primaryMetric.label} 为主线；点击任一指标可回看对应数据与质量记录。` : "尚未识别到可计算的康复指标。"}</p>
      <small>Galen Insight · 基于当前清洗版本</small>
    </section>

    <section className="insight-timeline-bridge" aria-label="写入 Rehab ID 时间轴">
      <div>
        <span>REHAB ID TIMELINE</span>
        <h2>把这份数据写入连续康复时间轴</h2>
        <p>{caseIds.length > 1 ? `识别到 ${caseIds.length} 个 Rehab ID，将按每行数据自动分别建立记录。` : "数据、质量报告和每个指标将以不可覆盖的来源记录写入对应 Rehab ID。"}</p>
      </div>
      <div className="insight-timeline-action">
        {caseIds.length <= 1 && <label>Rehab ID<input value={caseId} onChange={(event) => setCaseId(event.target.value)} aria-label="Rehab ID" /></label>}
        <button type="button" className="insight-timeline-import" disabled={!canImport || importing} onClick={() => void importTimeline()}>
          {importing ? "正在写入…" : "写入时间轴 →"}
        </button>
      </div>
      {importError && <p className="insight-timeline-error">写入失败：{importError}</p>}
    </section>

    {result.metrics.length > 0 && <section className="insight-section">
      <div className="insight-section-heading"><div><span>STATE CHANGE</span><h2>关键状态变化</h2></div><p>每张卡片保留从首个记录到最新记录的实际数据轨迹。</p></div>
      <div className="insight-metric-grid">{result.metrics.map((metric) => <MetricCard metric={metric} key={metric.id} />)}</div>
    </section>}

    <section className="insight-bottom-grid">
      <section className="insight-section insight-findings">
        <div className="insight-section-heading"><div><span>WHAT CHANGED</span><h2>系统发现</h2></div></div>
        <ol>{result.findings.map((finding, index) => <li key={finding}><b>{String(index + 1).padStart(2, "0")}</b><p>{finding}</p></li>)}</ol>
      </section>
      <aside className="insight-provenance">
        <span>WHY THIS RESULT</span><h2>数据依据</h2>
        <p>每一项趋势都能回到本次生成的清洗数据和质量记录。</p>
        <button type="button" onClick={() => onOpenArtifact(result.dataSource)}>原始字段映射与清洗版本 <b>↗</b></button>
        <button type="button" onClick={() => onOpenArtifact(result.qualityReport)}>缺失、异常与重复记录 <b>↗</b></button>
        {result.dataNotes.length > 0 && <div className="insight-data-notes">{result.dataNotes.map((note) => <span key={note}>{note}</span>)}</div>}
      </aside>
    </section>
  </main>;
}
