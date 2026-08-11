import { useState } from "react";
import { Tag, StatusDot, ProgressBar } from "./ui/primitives";
import type { SessionNode } from "../domain/sessionTypes";
import { MOCK_NODES } from "../domain/sessionTypes";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------
interface SessionInspectorDrawerProps {
  node: SessionNode;
  onClose: () => void;
  onEnterSession?: (node: SessionNode) => void;
  onApprove?: (node: SessionNode) => void;
  onAssign?: (node: SessionNode) => void;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------
export function SessionInspectorDrawer({
  node,
  onClose,
  onEnterSession,
  onApprove,
  onAssign,
}: SessionInspectorDrawerProps) {
  const [enteredSub, setEnteredSub] = useState<SessionNode | null>(null);

  // If a sub-session is entered, show its detail view
  if (enteredSub) {
    return (
      <div className="session-drawer">
        <SessionSubView
          parent={node}
          sub={enteredSub}
          onBack={() => setEnteredSub(null)}
          onClose={onClose}
        />
      </div>
    );
  }

  return (
    <div className="session-drawer">
      {/* Header */}
      <div className="session-drawer-header">
        <div>
          <span className="session-drawer-id">{node.index} · {node.title}</span>
          <span className={`plan-node-status plan-node-status-${node.status}`}>
            {statusLabel(node.status)}
          </span>
        </div>
        <button className="btn btn-ghost btn-sm" onClick={onClose}>✕</button>
      </div>

      {/* Body: two-column */}
      <div className="session-drawer-body">
        {/* Left: Context & Inputs */}
        <div className="session-drawer-left">
          <Section title="目标">
            <p className="session-meta-text">
              {goalForNode(node)}
            </p>
          </Section>

          <Section title="输入材料">
            {node.inputs?.length ? (
              <ul className="session-list">
                {node.inputs.map((inp, i) => (
                  <li key={i}>{inp}</li>
                ))}
              </ul>
            ) : (
              <p className="session-empty">无外部输入</p>
            )}
          </Section>

          <Section title="将执行">
            <p className="session-meta-text">{execForNode(node)}</p>
          </Section>

          {node.riskLevel && (
            <Section title="风险提示">
              <Tag type={node.riskLevel === "high" ? "risk" : "phase"}>
                {riskLabel(node.riskLevel)}风险
              </Tag>
              <p className="session-meta-text" style={{ marginTop: 4 }}>
                {riskNote(node)}
              </p>
            </Section>
          )}

          <Section title="回流产物">
            {node.outputs?.length ? (
              <ul className="session-list">
                {node.outputs.map((out, i) => (
                  <li key={i}>{out}</li>
                ))}
              </ul>
            ) : (
              <p className="session-empty">暂无</p>
            )}
          </Section>
        </div>

        {/* Right: Execution */}
        <div className="session-drawer-right">
          <Section title="负责人">
            <span className="session-meta-text">
              {node.owner ?? "未分配"}
            </span>
          </Section>

          <Section title="子 Session">
            {node.subSessions?.length ? (
              <div className="session-subs">
                {node.subSessions.map((sub) => (
                  <button
                    key={sub.id}
                    className="session-sub-item"
                    onClick={() => setEnteredSub(sub)}
                    type="button"
                  >
                    <span className="session-sub-index">{sub.index}</span>
                    <span className="session-sub-title">{sub.title}</span>
                    <span className={`plan-node-status plan-node-status-${sub.status}`}>
                      {statusLabel(sub.status)}
                    </span>
                  </button>
                ))}
              </div>
            ) : (
              <p className="session-empty">无子 Session</p>
            )}
          </Section>

          <Section title="代码">
            <p className="session-empty">进入 Session 后查看</p>
          </Section>

          <Section title="日志">
            <p className="session-empty">进入 Session 后查看</p>
          </Section>
        </div>
      </div>

      {/* Action bar */}
      <div className="session-drawer-actions">
        {node.approvalRequired && node.status === "pending_approval" && (
          <button className="btn btn-primary" onClick={() => onApprove?.(node)}>
            批准放行
          </button>
        )}
        <button className="btn btn-ghost" onClick={() => onEnterSession?.(node)}>
          进入 Session
        </button>
        <button className="btn btn-ghost" onClick={() => onAssign?.(node)}>
          分派他人
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-Session detail view
// ---------------------------------------------------------------------------
function SessionSubView({
  parent,
  sub,
  onBack,
  onClose,
}: {
  parent: SessionNode;
  sub: SessionNode;
  onBack: () => void;
  onClose: () => void;
}) {
  return (
    <>
      <div className="session-drawer-header">
        <div>
          <button className="btn btn-ghost btn-sm" onClick={onBack} style={{ marginRight: 8 }}>
            ← 返回 {parent.index}
          </button>
          <span className="session-drawer-id">{sub.index} · {sub.title}</span>
          <span className={`plan-node-status plan-node-status-${sub.status}`}>
            {statusLabel(sub.status)}
          </span>
        </div>
        <button className="btn btn-ghost btn-sm" onClick={onClose}>✕</button>
      </div>

      <div className="session-drawer-body">
        <div className="session-drawer-left">
          <Section title="目标">
            <p className="session-meta-text">{goalForNode(sub)}</p>
          </Section>
          <Section title="输入">
            {sub.inputs?.length ? (
              <ul className="session-list">
                {sub.inputs.map((inp, i) => (
                  <li key={i}>{inp} (来自 {parent.index})</li>
                ))}
              </ul>
            ) : (
              <p className="session-empty">继承父 Session 输入</p>
            )}
          </Section>
          <Section title="回流产物">
            {sub.outputs?.length ? (
              <ul className="session-list">
                {sub.outputs.map((out, i) => (
                  <li key={i}>{out}</li>
                ))}
              </ul>
            ) : (
              <p className="session-empty">暂无</p>
            )}
          </Section>
        </div>
        <div className="session-drawer-right">
          <Section title="负责人">
            <span className="session-meta-text">{sub.owner ?? "未分配"}</span>
          </Section>
          <Section title="代码">
            <p className="session-empty">进入 Session 后运行</p>
          </Section>
        </div>
      </div>

      <div className="session-drawer-actions">
        <button className="btn btn-primary btn-sm">批准放行</button>
      </div>
    </>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="session-section">
      <h4 className="session-section-title">{title}</h4>
      {children}
    </div>
  );
}

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
  const map: Record<string, string> = { low: "低", medium: "中", high: "高" };
  return map[r] ?? r;
}

function goalForNode(node: SessionNode): string {
  const goals: Record<string, string> = {
    s01: "明确研究问题、对象、暴露、结局和统计策略。",
    s02: "检索并筛选相关文献证据，形成证据摘要。",
    s03: "从数据源提取队列，执行纳入排除，输出清洁数据集。",
    s04: "清洗缺失值、编码变量、生成分析就绪数据集。",
    s05: "执行描述统计、Cox回归、KM曲线等分析。",
    s06: "撰写方法学、结果、讨论段落，生成图表和论文草稿。",
  };
  return goals[node.id] ?? "执行该节点的科研任务。";
}

function execForNode(node: SessionNode): string {
  const execs: Record<string, string> = {
    s01: "AI 分析研究问题，提出关键假设清单，请求研究者确认。",
    s02: "PubMed检索 + 筛选 + 提取关键证据，生成结构化证据摘要。",
    s03: "执行 SQL / Python 脚本提取队列，执行纳排标准，输出流程表。",
    s04: "Python pandas 清洗脚本：缺失值处理、变量编码、异常值标记。",
    s05: "R/Python 统计分析：基线表、Cox模型、KM曲线、亚组分析。",
    s06: "AI 生成方法学段落、结果解释、图表标注、参考文献格式化。",
  };
  return execs[node.id] ?? "自动执行该节点的科研任务。";
}

function riskNote(node: SessionNode): string {
  if (node.id === "s03") return "样本量可能受限于数据源。建议确认数据完整性后再批准。";
  if (node.id === "s05") return "统计方法选择可能影响结论。建议统计师审核后放行。";
  return "该节点存在一定风险，请在执行前确认输入完整。";
}
