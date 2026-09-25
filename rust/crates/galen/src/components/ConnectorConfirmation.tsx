import type { RehabTimelineImportOutput } from "../domain/analysisResult";
import type { ConnectorPreview } from "../domain/connectors";

interface ConnectorConfirmationProps {
  preview?: ConnectorPreview | null;
  loading: boolean;
  error?: string | null;
  result?: RehabTimelineImportOutput | null;
  latestAssessments?: number;
  onConfirm?: () => void;
  onDismiss?: () => void;
}

export function ConnectorConfirmation({
  preview,
  loading,
  error,
  result,
  latestAssessments,
  onConfirm,
  onDismiss,
}: ConnectorConfirmationProps) {
  if (!loading && !preview && !error && !result) return null;

  const status = result
    ? "已写入 RehabID"
    : loading
      ? "正在发现"
      : error
        ? "需要处理"
        : "等待确认";

  return (
    <section className="connector-confirmation" aria-live="polite" aria-label="科研数据源操作">
      <div className="connector-confirmation-rail" aria-hidden="true">
        <span className={result ? "complete" : error ? "error" : "active"} />
      </div>
      <div className="connector-confirmation-main">
        <div className="connector-confirmation-kicker">
          <span>RESEARCH DATA SOURCE</span>
          <strong>{status}</strong>
        </div>
        {loading && !preview && <p>正在读取康复师工作台最近一次数据备份…</p>}
        {error && <p className="connector-confirmation-error">{error}</p>}
        {preview && !result && (
          <>
            <h3>{preview.sourceLabel}</h3>
            <p>{preview.message}</p>
            <span className="connector-confirmation-mode">
              {preview.connectionMode === "live_bridge" ? "本地数据桥 · 自动同步" : "最近备份 · 兼容模式"}
            </span>
            <div className="connector-confirmation-stats">
              <span><strong>{preview.cases.length}</strong> 个对象</span>
              <span><strong>{preview.sessionCount}</strong> 次接诊</span>
              <span><strong>{preview.timepointCount}</strong> 个时间点</span>
              <span><strong>{preview.measurementCount}</strong> 条观察</span>
            </div>
            {preview.cases.length > 0 && (
              <div className="connector-case-strip">
                {preview.cases.slice(0, 4).map((item) => (
                  <span key={item.caseId}><strong>{item.caseId}</strong>{item.timepointCount} 个时间点</span>
                ))}
              </div>
            )}
            {latestAssessments && <small>本次只写入每个对象最近 {latestAssessments} 个评估时间点。</small>}
            <div className="connector-confirmation-actions">
              <button type="button" className="btn btn-primary" disabled={!preview.canImport || loading} onClick={onConfirm}>确认获取</button>
              <button type="button" className="btn btn-ghost" onClick={onDismiss}>取消</button>
            </div>
          </>
        )}
        {result && (
          <>
            <h3>数据已进入研究时间轴</h3>
            <p>已建立 {result.caseIds.length} 个 RehabID，新增 {result.importedEventCount} 个时间点和 {result.importedObservationCount} 条观察记录。</p>
            <div className="connector-case-strip">
              {result.caseIds.map((caseId) => <span key={caseId}><strong>{caseId}</strong>可继续在对话中分析</span>)}
            </div>
            <div className="connector-confirmation-actions">
              <button type="button" className="btn btn-ghost" onClick={onDismiss}>完成</button>
            </div>
          </>
        )}
      </div>
    </section>
  );
}
