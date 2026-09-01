import { useRef, useEffect, useState } from "react";
import { ArtifactMarkdown, artifactHref } from "./ArtifactMarkdown";
import { StatusDot, Tag, ApprovalCard } from "./ui/primitives";
import { TokenRing } from "./TokenRing";
import type { ChatMessage, ChatRunSummary, ModelConfig } from "../types";
import type { ArtifactRecord } from "../domain/artifact";
import type { ToolProgress } from "../hooks/useChat";

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
  latestRunMetrics: ChatRunSummary | null;
  toolProgress?: ToolProgress | null;
  error: string | null;
  backendAvailable: boolean;
  input: string;
  onInputChange: (v: string) => void;
  onSend: () => void;
  models: ModelConfig[];
  selectedModel: string;
  onModelChange: (model: string) => void;
  thinkingLevel: string;
  onThinkingLevelChange: (level: string) => void;
  artifacts?: ArtifactRecord[];
  onOpenArtifact?: (artifactId: string) => void;
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

const THINKING_OPTIONS = [
  { value: "off", label: "思考·关" },
  { value: "low", label: "思考·低" },
  { value: "medium", label: "思考·中" },
  { value: "high", label: "思考·高" },
] as const;

interface ThinkingSelectorProps {
  value: string;
  onChange: (level: string) => void;
  disabled: boolean;
}

