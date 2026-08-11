import { useRef, useEffect, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { StatusDot, Tag, ApprovalCard } from "./ui/primitives";
import { TokenRing } from "./TokenRing";
import type { ChatMessage, ModelConfig } from "../types";

// ---------------------------------------------------------------------------
// Block type detection from message content
// ---------------------------------------------------------------------------
type ThreadBlock =
  | { kind: "user-request"; text: string }
  | { kind: "ai-plan"; text: string }
  | { kind: "tool-execution"; text: string }
  | { kind: "revision-suggestion"; text: string }
  | { kind: "evidence-link"; text: string }
  | { kind: "approval-needed"; text: string }
  | { kind: "generic-text"; text: string }
  | { kind: "thinking"; text: string }
  | { kind: "error"; text: string };

function classifyBlock(text: string): ThreadBlock {
  if (!text) return { kind: "generic-text", text };
  if (text.startsWith("## 执行计划") || text.includes("执行计划")) {
    return { kind: "ai-plan", text };
  }
  if (text.startsWith("[工具调用]") || text.includes("工具调用")) {
    return { kind: "tool-execution", text };
  }
  if (text.includes("修订建议") || text.includes("建议修改")) {
    return { kind: "revision-suggestion", text };
  }
  if (text.includes("证据") && (text.includes("PMID") || text.includes("引用"))) {
    return { kind: "evidence-link", text };
  }
  if (text.includes("待签核") || text.includes("请确认")) {
    return { kind: "approval-needed", text };
  }
  return { kind: "generic-text", text };
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------
interface ResearchExecutionThreadProps {
  messages: ChatMessage[];
  streaming: string;
  thinking: string;
  sending: boolean;
  error: string | null;
  backendAvailable: boolean;
  input: string;
  onInputChange: (v: string) => void;
  onSend: () => void;
  models: ModelConfig[];
  selectedModel: string;
  onModelChange: (model: string) => void;
  // Callbacks for thread actions
  onApprove?: (messageId: number) => void;
  onReject?: (messageId: number) => void;
  onViewEvidence?: (messageId: number) => void;
  onRevisionRequest?: (actionId: string, selectedText: string) => void;
}

interface ModelSelectorProps {
  models: ModelConfig[];
  value: string;
  onChange: (model: string) => void;
  disabled: boolean;
}

function ModelSelector({ models, value, onChange, disabled }: ModelSelectorProps) {
  const [expanded, setExpanded] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const current = models.find((item) => item.name === value);

  useEffect(() => {
    if (!expanded) return;
    const closeOnOutsideClick = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setExpanded(false);
    };
    window.addEventListener("pointerdown", closeOnOutsideClick);
    return () => window.removeEventListener("pointerdown", closeOnOutsideClick);
  }, [expanded]);

  return (
    <div className={`composer-model-selector ${expanded ? "expanded" : ""}`} ref={rootRef}>
      <button
        type="button"
        className="composer-model-trigger"
        onClick={() => setExpanded((open) => !open)}
        disabled={disabled || models.length === 0}
        aria-expanded={expanded}
        aria-haspopup="listbox"
        title="切换当前对话使用的模型"
      >
        <span className="composer-model-mark" aria-hidden="true" />
        <span className="composer-model-current">
          {current?.name ?? (models.length > 0 ? "选择模型" : "未配置模型")}
        </span>
        <span className="composer-model-chevron" aria-hidden="true">⌃</span>
      </button>

      <div className="composer-model-panel" role="listbox" aria-label="可用模型">
        <div className="composer-model-panel-label">当前对话模型</div>
        {models.map((item) => {
          const active = item.name === value;
          return (
            <button
              type="button"
              key={item.name}
              className={`composer-model-option ${active ? "active" : ""}`}
              role="option"
              aria-selected={active}
              onClick={() => {
                onChange(item.name);
                setExpanded(false);
              }}
            >
              <span className="composer-model-option-copy">
                <strong>{item.name}</strong>
                {item.model_id !== item.name && <small>{item.model_id}</small>}
              </span>
              <span className="composer-model-check" aria-hidden="true">
                {active ? "✓" : ""}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------
export function ResearchExecutionThread({
  messages,
  streaming,
  thinking,
  sending,
  error,
  backendAvailable,
  input,
  onInputChange,
  onSend,
  models,
  selectedModel,
  onModelChange,
  onApprove,
  onReject,
  onViewEvidence,
}: ResearchExecutionThreadProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, thinking]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (input.trim() && !sending) onSend();
    }
  };

  // -------------------------------------------------------------------
  // Render a single thread block
  // -------------------------------------------------------------------
  const renderBlock = (block: ThreadBlock, msgIndex: number) => {
    switch (block.kind) {
      case "user-request":
        return (
          <div className="thread-block thread-block-user">
            <div className="thread-block-header">
              <span className="thread-block-role">研究者</span>
            </div>
            <div className="thread-block-body">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
                {block.text}
              </ReactMarkdown>
            </div>
          </div>
        );

      case "ai-plan":
        return (
          <div className="thread-block thread-block-plan">
            <div className="thread-block-header">
              <StatusDot tone="active">AI 执行计划</StatusDot>
            </div>
            <div className="thread-block-body">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
                {block.text}
              </ReactMarkdown>
            </div>
          </div>
        );

      case "tool-execution":
        return (
          <div className="thread-block thread-block-tool">
            <div className="thread-block-header">
              <Tag type="execution">工具执行</Tag>
            </div>
            <div className="thread-block-body">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
                {block.text}
              </ReactMarkdown>
            </div>
          </div>
        );

      case "revision-suggestion":
        return (
          <div className="thread-block thread-block-revision">
            <div className="thread-block-header">
              <Tag type="status">修订建议</Tag>
            </div>
            <div className="thread-block-body">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
                {block.text}
              </ReactMarkdown>
            </div>
            <div className="thread-block-actions">
              <button
                className="btn btn-primary btn-sm"
                onClick={() => onApprove?.(msgIndex)}
              >
                接受修订
              </button>
              <button
                className="btn btn-ghost btn-sm"
                onClick={() => onReject?.(msgIndex)}
              >
                要求修订
              </button>
            </div>
          </div>
        );

      case "evidence-link":
        return (
          <div className="thread-block thread-block-evidence">
            <div className="thread-block-header">
              <Tag type="evidence">证据链已关联</Tag>
              <button
                className="btn btn-ghost btn-sm"
                onClick={() => onViewEvidence?.(msgIndex)}
              >
                查看依据
              </button>
            </div>
            <div className="thread-block-body">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
                {block.text}
              </ReactMarkdown>
            </div>
          </div>
        );

      case "approval-needed":
        return (
          <ApprovalCard
            reason={block.text.slice(0, 200)}
            source="执行计划自动生成"
            impact="此操作将修改文档内容"
            onApprove={() => onApprove?.(msgIndex)}
            onViewEvidence={() => onViewEvidence?.(msgIndex)}
            onReject={() => onReject?.(msgIndex)}
          />
        );

      case "thinking":
        return (
          <div className="thread-block thread-block-thinking">
            <div className="thread-block-header">
              <StatusDot tone="idle">思考中</StatusDot>
            </div>
            <div className="thread-block-body thread-thinking-content">
              {block.text}
            </div>
          </div>
        );

      case "error":
        return (
          <div className="thread-block thread-block-error">
            <div className="thread-block-header">
              <StatusDot tone="error">错误</StatusDot>
            </div>
            <div className="thread-block-body">{block.text}</div>
          </div>
        );

      default:
        return (
          <div className="thread-block thread-block-text">
            <div className="thread-block-header">
              <span className="thread-block-role">Galen</span>
            </div>
            <div className="thread-block-body">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
                {block.text}
              </ReactMarkdown>
            </div>
          </div>
        );
    }
  };

  // -------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------
  return (
    <div className="execution-thread">
      {/* ── Thread header ── */}
      <div className="thread-header">
        <h2>科研执行线程</h2>
        <StatusDot tone={sending ? "active" : "idle"}>
          {sending ? "AI 运行中" : backendAvailable ? "就绪" : "离线"}
        </StatusDot>
      </div>

      {/* ── Thread messages ── */}
      <div className="thread-messages">
        {messages.length === 0 && !thinking && (
          <div className="thread-empty">
            <p className="thread-empty-title">科研执行线程</p>
            <p className="thread-empty-desc">
              在此提出研究目标，Galen 将生成执行计划、调用工具、
              提出修订建议并在关键节点请求研究者确认。
            </p>
          </div>
        )}

        {messages.map((msg, i) => {
          const isUser = msg.role === "user";
          if (isUser) {
            // Classify user messages as research requests
            const block = classifyBlock(msg.content);
            return (
              <div key={i}>
                {renderBlock(
                  { ...block, kind: "user-request" },
                  i,
                )}
              </div>
            );
          }

          // Split assistant messages into blocks by double-newline
          const blocks = splitIntoBlocks(msg.content);
          return (
            <div key={i}>
              {blocks.map((block, j) => (
                <div key={`${i}-${j}`}>{renderBlock(block, i)}</div>
              ))}
            </div>
          );
        })}

        {/* Streaming thinking */}
        {thinking && (
          <div className="thread-block thread-block-thinking">
            <div className="thread-block-header">
              <StatusDot tone="idle">推理中</StatusDot>
            </div>
            <div className="thread-block-body thread-thinking-content">
              {thinking}
            </div>
          </div>
        )}

        {/* Streaming indicator */}
        {streaming.length > 0 && !thinking && (
          <div className="thread-streaming-dot">
            <span className="thinking-dot" />
            Galen 正在回复...
          </div>
        )}

        {/* Error display */}
        {error && (
          <div className="thread-block thread-block-error">
            <div className="thread-block-header">
              <StatusDot tone="error">错误</StatusDot>
            </div>
            <div className="thread-block-body">{error}</div>
          </div>
        )}

        <div ref={bottomRef} />
      </div>

      {/* ── Thread input ── */}
      <div className="thread-input-area">
        <div className="thread-composer">
          <textarea
            className="thread-input"
            placeholder="输入研究请求…"
            value={input}
            onChange={(e) => onInputChange(e.target.value)}
            onKeyDown={handleKeyDown}
            rows={2}
            disabled={!backendAvailable || sending}
          />
          <div className="thread-composer-toolbar">
            <ModelSelector
              models={models}
              value={selectedModel}
              onChange={onModelChange}
              disabled={!backendAvailable || sending}
            />
            <span className="thread-composer-hint">Enter 发送 · Shift+Enter 换行</span>
            <TokenRing messages={messages} />
            <button
              className="btn btn-primary thread-send-button"
              onClick={onSend}
              disabled={!backendAvailable || sending || !input.trim()}
            >
              发送
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Block splitting helper
// ---------------------------------------------------------------------------
function splitIntoBlocks(text: string): ThreadBlock[] {
  if (!text) return [];
  // Split by double newline, then classify each segment
  return text
    .split(/\n\n+/)
    .map((s) => s.trim())
    .filter(Boolean)
    .map(classifyBlock);
}
