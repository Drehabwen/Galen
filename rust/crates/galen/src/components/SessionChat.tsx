import { useState, useRef, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { StatusDot } from "./ui/primitives";
import type { ChatMessage } from "../types";
import type { SessionNode } from "../domain/sessionTypes";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------
interface SessionChatProps {
  node: SessionNode;
  onClose: () => void;
  onFlowBack?: (node: SessionNode, summary: string) => void;
  backendAvailable: boolean;
  modelAlias: string;
  thinkingLevel?: string;
  /** When true, the session starts autonomously: the node goal is sent
   *  automatically and [SESSION_DONE] triggers an automatic flow-back. */
  autoRun?: boolean;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------
export function SessionChat({
  node,
  onClose,
  onFlowBack,
  backendAvailable,
  modelAlias,
  thinkingLevel,
  autoRun,
}: SessionChatProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [streaming, setStreaming] = useState("");
  const [thinking, setThinking] = useState("");
  const [error, setError] = useState<string | null>(null);
  const sendingRef = useRef(false);
  const bottomRef = useRef<HTMLDivElement>(null);
  const doneHandledRef = useRef(false);
  const messagesRef = useRef(messages);
  const onFlowBackRef = useRef(onFlowBack);
  const autoRunRef = useRef(false);
  onFlowBackRef.current = onFlowBack;
  useEffect(() => {
    messagesRef.current = messages;
  }, [messages]);

  // Register tagged event listeners for this session (isolated from main chat)
  const tag = node.id;
  useEffect(() => {
    if (!backendAvailable) return;
    const unlisteners: UnlistenFn[] = [];
    let cancelled = false;

    const register = async () => {
      const u1 = await listen<string>(`chat-delta:${tag}`, (e) => {
        if (!cancelled) setStreaming((p) => p + e.payload);
      });
      const u2 = await listen<string>(`chat-done:${tag}`, (e) => {
        if (cancelled || doneHandledRef.current) return;
        doneHandledRef.current = true;
        const content = e.payload;
        setMessages((p) => [...p, { role: "assistant", content, timestamp: Date.now() }]);
        setStreaming("");
        setSending(false);
        sendingRef.current = false;
        // Autonomous completion: the agent signals SESSION_DONE -> flow back
        if (content.includes("[SESSION_DONE]")) {
          const summary =
            content.replace(/\[SESSION_DONE\]\s*/, "").trim() || content;
          onFlowBackRef.current?.(node, summary);
        }
      });
      const u3 = await listen<string>(`chat-error:${tag}`, (e) => {
        if (!cancelled) {
          setError(e.payload);
          setSending(false);
          sendingRef.current = false;
        }
      });
      const u4 = await listen<string>(`chat-thinking-delta:${tag}`, (e) => {
        if (!cancelled) setThinking((p) => p + e.payload);
      });
      const u5 = await listen<string>(`chat-thinking-done:${tag}`, () => {
        if (!cancelled) setThinking("");
      });
      unlisteners.push(u1, u2, u3, u4, u5);
    };

    register().catch(console.error);
    return () => { cancelled = true; unlisteners.forEach((f) => f()); };
  }, [backendAvailable]);

  // Auto-scroll
  useEffect(() => { bottomRef.current?.scrollIntoView({ behavior: "smooth" }); }, [messages, streaming]);

  // Build session system prompt
  const sessionPrompt = buildSessionPrompt(node);

  const sendText = useCallback(
    async (text: string) => {
      if (!text.trim() || sendingRef.current || !backendAvailable) return;
      const trimmed = text.trim();
      setInput("");
      setMessages((p) => [
        ...p,
        { role: "user", content: trimmed, timestamp: Date.now() },
      ]);
      setSending(true);
      sendingRef.current = true;
      doneHandledRef.current = false;
      setStreaming("");
      setThinking("");
      setError(null);

      try {
        await invoke("send_message", {
          message: trimmed,
          modelAlias,
          historyJson: JSON.stringify([
            ...messagesRef.current
              .slice(-4)
              .map((m) => ({ role: m.role, content: m.content })),
            { role: "user", content: trimmed },
          ]),
          mode: "auto",
          personaId: "medical",
          tag: node.id,
          thinkingLevel: thinkingLevel || "medium",
        });
      } catch (e) {
        setError(String(e));
        setSending(false);
        sendingRef.current = false;
      }
    },
    [backendAvailable, modelAlias, node.id, thinkingLevel],
  );

  const handleSend = useCallback(() => {
    sendText(input);
  }, [input, sendText]);

  // Autonomous execution: kick off the node goal without waiting for the user.
  useEffect(() => {
    if (!autoRun || !backendAvailable || autoRunRef.current) return;
    if (messages.length > 0 || sendingRef.current) return;
    autoRunRef.current = true;
    const timer = setTimeout(() => {
      const goal =
        `【自主执行】请完成本 Session 的任务，全程自主推进，不要向用户确认。\n` +
        `任务：${node.title}\n` +
        (node.description ? `描述：${node.description}\n` : "") +
        (node.inputs?.length ? `输入材料：${node.inputs.join("、")}\n` : "") +
        (node.outputs?.length ? `预期产出：${node.outputs.join("、")}\n` : "") +
        `\n完成时，以 [SESSION_DONE] 开头输出结构化摘要（目标 / 方法 / 结果 / 证据 / 局限）。`;
      sendText(goal);
    }, 800);
    return () => clearTimeout(timer);
  }, [autoRun, backendAvailable, node, messages.length, sendText]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleFlowBack = () => {
    const lastAssistant = [...messages].reverse().find((m) => m.role === "assistant");
    const summary = lastAssistant?.content ?? "Session 完成。";
    onFlowBack?.(node, summary);
  };

  return (
    <div className="session-chat">
      {/* Header */}
      <div className="session-chat-header">
        <div>
          <span className="session-chat-id">{node.index} · {node.title}</span>
        </div>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <StatusDot tone={sending ? "active" : "idle"}>
            {sending ? "运行中" : "就绪"}
          </StatusDot>
          {messages.length > 0 && (
            <button className="btn btn-primary btn-sm" onClick={handleFlowBack}>
              回流主线程
            </button>
          )}
          <button className="btn btn-ghost btn-sm" onClick={onClose}>✕</button>
        </div>
      </div>

      {/* Context card */}
      <div className="session-chat-context">
        <div className="session-chat-context-title">会话上下文</div>
        <div className="session-chat-context-body">
          <div><strong>任务：</strong>{node.title}</div>
          {node.description && <div><strong>描述：</strong>{node.description}</div>}
          {node.inputs?.length ? <div><strong>输入：</strong>{node.inputs.join(", ")}</div> : null}
        </div>
      </div>

      {/* Messages */}
      <div className="session-chat-messages">
        {messages.length === 0 && !thinking && (
          <div className="session-chat-empty">
            <p>开始执行「{node.title}」</p>
            <p style={{ fontSize: "var(--text-xs)", color: "var(--text-tertiary)" }}>
              描述你的需求或直接说"开始执行"
            </p>
          </div>
        )}
        {messages.map((msg, i) => (
          <div key={i} className={`session-msg session-msg-${msg.role}`}>
            <div className="session-msg-role">
              {msg.role === "user" ? "你" : "Galen"}
            </div>
            <div className="session-msg-body">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{msg.content}</ReactMarkdown>
            </div>
          </div>
        ))}
        {thinking && (
          <div className="session-msg session-msg-assistant">
            <div className="session-msg-role">思考中</div>
            <div className="session-msg-body session-msg-thinking">{thinking}</div>
          </div>
        )}
        {error && (
          <div className="session-msg session-msg-error">
            <div className="session-msg-role">错误</div>
            <div className="session-msg-body">{error}</div>
          </div>
        )}
        <div ref={bottomRef} />
      </div>

      {/* Input */}
      <div className="session-chat-input-area">
        <textarea
          className="session-chat-input"
          placeholder={`在「${node.title}」中描述你的需求...`}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          rows={2}
          disabled={!backendAvailable || sending}
        />
        <button
          className="btn btn-primary btn-sm"
          onClick={handleSend}
          disabled={!backendAvailable || sending || !input.trim()}
        >
          发送
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function buildSessionPrompt(node: SessionNode): string {
  return [
    `你是 Galen，正在执行科研 Session「${node.title}」。`,
    node.description ? `任务描述：${node.description}` : "",
    node.inputs?.length ? `输入材料：${node.inputs.join("、")}` : "",
    node.outputs?.length ? `预期产出：${node.outputs.join("、")}` : "",
    "专注于当前 Session 的任务，不要讨论其他节点。完成后告知结果。",
  ].filter(Boolean).join("\n");
}
