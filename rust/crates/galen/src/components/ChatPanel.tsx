import { useState, useCallback } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { ChatMessage } from "../types";

interface Props {
  messages: ChatMessage[];
  streaming: string;
  thinking: string;
  sending: boolean;
  error: string | null;
  backendAvailable: boolean;
  input: string;
  onInputChange: (value: string) => void;
  onSend: () => void;
}

function formatTime(ts: number): string {
  const d = new Date(ts);
  const now = new Date();
  const isToday = d.toDateString() === now.toDateString();
  const time = d.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
  return isToday ? time : `${d.toLocaleDateString("zh-CN")} ${time}`;
}

function ThinkingBox({ content }: { content: string }) {
  const [expanded, setExpanded] = useState(true);
  if (!content) return null;
  return (
    <details className="thinking-box" open={expanded}>
      <summary className="thinking-summary" onClick={(e) => { e.preventDefault(); setExpanded(!expanded); }}>
        思考过程{(content.length > 80 ? ` (${Math.ceil(content.length / 80)} 行)` : "")}
      </summary>
      <pre className="thinking-content">{content}</pre>
    </details>
  );
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }, [text]);
  return (
    <button
      className={`msg-action-btn ${copied ? "copied" : ""}`}
      onClick={handleCopy}
      title="复制"
    >
        {copied ? "已复制" : "复制"}
    </button>
  );
}

export function ChatPanel({
  messages,
  streaming,
  thinking,
  sending,
  error,
  backendAvailable,
  input,
  onInputChange,
  onSend,
}: Props) {
  // Build unified display list: permanent messages + streaming placeholder
  // This avoids the "disappear then reappear" visual glitch when Done fires.
  const displayMessages = [...messages];
  const isStreaming = sending && (streaming.length > 0 || thinking.length > 0);
  if (isStreaming) {
    // Add a virtual streaming message at the end
    displayMessages.push({
      role: "assistant",
      content: streaming,
      timestamp: Date.now(),
    } as ChatMessage);
  }

  return (
    <div className="chat-panel-inner">
      <div className="chat-messages">
        {displayMessages.map((msg, i) => {
          const isLast = i === displayMessages.length - 1;
          const isStreamingMsg = isLast && isStreaming;
          return (
          <div key={i} className={`chat-msg chat-${msg.role}`}>
            <div className="msg-card">
              <div className="msg-row">
                <div className="msg-avatar">
                  {msg.role === "user" ? "你" : "A"}
                </div>
                <span className="msg-role">
                  {msg.role === "user" ? "你" : "Agent"}
                </span>
                {msg.model && (
                  <span className="msg-model-badge">{msg.model}</span>
                )}
                {isStreamingMsg && (
                  <span className="msg-model-badge">生成中</span>
                )}
                <span className="msg-time">{formatTime(msg.timestamp)}</span>
                {!isStreamingMsg && (
                  <div className="msg-actions">
                    <CopyButton text={msg.content} />
                  </div>
                )}
              </div>
              <div className="msg-body">
                {isStreamingMsg && thinking && <ThinkingBox content={thinking} />}
                {isStreamingMsg ? (
                  <div className="streaming-text">
                    {msg.content || "..."}
                    <span className="streaming-cursor" />
                  </div>
                ) : (
                  <ReactMarkdown remarkPlugins={[remarkGfm]}>
                    {msg.content}
                  </ReactMarkdown>
                )}
              </div>
            </div>
          </div>
        )})}

        {error && (
          <div className="chat-msg chat-error">
            <div className="msg-card">
              <div className="msg-row">
                <div className="msg-avatar">!</div>
                <span className="msg-role">发生错误</span>
              </div>
              <div className="msg-body">{error}</div>
            </div>
          </div>
        )}
      </div>

      {/* Inline input area */}
      <div className="chat-input-area">
        <input
          type="text"
          className="chat-input-inline"
          placeholder={
            backendAvailable
              ? "让 Agent 检查方案、采集、随访、统计或写作材料..."
              : "浏览器预览未连接 Tauri 后端，请启动桌面应用使用 Agent"
          }
          value={input}
          disabled={!backendAvailable}
          onChange={(e) => onInputChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
              onSend();
            }
          }}
        />
        <button
          className="btn btn-primary btn-sm-send"
          disabled={!backendAvailable || sending || !input.trim()}
          onClick={onSend}
        >
          {sending ? "..." : "发送"}
        </button>
      </div>
    </div>
  );
}
