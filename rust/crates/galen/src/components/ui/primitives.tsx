import type { ReactNode, CSSProperties } from "react";

// ---------------------------------------------------------------------------
// Section — panel section with heading, eyebrow, and actions
// ---------------------------------------------------------------------------

interface SectionProps {
  eyebrow?: string;
  title: string;
  children: ReactNode;
  actions?: ReactNode;
  compact?: boolean;
  ariaLabel?: string;
}

export function Section({ eyebrow, title, children, actions, compact, ariaLabel }: SectionProps) {
  return (
    <section aria-label={ariaLabel ?? title}>
      <div className={`section-heading${compact ? " section-heading-compact" : ""}`}>
        <div>
          {eyebrow && <span className="section-eyebrow">{eyebrow}</span>}
          <h2>{title}</h2>
        </div>
        {actions}
      </div>
      {children}
    </section>
  );
}

// ---------------------------------------------------------------------------
// StatusPill — small colored label
// ---------------------------------------------------------------------------

type PillTone = "teal" | "amber" | "neutral";

interface StatusPillProps {
  tone?: PillTone;
  children: ReactNode;
}

export function StatusPill({ tone = "neutral", children }: StatusPillProps) {
  const cls = tone === "neutral" ? "mode-pill" : `mode-pill mode-pill-${tone}`;
  return <span className={cls}>{children}</span>;
}

// ---------------------------------------------------------------------------
// DataRow — clickable row for workflow, dataset, and handoff lists
// ---------------------------------------------------------------------------

interface DataRowProps {
  index?: number;
  label: string;
  detail?: string;
  state?: string;
  stateWarn?: boolean;
  onClick?: () => void;
}

export function DataRow({ index, label, detail, state, stateWarn, onClick }: DataRowProps) {
  return (
    <button className="workflow-row" onClick={onClick} type="button">
      {index != null && <span className="workflow-index">{index}</span>}
      <div>
        <div className="workflow-title-row">
          <strong>{label}</strong>
          {state && (
            <span className={stateWarn ? "workflow-state-warn" : ""}>{state}</span>
          )}
        </div>
        {detail && <p>{detail}</p>}
      </div>
    </button>
  );
}

// ---------------------------------------------------------------------------
// MetricSignal — metric value display in study overview
// ---------------------------------------------------------------------------

interface MetricSignalProps {
  label: string;
  value: string;
  warn?: boolean;
}

