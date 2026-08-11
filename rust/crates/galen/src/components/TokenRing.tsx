import { useState, useRef } from "react";
import type { ChatMessage } from "../types";

// Rough: 4 chars ≈ 1 token
const CHARS_PER_TOKEN = 4;
const CONTEXT_WINDOW = 32_768;
const RING_SIZE = 28;
const RING_R = 11;
const RING_C = 2 * Math.PI * RING_R;

interface TokenRingProps {
  messages: ChatMessage[];
}

function fmtCompact(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + "M";
  if (n >= 1_000) return (n / 1_000).toFixed(1) + "k";
  return String(n);
}

export function TokenRing({ messages }: TokenRingProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const totalChars = messages.reduce((s, m) => s + m.content.length, 0);
  const estimatedTokens = Math.ceil(totalChars / CHARS_PER_TOKEN);
  const usagePct = Math.min(100, Math.round((estimatedTokens / CONTEXT_WINDOW) * 100));
  const ringOffset = RING_C * (1 - usagePct / 100);

  const tone = usagePct > 85 ? "high" : usagePct > 60 ? "mid" : "low";

  return (
    <div className="token-ring-wrap" ref={ref}>
      <div
        className={`token-ring token-ring-${tone} ${open ? "token-ring-open" : ""}`}
        onMouseEnter={() => setOpen(true)}
        onMouseLeave={() => setOpen(false)}
      >
        <svg width={RING_SIZE} height={RING_SIZE} viewBox={`0 0 ${RING_SIZE} ${RING_SIZE}`}>
          <circle cx={RING_SIZE / 2} cy={RING_SIZE / 2} r={RING_R}
            fill="none" stroke="var(--border-muted)" strokeWidth={2.5} />
          <circle cx={RING_SIZE / 2} cy={RING_SIZE / 2} r={RING_R}
            fill="none" strokeWidth={2.5} strokeLinecap="round"
            strokeDasharray={RING_C} strokeDashoffset={ringOffset}
            transform={`rotate(-90 ${RING_SIZE / 2} ${RING_SIZE / 2})`}
            stroke={
              tone === "high" ? "var(--error)" :
              tone === "mid" ? "var(--warning)" : "var(--accent)"
            }
          />
        </svg>
      </div>
      {open && (
        <div className="token-ring-popover">
          <div className="token-ring-popover-row">
            <span>Token</span>
            <strong>{fmtCompact(estimatedTokens)} / {fmtCompact(CONTEXT_WINDOW)}</strong>
          </div>
          <div className="token-ring-popover-row">
            <span>窗口占用</span>
            <strong>{usagePct}%</strong>
          </div>
          <div className="token-ring-popover-row">
            <span>消息数</span>
            <strong>{messages.length}</strong>
          </div>
          <div className="token-ring-popover-row">
            <span>缓存前缀</span>
            <strong style={{ color: "var(--success)" }}>稳定</strong>
          </div>
          {usagePct > 85 && (
            <div className="token-ring-popover-note">
              窗口接近上限，将自动压缩早期消息
            </div>
          )}
        </div>
      )}
    </div>
  );
}
