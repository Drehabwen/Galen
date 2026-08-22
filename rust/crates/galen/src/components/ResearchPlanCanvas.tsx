import type { SessionNode } from "../domain/sessionTypes";

interface ResearchPlanCanvasProps {
  nodes: SessionNode[];
  planConfirmed: boolean;
  pendingPlan?: SessionNode[] | null;
  onConfirmPlan?: () => void;
  onSelectNode?: (node: SessionNode) => void;
  onPreviewArtifact?: (path: string, node: SessionNode) => void;
  selectedNodeId?: string | null;
}

const PREVIEWABLE = /\.(md|mdx|txt|json|csv|tsv|typ|tex|py|r|js|ts)$/i;

export function ResearchPlanCanvas({
  nodes, planConfirmed, pendingPlan, onConfirmPlan, onSelectNode,
  onPreviewArtifact,
  selectedNodeId,
}: ResearchPlanCanvasProps) {
  if (!planConfirmed) {
    return (
      <div className="plan-canvas plan-canvas-empty">
        <div className="plan-canvas-empty-inner">
          <div className="empty-evidence-trace" aria-hidden="true">
            <span /><span /><span />
          </div>
          <span className="plan-canvas-kicker">EVIDENCE BEGINS WITH A QUESTION</span>
          <h3>从一个可验证的问题开始</h3>
          <p>描述研究对象、已有资料与期望产物。Galen 会把任务拆成可追踪的证据节点，并保留每一步的来源与结果。</p>
          {pendingPlan && pendingPlan.length > 0 && (
            <button className="btn btn-primary" onClick={onConfirmPlan}>开始执行 {pendingPlan.length} 个节点</button>
          )}
        </div>
      </div>
    );
  }

  const completed = nodes.filter((node) => node.status === "completed").length;
  const evidenceCount = nodes.reduce((count, node) => count + (node.evidence?.length ?? 0), 0);
  const artifactCount = nodes.reduce(
    (count, node) => count + (node.outputs ?? []).filter((item) => PREVIEWABLE.test(item)).length,
    0,
  );
  const allCompleted = nodes.length > 0 && completed === nodes.length;
  return (
    <div className="plan-canvas">
      <div className="plan-canvas-header">
        <div className="plan-canvas-heading">
          <span className="plan-canvas-kicker">RESEARCH EVIDENCE FLOW</span>
          <div className="plan-canvas-title">证据脉络</div>
        </div>
        <div className="evidence-ledger" aria-label="研究任务状态">
          <span><strong>{completed}/{nodes.length}</strong> 节点</span>
          <span><strong>{evidenceCount}</strong> 证据</span>
          <span><strong>{artifactCount}</strong> 产物</span>
          <span className={allCompleted ? "ready" : "waiting"}>
            <i aria-hidden="true" />{allCompleted ? "可交付" : "验证中"}
          </span>
        </div>
      </div>

      <div className="plan-canvas-nodes">
        {nodes.map((node, i) => {
          const artifacts = (node.outputs ?? []).filter((item) => PREVIEWABLE.test(item));
          const edgeState = node.status === "completed" ? "complete" : node.status === "running" ? "active" : "idle";
          return (
            <div className="evidence-flow-step" key={node.id}>
              <button
                className={`plan-node plan-node-${node.status} ${selectedNodeId === node.id ? "plan-node-selected" : ""}`}
                onClick={() => onSelectNode?.(node)} type="button"
              >
                <span className="plan-node-orbit" aria-hidden="true" />
                <div className="plan-node-header">
                  <span className="plan-node-index">{String(i + 1).padStart(2, "0")}</span>
                  <span className={`plan-node-status plan-node-status-${node.status}`}>{statusLabel(node.status)}</span>
                </div>
                <div className="plan-node-title">{node.title}</div>
                {node.result && <div className="plan-node-result">{node.result.slice(0, 150)}{node.result.length > 150 ? "…" : ""}</div>}
                <div className="plan-node-meta">
                  {node.evidence?.length ? <span>证据 {node.evidence.length} 条</span> : <span>等待证据回流</span>}
                  {node.dependsOn?.length ? <span>依赖 {node.dependsOn.join(" · ")}</span> : <span>起始节点</span>}
                </div>
                {artifacts.length > 0 && (
                  <div className="plan-node-artifacts">
                    {artifacts.map((path) => (
                      <span key={path} className="artifact-preview-button" role="button" tabIndex={0}
                        onClick={(event) => { event.stopPropagation(); onPreviewArtifact?.(path, node); }}
                        onKeyDown={(event) => { if (event.key === "Enter") { event.stopPropagation(); onPreviewArtifact?.(path, node); } }}>
                        预览 {path.split(/[\\/]/).pop()}
                      </span>
                    ))}
                  </div>
                )}
              </button>
              {i < nodes.length - 1 && (
                <div className={`evidence-connector evidence-connector-${edgeState}`}>
                  <span className="evidence-pulse" />
                  <span className="evidence-connector-label">{node.outputs?.[0] || "证据回流"}</span>
                </div>
              )}
            </div>
          );
        })}

        <section className={`delivery-gate ${allCompleted ? "delivery-gate-ready" : ""}`}>
          <div className="delivery-gate-mark">{allCompleted ? "✓" : "◇"}</div>
          <div className="delivery-gate-copy">
            <span className="delivery-gate-eyebrow">DELIVERY READINESS</span>
            <strong>{allCompleted ? "证据齐备，正在形成成果" : "交付闸门等待证据"}</strong>
            <p>{allCompleted
              ? "Galen 将验证引用与产物，并在成果预览中呈现最终报告。"
              : `还有 ${nodes.length - completed} 个节点需要完成；模型不能跳过证据直接宣布完成。`}</p>
          </div>
        </section>
      </div>
    </div>
  );
}

function statusLabel(status: string): string {
  return ({ pending: "待执行", pending_approval: "待批准", approved: "已批准", assigned: "已分派", running: "运行中", blocked: "已阻塞", completed: "已回流", returned: "已退回" } as Record<string, string>)[status] ?? status;
}

export { type SessionNode };
