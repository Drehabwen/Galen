import { useEffect, useRef } from "react";
import type { ArtifactRecord } from "../domain/artifact";
import type { ChatMessage } from "../types";
import { ArtifactMarkdown, artifactHref } from "./ArtifactMarkdown";
import type { VerifiableSource } from "./SourceInspector";
import { ApprovalCard, StatusDot, Tag } from "./ui/primitives";

type ThreadBlock =
  | { kind: "user-request"; text: string }
  | { kind: "ai-plan"; text: string }
  | { kind: "tool-execution"; text: string }
  | { kind: "revision-suggestion"; text: string }
  | { kind: "evidence-link"; text: string }
  | { kind: "approval-needed"; text: string }
  | { kind: "generic-text"; text: string };

function classifyBlock(text: string): ThreadBlock {
  if (text.startsWith("## 执行计划") || text.includes("执行计划")) return { kind: "ai-plan", text };
  if (text.startsWith("[工具调用]") || text.includes("工具调用")) return { kind: "tool-execution", text };
  if (text.includes("修订建议") || text.includes("建议修改")) return { kind: "revision-suggestion", text };
  if (text.includes("证据") && (text.includes("PMID") || text.includes("引用"))) return { kind: "evidence-link", text };
  if (text.includes("待签核") || text.includes("请确认")) return { kind: "approval-needed", text };
  return { kind: "generic-text", text };
}

function splitIntoBlocks(text: string): ThreadBlock[] {
  return text
    .split(/\n\n+/)
    .map((segment) => segment.trim())
    .filter(Boolean)
    .map(classifyBlock);
}

interface BlockRendererProps {
  block: ThreadBlock;
  messageIndex: number;
  onApprove?: (messageId: number) => void;
  onReject?: (messageId: number) => void;
  onViewEvidence?: (messageId: number) => void;
  onOpenArtifact?: (artifactId: string) => void;
  onOpenSource?: (source: VerifiableSource) => void;
}

function BlockRenderer({
  block,
  messageIndex,
  onApprove,
  onReject,
  onViewEvidence,
  onOpenArtifact,
  onOpenSource,
}: BlockRendererProps) {
  const body = (
    <ArtifactMarkdown onOpenArtifact={onOpenArtifact} onOpenSource={onOpenSource}>
      {block.text}
    </ArtifactMarkdown>
  );

  switch (block.kind) {
    case "user-request":
      return <div className="thread-block thread-block-user"><div className="thread-block-header"><span className="thread-block-role">研究者</span></div><div className="thread-block-body">{body}</div></div>;
    case "ai-plan":
      return <div className="thread-block thread-block-plan"><div className="thread-block-header"><StatusDot tone="active">AI 执行计划</StatusDot></div><div className="thread-block-body">{body}</div></div>;
    case "tool-execution":
      return <div className="thread-block thread-block-tool"><div className="thread-block-header"><Tag type="execution">工具执行</Tag></div><div className="thread-block-body">{body}</div></div>;
    case "revision-suggestion":
      return (
        <div className="thread-block thread-block-revision">
          <div className="thread-block-header"><Tag type="status">修订建议</Tag></div>
          <div className="thread-block-body">{body}</div>
          <div className="thread-block-actions">
            <button className="btn btn-primary btn-sm" onClick={() => onApprove?.(messageIndex)}>接受修订</button>
            <button className="btn btn-ghost btn-sm" onClick={() => onReject?.(messageIndex)}>要求修订</button>
          </div>
        </div>
      );
    case "evidence-link":
      return (
        <div className="thread-block thread-block-evidence">
          <div className="thread-block-header">
            <Tag type="evidence">证据链已关联</Tag>
            <button className="btn btn-ghost btn-sm" onClick={() => onViewEvidence?.(messageIndex)}>查看依据</button>
          </div>
          <div className="thread-block-body">{body}</div>
        </div>
      );
    case "approval-needed":
      return (
        <ApprovalCard
          reason={block.text.slice(0, 200)}
          source="执行计划自动生成"
          impact="此操作将修改文档内容"
          onApprove={() => onApprove?.(messageIndex)}
          onViewEvidence={() => onViewEvidence?.(messageIndex)}
          onReject={() => onReject?.(messageIndex)}
        />
      );
    default:
      return <div className="thread-block thread-block-text"><div className="thread-block-header"><span className="thread-block-role">Galen</span></div><div className="thread-block-body">{body}</div></div>;
  }
}