function ThinkingSelector({ value, onChange, disabled }: ThinkingSelectorProps) {
  const [expanded, setExpanded] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!expanded) return;
    const closeOnOutsideClick = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setExpanded(false);
    };
    window.addEventListener("pointerdown", closeOnOutsideClick);
    return () => window.removeEventListener("pointerdown", closeOnOutsideClick);
  }, [expanded]);

  const current =
    THINKING_OPTIONS.find((option) => option.value === value) ?? THINKING_OPTIONS[2];

  return (
    <div
      className={`composer-model-selector ${expanded ? "expanded" : ""}`}
      ref={rootRef}
    >
      <button
        type="button"
        className="composer-model-trigger"
        onClick={() => setExpanded((open) => !open)}
        disabled={disabled}
        title="思考强度"
      >
        <span className="composer-model-mark" aria-hidden="true" />
        <span className="composer-model-current">{current.label}</span>
        <span className="composer-model-chevron" aria-hidden="true">
          ▾
        </span>
      </button>

      <div className="composer-model-panel" role="listbox" aria-label="思考强度">
        <div className="composer-model-panel-label">思考强度</div>
        {THINKING_OPTIONS.map((option) => {
          const active = option.value === value;
          return (
            <button
              type="button"
              key={option.value}
              className={`composer-model-option ${active ? "active" : ""}`}
              role="option"
              aria-selected={active}
              onClick={() => {
                onChange(option.value);
                setExpanded(false);
              }}
            >
              <span className="composer-model-option-copy">
                <strong>{option.label}</strong>
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
  latestRunMetrics,
  toolProgress,
  error,
  backendAvailable,
  input,
  onInputChange,
  onSend,
  models,
  selectedModel,
  onModelChange,
  thinkingLevel,
  onThinkingLevelChange,
  onApprove,
  onReject,
  onViewEvidence,
  artifacts = [],
  onOpenArtifact,
}: ResearchExecutionThreadProps) {
  const bottomRef = useRef<HTMLDivElement>(null);
  const [elapsedSeconds, setElapsedSeconds] = useState(0);

  useEffect(() => {
    if (!sending) {
      setElapsedSeconds(0);
      return;
    }
    const startedAt = Date.now();
    const update = () =>
      setElapsedSeconds(Math.floor((Date.now() - startedAt) / 1000));
    update();
    const timer = window.setInterval(update, 1000);
    return () => window.clearInterval(timer);
  }, [sending]);

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
    ? latestRunMetrics.cacheReadInputTokens +
      latestRunMetrics.cacheCreationInputTokens
    : 0;
  const cacheHitRate =
    latestRunMetrics && cacheTotal > 0
      ? Math.round(
          (latestRunMetrics.cacheReadInputTokens / cacheTotal) * 100,
        )
      : null;

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
              <ArtifactMarkdown onOpenArtifact={onOpenArtifact}>
                {block.text}
              </ArtifactMarkdown>
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
              <ArtifactMarkdown onOpenArtifact={onOpenArtifact}>
                {block.text}
              </ArtifactMarkdown>
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
              <ArtifactMarkdown onOpenArtifact={onOpenArtifact}>
                {block.text}
              </ArtifactMarkdown>
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
              <ArtifactMarkdown onOpenArtifact={onOpenArtifact}>
                {block.text}
              </ArtifactMarkdown>
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
              <ArtifactMarkdown onOpenArtifact={onOpenArtifact}>
                {block.text}
              </ArtifactMarkdown>
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
              <ArtifactMarkdown onOpenArtifact={onOpenArtifact}>
                {block.text}
              </ArtifactMarkdown>
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
        <div>
          <span className="thread-header-kicker">RESEARCH BRIEF</span>
          <h2>研究委托</h2>
        </div>
        <StatusDot tone={sending ? "active" : "idle"}>
          {activityLabel}
        </StatusDot>
      </div>

      {sending && toolProgress && (
        <div className="thread-run-metrics" aria-live="polite">
          <span>执行中</span>
          <strong>{toolProgress.tool}</strong>
          <span>{toolProgress.phase === "running" ? "运行中" : toolProgress.phase === "failed" ? "失败，正在调整" : "已完成"}</span>
          <span>步骤 {toolProgress.turn}/{toolProgress.maxTurns}</span>
        </div>
      )}

      {!sending && latestRunMetrics && (
        <div className="thread-run-metrics" aria-label="上一轮模型性能">
          <span>上一轮</span>
          <strong>{(latestRunMetrics.totalMs / 1000).toFixed(1)}s</strong>
          <span>首个可见响应</span>
          <strong>
            {latestRunMetrics.ttftMs == null
              ? "—"
              : `${(latestRunMetrics.ttftMs / 1000).toFixed(1)}s`}
          </strong>
          <span>Token</span>
          <strong>
            {latestRunMetrics.inputTokens.toLocaleString()} →{" "}
            {latestRunMetrics.outputTokens.toLocaleString()}
          </strong>
          <span>缓存命中</span>
          <strong>{cacheHitRate == null ? "—" : `${cacheHitRate}%`}</strong>
          {latestRunMetrics.toolCallCount > 0 && (
            <>
              <span>工具</span>
              <strong>{latestRunMetrics.toolCallCount} 次</strong>
            </>
          )}
        </div>
      )}

      {/* ── Thread messages ── */}
      <div className="thread-messages">
        {messages.length === 0 && !thinking && (
          <div className="thread-empty">
            <span className="thread-empty-index">01</span>
            <p className="thread-empty-title">今天要推进哪项研究？</p>
            <p className="thread-empty-desc">
              给出研究问题、现有资料和希望得到的成果。Galen 会建立任务契约，沿证据脉络持续执行，并在成果可验证后交付。
            </p>
            <div className="thread-starter-list">
              <button type="button" onClick={() => onInputChange("基于当前工作区的数据，提出一个可验证的康复科研问题，并生成研究方案。")}>从工作区数据开始</button>
              <button type="button" onClick={() => onInputChange("围绕一个康复临床问题检索证据，形成带引用的证据摘要。")}>从临床问题开始</button>
              <button type="button" onClick={() => onInputChange("检查现有研究产物的证据、方法和交付完整性，并列出需要修复的问题。")}>检查现有研究</button>
            </div>
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

        {artifacts.length > 0 && (
          <div className="thread-block thread-block-artifacts" aria-label="本次研究产物">
            <div className="thread-block-header"><Tag type="evidence">产物已生成</Tag></div>
            <div className="thread-block-body artifact-delivery-links">
              {[...artifacts]
                .sort((left, right) => right.createdAt.localeCompare(left.createdAt))
                .slice(0, 5)
                .map((artifact) => (
                  <ArtifactMarkdown key={artifact.id} onOpenArtifact={onOpenArtifact}>
                    {`[预览 ${artifact.path.split(/[/\\]/).pop() ?? artifact.path}](${artifactHref(artifact.id)})`}
                  </ArtifactMarkdown>
                ))}
            </div>
          </div>
        )}

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
            placeholder="描述研究问题、已有资料和期望成果…"
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
            <ThinkingSelector
              value={thinkingLevel}
              onChange={onThinkingLevelChange}
              disabled={!backendAvailable || sending}
            />
            <span className="thread-composer-hint">Enter 发送 · Shift+Enter 换行</span>
            <TokenRing messages={messages} />
            <button
              className="btn btn-primary thread-send-button"
              onClick={onSend}
              disabled={!backendAvailable || sending || !input.trim()}
            >
              开始推进
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
