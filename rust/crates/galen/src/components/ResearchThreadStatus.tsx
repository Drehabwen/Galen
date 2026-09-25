import { useEffect, useState } from "react";
import type { ToolProgress } from "../hooks/useChat";
import type { ChatRunSummary } from "../types";
import { StatusDot } from "./ui/primitives";

interface ResearchThreadStatusProps {
  backendAvailable: boolean;
  workspaceSelected: boolean;
  sending: boolean;
  streaming: string;
  thinking: string;
  latestRunMetrics: ChatRunSummary | null;
  toolProgress?: ToolProgress | null;
  toolProgressHistory: ToolProgress[];
}

export function ResearchThreadStatus({
  backendAvailable,
  workspaceSelected,
  sending,
  streaming,
  thinking,
  latestRunMetrics,
  toolProgress,
  toolProgressHistory,
}: ResearchThreadStatusProps) {
  const [elapsedSeconds, setElapsedSeconds] = useState(0);

  useEffect(() => {
    if (!sending) {
      setElapsedSeconds(0);
      return;
    }
    const startedAt = Date.now();
    const update = () => setElapsedSeconds(Math.floor((Date.now() - startedAt) / 1000));
    update();
    const timer = window.setInterval(update, 1000);
    return () => window.clearInterval(timer);
  }, [sending]);

  const activityLabel = !backendAvailable
    ? "离线"
    : !sending
      ? "等待研究问题"
      : streaming
        ? `正在生成回答 · ${elapsedSeconds}s`
        : thinking
          ? `模型推理中 · ${elapsedSeconds}s`
          : elapsedSeconds < 2
            ? "正在组装上下文"
            : `等待模型响应 · ${elapsedSeconds}s`;

  const cacheTotal = latestRunMetrics
    ? latestRunMetrics.cacheReadInputTokens + latestRunMetrics.cacheCreationInputTokens
    : 0;
  const cacheHitRate = latestRunMetrics && cacheTotal > 0
    ? Math.round((latestRunMetrics.cacheReadInputTokens / cacheTotal) * 100)
    : null;
  const progressPercent = toolProgress
    ? Math.min(96, Math.max(4, Math.round((toolProgress.turn / toolProgress.maxTurns) * 100)))
    : 0;

  return (
    <>
      <div className="thread-header">
        <div><span className="thread-header-kicker">RESEARCH BRIEF</span><h2>研究委托</h2></div>
        <StatusDot tone={sending ? "active" : "idle"}>{activityLabel}</StatusDot>
      </div>

      {!workspaceSelected && (
        <div className="workspace-inline-notice" role="status">
          <span className="workspace-inline-notice-icon" aria-hidden="true">⌂</span>
          <span><strong>尚未选择工作区</strong>：可以继续讨论、检索和起草；保存证据、文件与最终产物前再选择工作区即可。</span>
        </div>
      )}

      {sending && toolProgress && (
        <div className="thread-run-progress" aria-live="polite">
          <div className="thread-run-progress__summary">
            <span>执行中</span>
            <strong>{toolProgress.tool}</strong>
            <span>{toolProgress.phase === "running" ? "正在执行" : toolProgress.phase === "failed" ? "失败，正在调整" : "已完成"}</span>
            <span>第 {toolProgress.turn}/{toolProgress.maxTurns} 轮</span>
          </div>
          <div className="thread-run-progress__track" aria-label={`当前执行进度：第 ${toolProgress.turn} 轮，共 ${toolProgress.maxTurns} 轮`}>
            <span style={{ width: `${progressPercent}%` }} />
          </div>
          {toolProgressHistory.length > 0 && (
            <div className="thread-run-progress__history">
              {toolProgressHistory.map((item, index) => (
                <span key={`${item.turn}-${item.tool}-${index}`} className={`phase-${item.phase}`}>
                  {item.phase === "completed" ? "✓" : item.phase === "failed" ? "!" : "·"} {item.tool}
                </span>
              ))}
            </div>
          )}
        </div>
      )}

      {!sending && latestRunMetrics && (
        <div className="thread-run-metrics" aria-label="上一轮模型性能">
          <span>上一轮</span><strong>{(latestRunMetrics.totalMs / 1000).toFixed(1)}s</strong>
          <span>首个可见响应</span><strong>{latestRunMetrics.ttftMs == null ? "—" : `${(latestRunMetrics.ttftMs / 1000).toFixed(1)}s`}</strong>
          <span>Token</span><strong>{latestRunMetrics.inputTokens.toLocaleString()} → {latestRunMetrics.outputTokens.toLocaleString()}</strong>
          <span>缓存命中</span><strong>{cacheHitRate == null ? "—" : `${cacheHitRate}%`}</strong>
          {latestRunMetrics.toolCallCount > 0 && <><span>工具</span><strong>{latestRunMetrics.toolCallCount} 次</strong></>}
        </div>
      )}
    </>
  );
}