export function MetricSignal({ label, value, warn }: MetricSignalProps) {
  return (
    <div className={`study-signal ${warn ? "study-signal-amber" : "study-signal-teal"}`}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

// ---------------------------------------------------------------------------
// EmptyState — placeholder when no data is available
// ---------------------------------------------------------------------------

interface EmptyStateProps {
  message: string;
}

export function EmptyState({ message }: EmptyStateProps) {
  return <div className="workbench-empty">{message}</div>;
}

// ---------------------------------------------------------------------------
// Tag — GitHub-style label pill with semantic color categories
// ---------------------------------------------------------------------------

export type TagType = "phase" | "status" | "evidence" | "risk" | "owner" | "execution";

interface TagProps {
  type?: TagType;
  children: ReactNode;
  active?: boolean;
  onClick?: () => void;
  className?: string;
}

export function Tag({ type = "phase", children, active, onClick, className }: TagProps) {
  const classes = [
    "galen-tag",
    `galen-tag-${type}`,
    active ? "galen-tag-active" : "",
    onClick ? "galen-tag-clickable" : "",
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <span className={classes} onClick={onClick} role={onClick ? "button" : undefined} tabIndex={onClick ? 0 : undefined}>
      {children}
    </span>
  );
}

// ---------------------------------------------------------------------------
// StatusDot — small colored dot with label for AI/execution status
// ---------------------------------------------------------------------------

export type DotTone = "active" | "waiting" | "error" | "idle";

interface StatusDotProps {
  tone?: DotTone;
  children: ReactNode;
}

export function StatusDot({ tone = "idle", children }: StatusDotProps) {
  return (
    <span className={`galen-status-dot galen-status-dot-${tone}`}>
      <span className="galen-status-dot-indicator" />
      <span className="galen-status-dot-label">{children}</span>
    </span>
  );
}

// ---------------------------------------------------------------------------
// ProgressBar — thin brand-tinted progress indicator
// ---------------------------------------------------------------------------

interface ProgressBarProps {
  value: number; // 0–100
  label?: string;
}

export function ProgressBar({ value, label }: ProgressBarProps) {
  const clamped = Math.max(0, Math.min(100, value));
  return (
    <div className="galen-progress" role="progressbar" aria-valuenow={clamped} aria-valuemin={0} aria-valuemax={100}>
      {label && <span className="galen-progress-label">{label}</span>}
      <div className="galen-progress-track">
        <div className="galen-progress-fill" style={{ width: `${clamped}%` } as CSSProperties} />
      </div>
      <span className="galen-progress-pct">{Math.round(clamped)}%</span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// ApprovalCard — researcher sign-off card with actions
// ---------------------------------------------------------------------------

interface ApprovalCardProps {
  reason: string;
  source?: string;
  impact?: string;
  onApprove?: () => void;
  onViewEvidence?: () => void;
  onReject?: () => void;
}

export function ApprovalCard({
  reason,
  source,
  impact,
  onApprove,
  onViewEvidence,
  onReject,
}: ApprovalCardProps) {
  return (
    <div className="galen-approval-card">
      <div className="galen-approval-header">
        <StatusDot tone="waiting">待研究者确认</StatusDot>
      </div>
      <p className="galen-approval-reason">{reason}</p>
      {source && <p className="galen-approval-meta">来源：{source}</p>}
      {impact && <p className="galen-approval-meta">影响：{impact}</p>}
      <div className="galen-approval-actions">
        {onApprove && (
          <button className="btn btn-primary" onClick={onApprove}>批准继续</button>
        )}
        {onViewEvidence && (
          <button className="btn btn-ghost" onClick={onViewEvidence}>查看依据</button>
        )}
        {onReject && (
          <button className="btn btn-ghost" onClick={onReject}>要求修订</button>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// BottomDrawerItem — single tab in the bottom collapsible drawer
// ---------------------------------------------------------------------------

interface BottomDrawerItemProps {
  icon: string;
  label: string;
  active?: boolean;
  badge?: string;
  onClick?: () => void;
}

export function BottomDrawerItem({ icon, label, active, badge, onClick }: BottomDrawerItemProps) {
  return (
    <button
      className={`galen-drawer-item ${active ? "galen-drawer-item-active" : ""}`}
      onClick={onClick}
      type="button"
    >
      <span className="galen-drawer-item-icon">{icon}</span>
      <span className="galen-drawer-item-label">{label}</span>
      {badge && <span className="galen-drawer-item-badge">{badge}</span>}
      <span className="galen-drawer-item-arrow">▸</span>
    </button>
  );
}

// ---------------------------------------------------------------------------
// InspectorSection — collapsible section in the right inspection panel
// ---------------------------------------------------------------------------

interface InspectorSectionProps {
  title: string;
  children?: ReactNode;
  emptyMessage?: string;
}

export function InspectorSection({ title, children, emptyMessage }: InspectorSectionProps) {
  return (
    <div className="galen-inspector-section">
      <div className="galen-inspector-section-header">
        <h3>{title}</h3>
      </div>
      <div className="galen-inspector-section-body">
        {children ?? <p className="galen-inspector-empty">{emptyMessage ?? "暂无数据"}</p>}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// SelectionActionMenu — floating menu triggered by text selection
// ---------------------------------------------------------------------------

export const SELECTION_ACTIONS = [
  { id: "polish", label: "润色表达" },
  { id: "evidence", label: "补充依据" },
  { id: "stats_lang", label: "改写为统计语言" },
  { id: "alt_version", label: "生成替代版本" },
  { id: "explain", label: "解释此段落" },
] as const;

interface SelectionActionMenuProps {
  position: { x: number; y: number };
  onAction: (actionId: string) => void;
  onDismiss: () => void;
}

export function SelectionActionMenu({ position, onAction, onDismiss }: SelectionActionMenuProps) {
  return (
    <>
      <div className="galen-selection-backdrop" onClick={onDismiss} />
      <div
        className="galen-selection-menu"
        style={{ left: position.x, top: position.y } as CSSProperties}
        role="menu"
      >
        <div className="galen-selection-menu-title">AI 修订</div>
        {SELECTION_ACTIONS.map((action) => (
          <button
            key={action.id}
            className="galen-selection-menu-item"
            onClick={() => onAction(action.id)}
            role="menuitem"
            type="button"
          >
            {action.label}
          </button>
        ))}
      </div>
    </>
  );
}
