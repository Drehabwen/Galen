import { useEffect, useRef, useState } from "react";
import type { ChatMessage, ModelConfig } from "../types";
import { TokenRing } from "./TokenRing";

const THINKING_OPTIONS = [
  { value: "off", label: "思考·关" },
  { value: "low", label: "思考·低" },
  { value: "medium", label: "思考·中" },
  { value: "high", label: "思考·高" },
] as const;

interface SelectorProps {
  value: string;
  onChange: (value: string) => void;
  disabled: boolean;
}

interface ModelSelectorProps extends SelectorProps {
  models: ModelConfig[];
}

function useCloseOnOutsideClick(
  expanded: boolean,
  setExpanded: (expanded: boolean) => void,
) {
  const rootRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!expanded) return;
    const close = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setExpanded(false);
    };
    window.addEventListener("pointerdown", close);
    return () => window.removeEventListener("pointerdown", close);
  }, [expanded, setExpanded]);
  return rootRef;
}

function ThinkingSelector({ value, onChange, disabled }: SelectorProps) {
  const [expanded, setExpanded] = useState(false);
  const rootRef = useCloseOnOutsideClick(expanded, setExpanded);
  const current =
    THINKING_OPTIONS.find((option) => option.value === value) ?? THINKING_OPTIONS[2];

  return (
    <div className={`composer-model-selector ${expanded ? "expanded" : ""}`} ref={rootRef}>
      <button
        type="button"
        className="composer-model-trigger"
        onClick={() => setExpanded((open) => !open)}
        disabled={disabled}
        title="思考强度"
      >
        <span className="composer-model-mark" aria-hidden="true" />
        <span className="composer-model-current">{current.label}</span>
        <span className="composer-model-chevron" aria-hidden="true">▾</span>
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
              <span className="composer-model-option-copy"><strong>{option.label}</strong></span>
              <span className="composer-model-check" aria-hidden="true">{active ? "✓" : ""}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

function ModelSelector({ models, value, onChange, disabled }: ModelSelectorProps) {
  const [expanded, setExpanded] = useState(false);
  const rootRef = useCloseOnOutsideClick(expanded, setExpanded);
  const current = models.find((item) => item.name === value);

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
              <span className="composer-model-check" aria-hidden="true">{active ? "✓" : ""}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

interface ResearchThreadComposerProps {
  input: string;
  onInputChange: (value: string) => void;
  onSend: () => void;
  messages: ChatMessage[];
  models: ModelConfig[];
  selectedModel: string;
  onModelChange: (model: string) => void;
  thinkingLevel: string;
  onThinkingLevelChange: (level: string) => void;
  disabled: boolean;
  sending: boolean;
}

export function ResearchThreadComposer({
  input,
  onInputChange,
  onSend,
  messages,
  models,
  selectedModel,
  onModelChange,
  thinkingLevel,
  onThinkingLevelChange,
  disabled,
  sending,
}: ResearchThreadComposerProps) {
  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      if (input.trim() && !sending) onSend();
    }
  };

  return (
    <div className="thread-input-area">
      <div className="thread-composer">
        <textarea
          className="thread-input"
          placeholder="描述研究问题、已有资料和期望成果…"
          value={input}
          onChange={(event) => onInputChange(event.target.value)}
          onKeyDown={handleKeyDown}
          rows={2}
          disabled={disabled || sending}
        />
        <div className="thread-composer-toolbar">
          <ModelSelector models={models} value={selectedModel} onChange={onModelChange} disabled={disabled || sending} />
          <ThinkingSelector value={thinkingLevel} onChange={onThinkingLevelChange} disabled={disabled || sending} />
          <span className="thread-composer-hint">Enter 发送 · Shift+Enter 换行</span>
          <TokenRing messages={messages} />
          <button
            className="btn btn-primary thread-send-button"
            onClick={onSend}
            disabled={disabled || sending || !input.trim()}
          >
            开始推进
          </button>
        </div>
      </div>
    </div>
  );
}
