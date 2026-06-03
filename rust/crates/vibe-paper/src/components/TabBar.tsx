import type { LeftTab } from "../types";

interface Props {
  active: LeftTab;
  onChange: (tab: LeftTab) => void;
}

const TABS: { id: LeftTab; label: string; icon: string }[] = [
  { id: "workspace", label: "工作区", icon: "📁" },
  { id: "papers", label: "文献", icon: "📄" },
  { id: "notes", label: "笔记", icon: "📝" },
];

export function TabBar({ active, onChange }: Props) {
  return (
    <div className="tab-bar">
      {TABS.map((t) => (
        <button
          key={t.id}
          className={`tab-btn ${active === t.id ? "tab-active" : ""}`}
          onClick={() => onChange(t.id)}
        >
          {t.icon} {t.label}
        </button>
      ))}
    </div>
  );
}
