import { ArtifactMarkdown, artifactHref } from "./ArtifactMarkdown";
import { StatusDot } from "./ui/primitives";
import { useSessionChat } from "../hooks/useSessionChat";
import type { SessionNode } from "../domain/sessionTypes";
import type { ArtifactRecord } from "../domain/artifact";
import type { VerifiableSource } from "./SourceInspector";

interface SessionChatProps {
  node: SessionNode;
  onClose: () => void;
  onFlowBack?: (node: SessionNode, summary: string) => void;
  backendAvailable: boolean;
  modelAlias: string;
  thinkingLevel?: string;
  autoRun?: boolean;
  artifacts?: ArtifactRecord[];
  onOpenArtifact?: (artifactId: string) => void;
  onOpenSource?: (source: VerifiableSource) => void;
}

export function SessionChat({
  node,
  onClose,
  onFlowBack,
  backendAvailable,
  modelAlias,
  thinkingLevel,
  autoRun,
  artifacts = [],
  onOpenArtifact,
  onOpenSource,
}: SessionChatProps) {
  const chat = useSessionChat({ node, backendAvailable, modelAlias, thinkingLevel, autoRun, onFlowBack });
  const handleSend = () => void chat.sendText(chat.input);
  const nodeArtifacts = artifacts.filter((artifact) => artifact.nodeId === node.id);

  return (
    <div className="session-chat">
      <div className="session-chat-header">
        <span className="session-chat-id">{node.index} · {node.title}</span>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <StatusDot tone={chat.sending ? "active" : "idle"}>{chat.sending ? "运行中" : "就绪"}</StatusDot>
          {chat.messages.length > 0 && (
            <button className="btn btn-primary btn-sm" onClick={chat.flowBack}>回流主线程</button>
          )}
          <button className="btn btn-ghost btn-sm" onClick={onClose}>✕</button>
        </div>
      </div>

      <div className="session-chat-context">
        <div className="session-chat-context-title">会话上下文</div>
        <div className="session-chat-context-body">
          <div><strong>任务：</strong>{node.title}</div>
          {node.description && <div><strong>描述：</strong>{node.description}</div>}
          {node.inputs?.length ? <div><strong>输入：</strong>{node.inputs.join(", ")}</div> : null}
        </div>
      </div>

      <div className="session-chat-messages">
        {chat.messages.length === 0 && !chat.thinking && (
          <div className="session-chat-empty">
            <p>开始执行「{node.title}」</p>
            <p style={{ fontSize: "var(--text-xs)", color: "var(--text-tertiary)" }}>描述你的需求或直接说"开始执行"</p>
          </div>
        )}
        {chat.messages.map((message, index) => (
          <div key={index} className={`session-msg session-msg-${message.role}`}>
            <div className="session-msg-role">{message.role === "user" ? "你" : "Galen"}</div>
            <div className="session-msg-body">
              <ArtifactMarkdown onOpenArtifact={onOpenArtifact} onOpenSource={onOpenSource}>{message.content}</ArtifactMarkdown>
            </div>
          </div>
        ))}
        {nodeArtifacts.map((artifact) => (
          <div key={artifact.id} className="session-msg session-msg-assistant session-artifact-delivery">
            <div className="session-msg-role">产物已生成</div>
            <div className="session-msg-body">
              <ArtifactMarkdown onOpenArtifact={onOpenArtifact} onOpenSource={onOpenSource}>
                {`[预览 ${artifact.path.split(/[/\\]/).pop() ?? artifact.path}](${artifactHref(artifact.id)})`}
              </ArtifactMarkdown>
            </div>
          </div>
        ))}
        {chat.thinking && (
          <div className="session-msg session-msg-assistant">
            <div className="session-msg-role">思考中</div>
            <div className="session-msg-body session-msg-thinking">{chat.thinking}</div>
          </div>
        )}
        {chat.streaming && (
          <div className="session-msg session-msg-assistant">
            <div className="session-msg-role">Galen</div>
            <div className="session-msg-body"><ArtifactMarkdown>{chat.streaming}</ArtifactMarkdown></div>
          </div>
        )}
        {chat.error && (
          <div className="session-msg session-msg-error">
            <div className="session-msg-role">错误</div>
            <div className="session-msg-body">{chat.error}</div>
          </div>
        )}
        <div ref={chat.bottomRef} />
      </div>

      <div className="session-chat-input-area">
        <textarea
          className="session-chat-input"
          placeholder={`在「${node.title}」中描述你的需求...`}
          value={chat.input}
          onChange={(event) => chat.setInput(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              handleSend();
            }
          }}
          rows={2}
          disabled={!backendAvailable || chat.sending}
        />
        <button className="btn btn-primary btn-sm" onClick={handleSend} disabled={!backendAvailable || chat.sending || !chat.input.trim()}>
          发送
        </button>
      </div>
    </div>
  );
}
