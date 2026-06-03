import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { ChatMessage } from "../types";

interface Props {
  messages: ChatMessage[];
  streaming: string;
  sending: boolean;
  error: string | null;
}

export function ChatPanel({ messages, streaming, sending, error }: Props) {
  return (
    <div className="chat-messages">
      {messages.map((msg, i) => (
        <div key={i} className={`chat-msg chat-${msg.role}`}>
          <div className="msg-header">
            {msg.role === "user" ? "🧑 你" : "🤖 VIBE Paper"}
          </div>
          <div className="msg-body">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>
              {msg.content}
            </ReactMarkdown>
          </div>
        </div>
      ))}

      {streaming && (
        <div className="chat-msg chat-assistant">
          <div className="msg-header">
            🤖 VIBE Paper
            {sending && <span className="spinner" />}
          </div>
          <div className="msg-body">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>
              {streaming}
            </ReactMarkdown>
          </div>
        </div>
      )}

      {error && (
        <div className="chat-msg chat-error">
          <div className="msg-header">❌ 错误</div>
          <div className="msg-body">{error}</div>
        </div>
      )}
    </div>
  );
}