interface ResearchThreadMessagesProps {
  messages: ChatMessage[];
  thinking: string;
  streaming: string;
  error: string | null;
  artifacts: ArtifactRecord[];
  onInputChange: (value: string) => void;
  onApprove?: (messageId: number) => void;
  onReject?: (messageId: number) => void;
  onViewEvidence?: (messageId: number) => void;
  onOpenArtifact?: (artifactId: string) => void;
  onOpenSource?: (source: VerifiableSource) => void;
  children?: React.ReactNode;
}

export function ResearchThreadMessages({
  messages,
  thinking,
  streaming,
  error,
  artifacts,
  onInputChange,
  onApprove,
  onReject,
  onViewEvidence,
  onOpenArtifact,
  onOpenSource,
  children,
}: ResearchThreadMessagesProps) {
  const bottomRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, thinking, streaming, error]);

  return (
    <div className="thread-messages">
      {messages.length === 0 && !thinking && (
        <div className="thread-empty">
          <span className="thread-empty-index">01</span>
          <p className="thread-empty-title">今天要推进哪项研究？</p>
          <p className="thread-empty-desc">给出研究问题、现有资料和希望得到的成果。Galen 会建立任务契约，沿证据脉络持续执行，并在成果可验证后交付。</p>
          <div className="thread-starter-list">
            <button type="button" onClick={() => onInputChange("基于当前工作区的数据，提出一个可验证的康复科研问题，并生成研究方案。")}>从工作区数据开始</button>
            <button type="button" onClick={() => onInputChange("围绕一个康复临床问题检索证据，形成带引用的证据摘要。")}>从临床问题开始</button>
            <button type="button" onClick={() => onInputChange("检查现有研究产物的证据、方法和交付完整性，并列出需要修复的问题。")}>检查现有研究</button>
          </div>
        </div>
      )}

      {messages.map((message, messageIndex) => {
        const blocks = message.role === "user"
          ? [{ kind: "user-request", text: message.content } as ThreadBlock]
          : splitIntoBlocks(message.content);
        return (
          <div key={`${message.timestamp}-${messageIndex}`}>
            {blocks.map((block, blockIndex) => (
              <BlockRenderer
                key={`${messageIndex}-${blockIndex}`}
                block={block}
                messageIndex={messageIndex}
                onApprove={onApprove}
                onReject={onReject}
                onViewEvidence={onViewEvidence}
                onOpenArtifact={onOpenArtifact}
                onOpenSource={onOpenSource}
              />
            ))}
          </div>
        );
      })}

      {children}

      {artifacts.length > 0 && (
        <div className="thread-block thread-block-artifacts" aria-label="本次研究产物">
          <div className="thread-block-header"><Tag type="evidence">产物已生成</Tag></div>
          <div className="thread-block-body artifact-delivery-links">
            {[...artifacts]
              .sort((left, right) => right.createdAt.localeCompare(left.createdAt))
              .slice(0, 5)
              .map((artifact) => (
                <ArtifactMarkdown key={artifact.id} onOpenArtifact={onOpenArtifact} onOpenSource={onOpenSource}>
                  {`[预览 ${artifact.path.split(/[/\\]/).pop() ?? artifact.path}](${artifactHref(artifact.id)})`}
                </ArtifactMarkdown>
              ))}
          </div>
        </div>
      )}

      {thinking && <div className="thread-block thread-block-thinking"><div className="thread-block-header"><StatusDot tone="idle">推理中</StatusDot></div><div className="thread-block-body thread-thinking-content">{thinking}</div></div>}
      {streaming.length > 0 && !thinking && <div className="thread-streaming-dot"><span className="thinking-dot" />Galen 正在回复...</div>}
      {error && <div className="thread-block thread-block-error"><div className="thread-block-header"><StatusDot tone="error">错误</StatusDot></div><div className="thread-block-body">{error}</div></div>}
      <div ref={bottomRef} />
    </div>
  );
}
