import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { FileEntry } from "../types";
import {
  resolveDomain,
  classifyEntries,
  artifactTypeLabel,
  formatSize,
} from "../domain/registry";
import type { ActiveDomain } from "../domain/registry";
import {
  StatusDot,
  ProgressBar,
  Tag,
  ApprovalCard,
  EmptyState,
} from "./ui/primitives";

// ---------------------------------------------------------------------------
// Five research stages
// ---------------------------------------------------------------------------
const RESEARCH_STAGES = [
  { id: "design", label: "课题设计", detail: "方案、纳排标准、伦理" },
  { id: "data", label: "数据处理", detail: "清洗、编码、质控" },
  { id: "stats", label: "统计分析", detail: "描述统计、推断、建模" },
  { id: "charts", label: "图表生成", detail: "基线表、森林图、KM曲线" },
  { id: "writing", label: "论文写作", detail: "方法学、结果、讨论" },
] as const;

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------
interface ResearchWorkbenchProps {
  wsRoot: string | null;
  files: FileEntry[];
  currentFile: { path: string; content: string } | null;
  backendAvailable: boolean;
  onAgentPrompt: (prompt: string) => void;
  onReadFile: (path: string) => void;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------
export function ResearchWorkbench({
  wsRoot,
  files,
  currentFile: _currentFile,
  backendAvailable,
  onAgentPrompt,
  onReadFile: _onReadFile,
}: ResearchWorkbenchProps) {
  const [rootFiles, setRootFiles] = useState<FileEntry[]>([]);
  useEffect(() => {
    if (!wsRoot) { setRootFiles([]); return; }
    invoke<FileEntry[]>("list_workspace_files", { path: null })
      .then(setRootFiles)
      .catch(() => setRootFiles(files));
  }, [wsRoot, files]);

  const entries = useMemo(() => {
    const source = rootFiles.length > 0 ? rootFiles : files;
    return [...source].sort(
      (a, b) => Number(b.is_dir) - Number(a.is_dir) || a.name.localeCompare(b.name),
    );
  }, [rootFiles, files]);

  const domain: ActiveDomain = useMemo(
    () => resolveDomain(wsRoot, entries),
    [wsRoot, entries],
  );
  const classified = useMemo(() => classifyEntries(entries), [entries]);
  const fileArtifacts = classified.filter((c) => !c.entry.is_dir);
  const packageName = wsRoot
    ? wsRoot.split(/[/\\]/).pop() ?? "未命名"
    : "未选择项目";

  // Recent artifacts (top 5 non-directory entries)
  const recentArtifacts = useMemo(
    () => fileArtifacts.slice(0, 5),
    [fileArtifacts],
  );

  // Active stage (inferred from file types)
  const activeStage = useMemo(() => {
    const hasSource = classified.some((c) => c.kind === "source");
    const hasData = classified.some((c) => c.kind === "data");
    const hasDoc = classified.some((c) => c.kind === "doc");
    if (hasSource) return 2; // stats stage
    if (hasData) return 1; // data stage
    if (hasDoc) return 4; // writing stage
    return 0; // design stage
  }, [classified]);

  const sendPackagePrompt = (prompt: string) => {
    onAgentPrompt(prompt);
  };

  // -------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------
  return (
    <div className="daily-workbench">
      {/* ── Header: Current Goal ── */}
      <header className="daily-header">
        <div className="daily-header-left">
          <span className="daily-kicker">当前目标</span>
          <h1>
            {wsRoot
              ? domain.identity.kind === "clinical"
                ? `${packageName} · 临床研究`
                : `${packageName} · 软件开发`
              : "打开工作区以开始"}
          </h1>
          <p className="daily-summary">
            {wsRoot
              ? domain.identity.summary
              : "选择项目目录，Galen 将自动识别项目类型并推荐下一步。"}
          </p>
        </div>
        <div className="daily-header-right">
          <StatusDot tone={chatActive(backendAvailable) ? "active" : "idle"}>
            {backendAvailable ? "AI 就绪" : "后端未连接"}
          </StatusDot>
          <button
            className="btn btn-primary"
            onClick={() =>
              sendPackagePrompt(
                domain.identity.kind === "clinical"
                  ? "请审查当前课题方案，按五个阶段列出缺口和优先事项。"
                  : "请分析当前项目结构，按优先级列出改进建议。",
              )
            }
          >
            生成下一步
          </button>
        </div>
      </header>

      {/* ── Five-Stage Progress ── */}
      <section className="daily-section">
        <div className="daily-section-header">
          <h2>阶段进度</h2>
        </div>
        <div className="daily-stages">
          {RESEARCH_STAGES.map((stage, i) => {
            const state: "done" | "active" | "pending" =
              i < activeStage ? "done" : i === activeStage ? "active" : "pending";
            return (
              <button
                key={stage.id}
                className={`daily-stage daily-stage-${state}`}
                onClick={() =>
                  sendPackagePrompt(
                    `请针对「${stage.label}」阶段（${stage.detail}）提出具体执行计划。`,
                  )
                }
                type="button"
              >
                <span className="daily-stage-index">
                  {state === "done" ? "✓" : i + 1}
                </span>
                <div className="daily-stage-body">
                  <strong>{stage.label}</strong>
                  <span>{stage.detail}</span>
                </div>
                {state === "active" && (
                  <Tag type="status">进行中</Tag>
                )}
                {state === "done" && (
                  <Tag type="execution">已完成</Tag>
                )}
              </button>
            );
          })}
        </div>
      </section>

      {/* ── Three-column: Executing | Recent Artifacts | Next Steps ── */}
      <div className="daily-grid">
        {/* Currently executing */}
        <section className="daily-panel">
          <div className="daily-panel-header">
            <h3>正在执行</h3>
          </div>
          <div className="daily-panel-body">
            {backendAvailable && wsRoot ? (
              <div className="daily-exec-placeholder">
                <StatusDot tone="active">AI 运行中</StatusDot>
                <p>尚未发起执行任务。点击上方阶段或使用科研执行线程开始。</p>
              </div>
            ) : (
              <EmptyState message={wsRoot ? "后端未连接" : "请先选择工作区"} />
            )}
          </div>
        </section>

        {/* Recent artifacts */}
        <section className="daily-panel">
          <div className="daily-panel-header">
            <h3>最近产物</h3>
          </div>
          <div className="daily-panel-body">
            {recentArtifacts.length > 0 ? (
              <table className="daily-artifact-table">
                <thead>
                  <tr>
                    <th>名称</th>
                    <th>类型</th>
                    <th>大小</th>
                  </tr>
                </thead>
                <tbody>
                  {recentArtifacts.map(({ entry, kind }) => (
                    <tr key={entry.path || entry.name}>
                      <td className="daily-artifact-name">{entry.name}</td>
                      <td>
                        <Tag type="phase">{artifactTypeLabel(kind)}</Tag>
                      </td>
                      <td className="daily-artifact-size">
                        {formatSize(entry.size)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : (
              <EmptyState message="尚无产物" />
            )}
          </div>
        </section>

        {/* Next steps + pending approval */}
        <section className="daily-panel">
          <div className="daily-panel-header">
            <h3>下一步建议</h3>
          </div>
          <div className="daily-panel-body">
            {wsRoot ? (
              <div className="daily-next-list">
                <button
                  className="daily-next-item"
                  onClick={() =>
                    sendPackagePrompt("请审查当前项目并提出下一步行动计划。")
                  }
                  type="button"
                >
                  <span className="daily-next-icon">→</span>
                  <span>AI 分析项目并推荐下一步</span>
                </button>
                <button
                  className="daily-next-item"
                  onClick={() =>
                    sendPackagePrompt("请检查当前项目的文档完整性。")
                  }
                  type="button"
                >
                  <span className="daily-next-icon">→</span>
                  <span>检查文档完整性</span>
                </button>
                <button
                  className="daily-next-item"
                  onClick={() =>
                    sendPackagePrompt(
                      "请分析数据质量并提出清洗方案。",
                    )
                  }
                  type="button"
                >
                  <span className="daily-next-icon">→</span>
                  <span>数据质量评估</span>
                </button>
              </div>
            ) : (
              <EmptyState message="打开工作区以获取建议" />
            )}
          </div>

          <div className="daily-panel-header" style={{ borderTop: "1px solid var(--border-muted)" }}>
            <h3>等待签核</h3>
          </div>
          <div className="daily-panel-body">
            <EmptyState message="无待签核事项" />
          </div>
        </section>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function chatActive(backendAvailable: boolean): boolean {
  // In a real implementation, this would check if the chat loop is running
  return backendAvailable;
}
