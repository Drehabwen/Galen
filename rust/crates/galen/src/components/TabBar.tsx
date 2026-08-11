import type { LibraryTab } from "../types";

interface Props {
  active: LibraryTab;
  onChange: (tab: LibraryTab) => void;
}

const TABS: { id: LibraryTab; label: string }[] = [
  { id: "files", label: "文件" },
  { id: "papers", label: "文献" },
  { id: "notes", label: "任务" },
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
          {t.label}
        </button>
      ))}
    </div>
  );
}
