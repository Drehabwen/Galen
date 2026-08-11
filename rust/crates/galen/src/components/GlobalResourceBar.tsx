import { useState } from "react";
import { BottomDrawerItem } from "./ui/primitives";

const RESOURCES = [
  { id: "logs", icon: "☰", label: "全局日志" },
  { id: "versions", icon: "⇄", label: "版本" },
  { id: "evidence_lib", icon: "⊗", label: "证据库" },
  { id: "code_repo", icon: "</>", label: "代码仓库" },
  { id: "artifacts_lib", icon: "⬡", label: "产物库" },
] as const;

export function GlobalResourceBar() {
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
          <p className="session-empty" style={{ padding: "var(--space-4)" }}>
            {activeTab === "logs" && "全局日志 — 展开后显示所有 Session 的执行日志汇总"}
            {activeTab === "versions" && "版本 — 展开后显示项目级版本记录与快照"}
            {activeTab === "evidence_lib" && "证据库 — 展开后显示课题关联的所有证据材料"}
            {activeTab === "code_repo" && "代码仓库 — 展开后显示所有 Session 的运行脚本"}
            {activeTab === "artifacts_lib" && "产物库 — 展开后显示已生成的表格、图表、文档"}
          </p>
        </div>
      )}
    </div>
  );
}
