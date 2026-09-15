import type { ConnectorPreview } from "../domain/connectors";
import type { RehabTimelineImportOutput } from "../domain/analysisResult";

interface ConnectorDataCanvasProps {
  preview: ConnectorPreview | null;
  result: RehabTimelineImportOutput | null;
  latestAssessments?: number;
  onContinueResearch?: () => void;
}

export function ConnectorDataCanvas({ preview, result, latestAssessments, onContinueResearch }: ConnectorDataCanvasProps) {
  const imported = Boolean(result);
  const caseIds = result?.caseIds ?? preview?.cases.map((item) => item.caseId) ?? [];
  const eventCount = result?.importedEventCount ?? preview?.timepointCount ?? 0;
  const observationCount = result?.importedObservationCount ?? preview?.measurementCount ?? 0;
  const sourceLabel = preview?.sourceLabel ?? "康复数据源";
  const previewTitle = `${sourceLabel}数据预览`;
  const sourceProvenance = preview?.connectionMode === "live_bridge"
    ? `${sourceLabel} · 本地数据桥`
    : sourceLabel;

  return (
    <div className={`connector-data-canvas ${imported ? "is-imported" : ""}`}>
      <header className="connector-data-canvas-header">
        <div>
          <span>REHABID DATA RECEIPT</span>
          <h1>{imported ? "研究数据已就位" : previewTitle}</h1>
          <p>{imported ? "数据已标准化并进入当前研究的纵向时间轴。" : "确认前只展示范围与结构，不写入研究数据。"}</p>
        </div>
        <strong>{imported ? "IMPORTED" : "PREVIEW"}</strong>
      </header>

      <section className="connector-data-identity">
        <span>RehabID</span>
        <div>
          {caseIds.length > 0 ? caseIds.map((caseId) => <strong key={caseId}>{caseId}</strong>) : <strong>等待识别</strong>}
        </div>
        <small>{latestAssessments ? `最近 ${latestAssessments} 个评估时间点` : "当前可用评估范围"}</small>
      </section>

      <section className="connector-data-flow" aria-label="数据治理流程">
        {[
          ["01", "发现", preview?.sourceLabel ?? "康复师工作台"],
          ["02", "标准化", `${observationCount} 条观察`],
          ["03", "时间轴", `${eventCount} 个时间点`],
          ["04", "研究上下文", imported ? "可调用" : "等待确认"],
        ].map(([index, label, value], itemIndex) => (
          <div className={imported || itemIndex === 0 ? "active" : ""} key={index}>
            <i>{index}</i>
            <span>{label}</span>
            <strong>{value}</strong>
          </div>
        ))}
      </section>

      <section className="connector-data-provenance">
        <div>
          <span>来源</span>
          <strong>{sourceProvenance}</strong>
        </div>
        <div>
          <span>身份字段</span>
          <strong>未写入研究数据</strong>
        </div>
        <div>
          <span>可追溯记录</span>
          <strong>{result ? result.receipt.path.split(/[/\\]/).pop() : "确认后生成"}</strong>
        </div>
      </section>

      <footer>
        <span className="connector-data-signal" aria-hidden="true" />
        {imported ? <><span>RehabID 时间轴已就位，可直接交给 PI-Galen 推进研究。</span>{onContinueResearch && <button type="button" onClick={onContinueResearch}>进入 PI 对话 →</button>}</> : "范围确认后，Galen 才会建立 RehabID 及来源回执。"}
      </footer>
    </div>
  );
}
