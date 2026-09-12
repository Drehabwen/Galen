import { useState } from "react";
import { BottomDrawerItem } from "./ui/primitives";
import type { ArtifactRecord } from "../domain/artifact";
import { FIRST_PARTY_CONNECTORS } from "../domain/connectors";

const RESOURCES = [
  { id: "logs", icon: "☰", label: "全局日志" },
  { id: "versions", icon: "⇄", label: "版本" },
  { id: "evidence_lib", icon: "⊗", label: "证据库" },
  { id: "code_repo", icon: "</>", label: "代码仓库" },
  { id: "connectors", icon: "↔", label: "连接器" },
  { id: "artifacts_lib", icon: "⬡", label: "产物库" },
] as const;

interface GlobalResourceBarProps {
  artifacts?: ArtifactRecord[];
  onOpenArtifact?: (artifact: ArtifactRecord) => void;
}

function formatBytes(size: number): string {
  if (size < 1024) return `${size} B`;
  return `${(size / 1024).toFixed(1)} KB`;
}

export function GlobalResourceBar({ artifacts = [], onOpenArtifact }: GlobalResourceBarProps) {
  const [activeTab, setActiveTab] = useState<string | null>(null);

  return (
    <div className={`global-resource-bar ${activeTab ? "global-resource-bar-open" : ""}`}>
      <div className="global-resource-tabs">
        {RESOURCES.map((r) => (
          <BottomDrawerItem
            key={r.id}
            icon={r.icon}
            label={r.label}
            active={activeTab === r.id}
            onClick={() => setActiveTab(activeTab === r.id ? null : r.id)}
          />
        ))}
      </div>
      {activeTab && (
        <div className="global-resource-content">
          {activeTab === "connectors" ? (
            <div className="connector-ledger">
              <div className="artifact-ledger-heading">
                <span>科研数据源</span>
                <strong>{FIRST_PARTY_CONNECTORS.length}</strong>
              </div>
              <p className="connector-ledger-intro">这里仅显示数据源状态。需要数据时，直接在主对话中说“从康复师工作台获取 ATH-001 最近三次评估”。</p>
              <div className="connector-ledger-list">
                {FIRST_PARTY_CONNECTORS.map((connector) => (
                  <div className="connector-ledger-card" key={connector.id}>
                    <div className="connector-ledger-card-head">
                      <strong>{connector.label}</strong>
                      <span className={`connector-status connector-status-${connector.status}`}>{connector.status === "ready" ? "可导入" : "需适配"}</span>
                    </div>
                    <p>{connector.notes}</p>
                    <div className="connector-ledger-meta">格式：{connector.exportFormats.join(" / ")} · 写入：{connector.normalizedEntities.join("、")}</div>
                    <div className="connector-ledger-mode">{connector.status === "ready" ? "可由对话调用" : "等待适配"}</div>
                  </div>
                ))}
              </div>
            </div>
          ) : activeTab === "artifacts_lib" ? (
            <div className="artifact-ledger">
              <div className="artifact-ledger-heading">
                <span>交付记录</span>
                <strong>{artifacts.length}</strong>
              </div>
              {artifacts.length === 0 ? (
                <p className="session-empty">任务交付后，文档、数据和图表会出现在这里。</p>
              ) : (
                <div className="artifact-ledger-list">
                  {artifacts.map((artifact) => (
                    <button
                      key={artifact.id}
                      type="button"
                      className="artifact-ledger-row"
                      onClick={() => {
                        onOpenArtifact?.(artifact);
                        setActiveTab(null);
                      }}
                    >
                      <span className="artifact-ledger-kind">{artifact.kind}</span>
                      <span className="artifact-ledger-path">{artifact.path}</span>
                      <span className="artifact-ledger-size">{formatBytes(artifact.size)}</span>
                    </button>
                  ))}
                </div>
              )}
            </div>
          ) : (
            <p className="session-empty" style={{ padding: "var(--space-4)" }}>
              {activeTab === "logs" && "全局日志 — 展开后显示所有 Session 的执行日志汇总"}
              {activeTab === "versions" && "版本 — 展开后显示项目级版本记录与快照"}
              {activeTab === "evidence_lib" && "证据库 — 展开后显示课题关联的所有证据材料"}
              {activeTab === "code_repo" && "代码仓库 — 展开后显示所有 Session 的运行脚本"}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
