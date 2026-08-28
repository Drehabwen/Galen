import { useMemo, useState } from "react";
import type { ChatMessage, DecisionRecord } from "../types";

const CHARS_PER_TOKEN = 4;
const CONTEXT_WINDOW = 32_768;
const TOPIC_LABELS: Record<string, string> = {
  protocol: "协议", sample_size: "样本量", primary_outcome: "主要结局",
  secondary_outcome: "次要结局", follow_up: "随访", population: "研究对象",
  intervention: "干预", control: "对照",
};

interface ContextPanelProps {
  messages: ChatMessage[];
  compacted: boolean;
  decisions: DecisionRecord[];
  onReviseDecision: (id: string, statement: string) => Promise<void>;
  onDismissDecision: (id: string) => Promise<void>;
}

export function ContextPanel({ messages, compacted, decisions, onReviseDecision, onDismissDecision }: ContextPanelProps) {
  const [showDecisions, setShowDecisions] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [pendingId, setPendingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const stats = useMemo(() => {
    const estimatedTokens = Math.ceil(messages.reduce((sum, message) => sum + message.content.length, 0) / CHARS_PER_TOKEN);
    const usagePct = Math.min(100, Math.round((estimatedTokens / CONTEXT_WINDOW) * 100));
    return { estimatedTokens, usagePct, isCompactable: estimatedTokens > (24_000 / CHARS_PER_TOKEN) * 0.75 };
  }, [messages]);
  const activeDecisions = decisions.filter((decision) => decision.status === "active");
  const visibleDecisions = showHistory ? [...decisions].reverse() : [...activeDecisions].reverse();
  const tokenLabel = stats.estimatedTokens >= 1000 ? `${(stats.estimatedTokens / 1000).toFixed(1)}K` : stats.estimatedTokens.toLocaleString();
  const topicLabel = (topic: string) => TOPIC_LABELS[topic] ?? (topic.startsWith("general:") ? "研究约束" : topic);

  const saveRevision = async (decision: DecisionRecord) => {
    if (!draft.trim() || draft.trim() === decision.statement) { setEditingId(null); return; }
    setPendingId(decision.id); setError(null);
    try { await onReviseDecision(decision.id, draft.trim()); setEditingId(null); }
    catch (reason) { setError(String(reason)); }
    finally { setPendingId(null); }
  };
  const dismiss = async (decision: DecisionRecord) => {
    setPendingId(decision.id); setError(null);
    try { await onDismissDecision(decision.id); }
    catch (reason) { setError(String(reason)); }
    finally { setPendingId(null); }
  };

  return (
    <div className="context-panel">
      <span className="context-item">消息 <strong>{messages.length}</strong></span><span className="context-sep" />
      <button type="button" className={`context-consensus-trigger ${showDecisions ? "active" : ""}`} aria-expanded={showDecisions} aria-haspopup="dialog" onClick={() => setShowDecisions((value) => !value)}>
        <span className="context-consensus-signal" />当前共识 <strong>{activeDecisions.length}</strong>
      </button><span className="context-sep" />
      <span className="context-item">Token <strong>{tokenLabel}</strong><span className="context-pct">{stats.usagePct}%</span></span><span className="context-sep" />
      <span className="context-item">缓存 <span className="context-dot ctx-dot-ok" /></span><span className="context-sep" />
      <span className="context-item">压缩 {compacted ? <span className="ctx-tag ctx-tag-done">已压缩</span> : stats.isCompactable ? <span className="ctx-tag ctx-tag-warn">接近阈值</span> : <span className="ctx-tag ctx-tag-idle">正常</span>}</span>
      <div className="context-bar-track" aria-label={`上下文已使用 ${stats.usagePct}%`}><div className="context-bar-fill" style={{ width: `${stats.usagePct}%` }} /></div>

      {showDecisions && (
        <section className="context-consensus-popover" role="dialog" aria-label="当前研究共识">
          <header className="context-consensus-header">
            <div><span className="context-consensus-kicker">DECISION LEDGER</span><strong>当前研究共识</strong><p>只有有效条目会进入下一轮推理。</p></div>
            <button type="button" className="context-consensus-close" aria-label="关闭当前共识" onClick={() => setShowDecisions(false)}>×</button>
          </header>
          <div className="context-consensus-toolbar">
            <span>{activeDecisions.length} 条有效约束</span>
            {decisions.some((decision) => decision.status !== "active") && <button type="button" onClick={() => setShowHistory((value) => !value)}>{showHistory ? "只看当前" : "查看修订历史"}</button>}
          </div>
          {visibleDecisions.length > 0 ? (
            <ol className="context-consensus-list">
              {visibleDecisions.map((decision) => (
                <li key={decision.id} className={`context-decision context-decision-${decision.status}`}>
                  <div className="context-decision-meta">
                    <span>{topicLabel(decision.topic)}</span>
                    <time dateTime={new Date(decision.timestampMs).toISOString()}>{new Date(decision.timestampMs).toLocaleString("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" })}</time>
                    {decision.status !== "active" && <em>{decision.status === "superseded" ? "已被替代" : "已停用"}</em>}
                  </div>
                  {editingId === decision.id ? (
                    <div className="context-decision-editor"><textarea value={draft} autoFocus onChange={(event) => setDraft(event.target.value)} /><div><button type="button" onClick={() => setEditingId(null)}>取消</button><button type="button" className="primary" disabled={pendingId === decision.id} onClick={() => saveRevision(decision)}>保存修订</button></div></div>
                  ) : <p>{decision.statement}</p>}
                  {decision.status === "active" && editingId !== decision.id && <div className="context-decision-actions"><button type="button" onClick={() => { setEditingId(decision.id); setDraft(decision.statement); setError(null); }}>修订</button><button type="button" disabled={pendingId === decision.id} onClick={() => dismiss(decision)}>不再使用</button></div>}
                </li>
              ))}
            </ol>
          ) : <div className="context-consensus-empty"><strong>还没有稳定共识</strong><p>在对话中明确说“样本量定为 48”或“随访改为 16 周”，Galen 会留下可核验记录。</p></div>}
          {error && <p className="context-consensus-error">更新失败：{error}</p>}
        </section>
      )}
    </div>
  );
}
