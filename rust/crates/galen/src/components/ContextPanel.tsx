import { useMemo } from "react";
import type { ChatMessage } from "../types";

// Rough estimate: 4 chars ≈ 1 token
const CHARS_PER_TOKEN = 4;
const CONTEXT_WINDOW = 32_768;

interface ContextPanelProps {
  messages: ChatMessage[];
  compacted: boolean;
}

export function ContextPanel({ messages, compacted }: ContextPanelProps) {
  const stats = useMemo(() => {
    const totalChars = messages.reduce((sum, m) => sum + m.content.length, 0);
    const estimatedTokens = Math.ceil(totalChars / CHARS_PER_TOKEN);
    const usagePct = Math.min(100, Math.round((estimatedTokens / CONTEXT_WINDOW) * 100));
    const compactAt = 24_000 / CHARS_PER_TOKEN;
    return {
      messages: messages.length,
      estimatedTokens,
      usagePct,
      compactAt,
      isCompactable: estimatedTokens > compactAt * 0.75,
    };
  }, [messages]);

  const tokenLabel =
    stats.estimatedTokens >= 1000
      ? `${(stats.estimatedTokens / 1000).toFixed(1)}K`
      : stats.estimatedTokens.toLocaleString();

  return (
    <div className="context-panel">
      <span className="context-item">
        消息 <strong>{stats.messages}</strong>
      </span>
      <span className="context-sep" />
      <span className="context-item">
        Token <strong>{tokenLabel}</strong>
        <span className="context-pct">{stats.usagePct}%</span>
      </span>
      <span className="context-sep" />
      <span className="context-item">
        缓存 <span className={`context-dot ${compacted ? "ctx-dot-ok" : "ctx-dot-ok"}`} />
      </span>
      <span className="context-sep" />
      <span className="context-item">
        压缩{" "}
        {compacted ? (
          <span className="ctx-tag ctx-tag-done">已压缩</span>
        ) : stats.isCompactable ? (
          <span className="ctx-tag ctx-tag-warn">接近阈值</span>
        ) : (
          <span className="ctx-tag ctx-tag-idle">正常</span>
        )}
      </span>
      {/* thin progress bar at the very right */}
      <div className="context-bar-track">
        <div
          className="context-bar-fill"
          style={{ width: `${stats.usagePct}%` }}
        />
      </div>
    </div>
  );
}
