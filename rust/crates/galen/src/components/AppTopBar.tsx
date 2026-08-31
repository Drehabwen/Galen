import type { CapabilityManifest } from "../hooks/useEnvironment";
import type { ChatMode, ModeMeta } from "../hooks/useMode";
import type { ModelStatus } from "../types";
import { UpdateManager } from "./UpdateManager";
import { StatusDot } from "./ui/primitives";

interface AppTopBarProps {
  studyLabel: string;
  modes: ModeMeta[];
  mode: ChatMode;
  onModeChange: (mode: ChatMode) => void;
  memorySize?: number;
  capabilities: CapabilityManifest[];
  modelStatuses: ModelStatus[];
  running: boolean;
  workspaceSelected: boolean;
  workspaceLabel: string;
  onOpenModelStatus: () => void;
  onPickWorkspace: () => void;
}

const MODE_FALLBACK: Record<ChatMode, { label: string; description: string }> = {
  plan: { label: "计划", description: "制定方案，列出步骤，确认后执行" },
  auto: { label: "自动", description: "自主分解目标，并行执行，汇总产出" },
};

export function AppTopBar({
  studyLabel,
  modes,
  mode,
  onModeChange,
  memorySize,
  capabilities,
  modelStatuses,
  running,
  workspaceSelected,
  workspaceLabel,
  onOpenModelStatus,
  onPickWorkspace,
}: AppTopBarProps) {
  const enabledCapabilities = capabilities.filter((item) => item.enabled);

  return (
    <div className="galen-topbar">
      <div className="galen-topbar-identity">
        <span className="galen-topbar-brand">Galen</span>
        <span className="galen-topbar-discipline">康复科研工作台</span>
      </div>
      <div className="galen-topbar-study">
        <span className="galen-topbar-study-label">当前研究</span>
        <span className="galen-topbar-project">{studyLabel}</span>
      </div>
      <div className="galen-topbar-spacer" />

      <div className="galen-mode-switch" role="group" aria-label="工作模式">
        {(["auto", "plan"] as ChatMode[]).map((id) => {
          const meta = modes.find((item) => item.id === id);
          const fallback = MODE_FALLBACK[id];
          return (
            <button
              key={id}
              className={`galen-mode-btn ${mode === id ? "active" : ""}`}
              onClick={() => onModeChange(id)}
              title={meta?.description ?? fallback.description}
            >
              {meta?.label ?? fallback.label}
            </button>
          );
        })}
      </div>

      {memorySize !== undefined && (
        <span
          title={`GALEN.md · ${memorySize} 字节`}
          style={{
            fontSize: "var(--text-xs)",
            color: "var(--success)",
            background: "var(--success-soft)",
            padding: "1px 8px",
            borderRadius: "var(--radius-pill)",
          }}
        >
          记忆已加载
        </span>
      )}

      {enabledCapabilities.length > 0 && (
        <span
          title={capabilities
            .map(
              (item) =>
                `${item.enabled ? "✓" : "○"} ${item.name} · ${item.toolNames.length} 工具 · ${item.uiSlots.join(", ")}`,
            )
            .join("\n")}
          style={{
            fontSize: "var(--text-xs)",
            color: "var(--text-secondary)",
            background: "var(--bg-elevated)",
            padding: "1px 8px",
            borderRadius: "var(--radius-pill)",
          }}
        >
          能力 {enabledCapabilities.length}/{capabilities.length}
        </span>
      )}

      <UpdateManager />
      <button
        className="btn btn-sm btn-ghost"
        onClick={onOpenModelStatus}
        title="管理模型、API Key 与连接状态"
      >
        设置
        <span
          className={`model-status-dot ${
            modelStatuses.length > 0 &&
            modelStatuses.every((status) => status.api_key_present)
              ? "ok"
              : "missing"
          }`}
        />
      </button>
      <StatusDot tone={running ? "active" : "idle"}>
        {running ? "运行中" : "就绪"}
      </StatusDot>
      <button className="btn btn-sm btn-ghost" onClick={onPickWorkspace}>
        {workspaceSelected ? workspaceLabel : "选择工作区"}
      </button>
    </div>
  );
}
