import { useState } from "react";
import type { SessionNode } from "../domain/sessionTypes";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------
interface ResearchPlanCanvasProps {
  nodes: SessionNode[];
  planConfirmed: boolean;
  pendingPlan?: SessionNode[] | null;
  onConfirmPlan?: () => void;
  onSelectNode?: (node: SessionNode) => void;
  selectedNodeId?: string | null;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------
export function ResearchPlanCanvas({
  nodes,
  planConfirmed,
  pendingPlan,
  onConfirmPlan,
  onSelectNode,
  selectedNodeId,
}: ResearchPlanCanvasProps) {
  if (!planConfirmed) {
    // Discussion phase — canvas is blank.  The plan is discussed in chat.
    // User clicks "确认计划" in chat to activate the canvas.
    return (
      <div className="plan-canvas plan-canvas-empty">
        <div className="plan-canvas-empty-inner">
          <div className="plan-canvas-empty-icon">◻</div>
          <h3>科研计划画布</h3>
          <p>在主线程中与 AI 讨论研究计划，确认后将在此生成可执行的 Session 节点。</p>
          {pendingPlan && pendingPlan.length > 0 && (
            <button className="btn btn-primary" onClick={onConfirmPlan} style={{ marginTop: "var(--space-4)" }}>
              确认计划（{pendingPlan.length} 个节点）
            </button>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="plan-canvas">
      {/* Canvas header */}
      <div className="plan-canvas-header">
        <span className="plan-canvas-title">科研计划画布</span>
        <span className="plan-canvas-status">
          计划已确认 · {nodes.filter((n) => n.status === "completed").length}/{nodes.length} 完成
        </span>
      </div>

      {/* Nodes */}
      <div className="plan-canvas-nodes">
        {nodes.map((node, i) => (
          <div key={node.id} style={{ position: "relative" }}>
            {/* Connector line to next node */}
            {i < nodes.length - 1 && (
              <div className="plan-canvas-connector">
                <div className="plan-canvas-connector-line" />
                <span className="plan-canvas-connector-label">
                  {node.outputs?.[0] ?? "→"}
                </span>
              </div>
            )}

            {/* Node card */}
            <button
              className={`plan-node ${selectedNodeId === node.id ? "plan-node-selected" : ""}`}
              onClick={() => onSelectNode?.(node)}
              type="button"
            >
              <div className="plan-node-header">
                <span className="plan-node-index">{node.index}</span>
                <span className={`plan-node-status plan-node-status-${node.status}`}>
                  {statusLabel(node.status)}
                </span>
              </div>
              <div className="plan-node-title">{node.title}</div>
              {node.status === "completed" && node.result && (
                <div className="plan-node-result">
                  {node.result.length > 120 ? `${node.result.slice(0, 120)}…` : node.result}
                </div>
              )}
              {node.evidence && node.evidence.length > 0 && (
                <div className="plan-node-evidence">证据 {node.evidence.length} 条</div>
              )}
              <div className="plan-node-meta">
                {node.owner && <span>负责人: {node.owner}</span>}
                {node.riskLevel && (
                  <span className={`plan-node-risk plan-node-risk-${node.riskLevel}`}>
                    风险: {riskLabel(node.riskLevel)}
                  </span>
                )}
              </div>
              {node.dependsOn && node.dependsOn.length > 0 && (
                <div className="plan-node-deps">
                  等待: {node.dependsOn.join(", ")}
                </div>
              )}
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function statusLabel(s: string): string {
  const map: Record<string, string> = {
    pending: "待执行",
    pending_approval: "待批准",
    approved: "已批准",
    assigned: "已分派",
    running: "运行中",
    blocked: "已阻塞",
    completed: "已完成",
    returned: "已回流",
  };
  return map[s] ?? s;
}

function riskLabel(r: string): string {
  const map: Record<string, string> = {
    low: "低",
    medium: "中",
    high: "高",
  };
  return map[r] ?? r;
}

export { type SessionNode };
