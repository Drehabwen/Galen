import type { CSSProperties, ReactNode } from "react";

export type WorkbenchView = "execution-thread" | "daily-workbench" | "data-quality" | "rehab-context";

interface WorkbenchRailProps {
  activeView: WorkbenchView;
  onViewChange: (view: WorkbenchView) => void;
  canvasTab: "insight" | "plan" | "evidence" | "doc";
  onCanvasTabChange: (tab: "insight" | "plan" | "evidence" | "doc") => void;
  completedNodes: number;
  totalNodes: number;
}

function RailIcon({ children }: { children: ReactNode }) {
  return <span className="workbench-rail-icon" aria-hidden="true">{children}</span>;
}

export function WorkbenchRail({
  activeView,
  onViewChange,
  canvasTab,
  onCanvasTabChange,
  completedNodes,
  totalNodes,
}: WorkbenchRailProps) {
  const progress = totalNodes > 0 ? Math.round((completedNodes / totalNodes) * 100) : 0;
  const progressStyle = { "--research-progress": `${progress}%` } as CSSProperties;

  return (
    <nav className="workbench-rail" aria-label="Galen 工作区导航">
      <div className="workbench-rail-monogram" aria-label="Galen">G</div>

      <div className="workbench-rail-primary">
        <button
          type="button"
          className={`workbench-rail-action ${activeView === "execution-thread" ? "active" : ""}`}
          onClick={() => onViewChange("execution-thread")}
          title="研究任务"
          aria-label="研究任务"
        >
          <RailIcon>
            <svg viewBox="0 0 24 24"><path d="M6 4.5h9l3 3V19.5H6z"/><path d="M15 4.5v3h3M9 11h6M9 14.5h6"/></svg>
          </RailIcon>
          <span>任务</span>
        </button>
        <button
          type="button"
          className={`workbench-rail-action ${activeView === "daily-workbench" ? "active" : ""}`}
          onClick={() => onViewChange("daily-workbench")}
          title="项目资料"
          aria-label="项目资料"
        >
          <RailIcon>
            <svg viewBox="0 0 24 24"><path d="M4.5 7.5h6l1.5 2h7.5v9h-15z"/><path d="M4.5 7.5v-2h6l1.5 2"/></svg>
          </RailIcon>
          <span>资料</span>
        </button>
        <button
          type="button"
          className={`workbench-rail-action ${activeView === "data-quality" ? "active" : ""}`}
          onClick={() => onViewChange("data-quality")}
          title="数据体检与清洗"
          aria-label="数据体检与清洗"
        >
          <RailIcon>
            <svg viewBox="0 0 24 24"><path d="M5 4.5h14v15H5z"/><path d="M8 8h8M8 12h3M8 16h8"/><path d="m15.5 12.5 1.2 1.2 2.3-2.5"/></svg>
          </RailIcon>
          <span>数据</span>
        </button>
        <button
          type="button"
          className={`workbench-rail-action ${activeView === "rehab-context" ? "active" : ""}`}
          onClick={() => onViewChange("rehab-context")}
          title="病例证据"
          aria-label="病例证据"
        >
          <RailIcon>
            <svg viewBox="0 0 24 24"><path d="M5 5.5h14v13H5z"/><path d="M8 12h2l1.2-3 2 6 1.3-3H17"/></svg>
          </RailIcon>
          <span>病例</span>
        </button>
      </div>

      {activeView === "execution-thread" && (
        <div className="workbench-rail-secondary" aria-label="研究任务视图">
          <button
            type="button"
            className={`workbench-rail-action compact ${canvasTab === "insight" ? "active" : ""}`}
            onClick={() => onCanvasTabChange("insight")}
            title="分析结果"
            aria-label="分析结果"
          >
            <RailIcon>
              <svg viewBox="0 0 24 24"><path d="M5 19.5V5.5M5 19.5h14"/><path d="m8 15 3-3 2.5 2.5L19 8"/><circle cx="8" cy="15" r="1"/><circle cx="11" cy="12" r="1"/><circle cx="13.5" cy="14.5" r="1"/><circle cx="19" cy="8" r="1"/></svg>
            </RailIcon>
          </button>
          <button
            type="button"
            className={`workbench-rail-action compact ${canvasTab === "plan" ? "active" : ""}`}
            onClick={() => onCanvasTabChange("plan")}
            title="证据脉络"
            aria-label="证据脉络"
          >
            <RailIcon>
              <svg viewBox="0 0 24 24"><circle cx="12" cy="5" r="2"/><circle cx="7" cy="18" r="2"/><circle cx="17" cy="18" r="2"/><path d="M12 7v4M12 11 7 16M12 11l5 5"/></svg>
            </RailIcon>
          </button>
          <button
            type="button"
            className={`workbench-rail-action compact ${canvasTab === "evidence" ? "active" : ""}`}
            onClick={() => onCanvasTabChange("evidence")}
            title="查看依据"
            aria-label="查看依据"
          >
            <RailIcon>
              <svg viewBox="0 0 24 24"><path d="M6 4.5h9l3 3V19.5H6z"/><path d="M15 4.5v3h3M9 11h6M9 14.5h6"/><circle cx="17.5" cy="17.5" r="3"/><path d="m19.7 19.7 1.8 1.8"/></svg>
            </RailIcon>
          </button>
          <button
            type="button"
            className={`workbench-rail-action compact ${canvasTab === "doc" ? "active" : ""}`}
            onClick={() => onCanvasTabChange("doc")}
            title="成果预览"
            aria-label="成果预览"
          >
            <RailIcon>
              <svg viewBox="0 0 24 24"><path d="M6 3.5h9l3 3v14H6z"/><path d="M15 3.5v3h3M9 11h6M9 14.5h6"/></svg>
            </RailIcon>
          </button>
        </div>
      )}

      <div className="workbench-rail-progress" title={`研究进度 ${completedNodes}/${totalNodes}`}>
        <span className="workbench-rail-progress-ring" style={progressStyle}>
          <span>{totalNodes > 0 ? progress : "—"}</span>
        </span>
        <small>进度</small>
      </div>
    </nav>
  );
}
