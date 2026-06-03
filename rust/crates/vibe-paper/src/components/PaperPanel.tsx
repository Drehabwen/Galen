import { useState } from "react";
import type { Paper } from "../types";

interface Props {
  papers: Paper[];
}

export function PaperPanel({ papers }: Props) {
  const [selected, setSelected] = useState<Paper | null>(null);

  if (papers.length === 0) {
    return (
      <div className="placeholder">
        <p>在聊天中提出医学问题</p>
        <p>AI 会自动检索 PubMed</p>
        <hr />
        <p>💡 试试问:</p>
        <p>"帮我查阿尔茨海默病的最新综述"</p>
        <p>"二甲双胍的作用机制是什么"</p>
      </div>
    );
  }

  return (
    <div className="paper-panel">
      <div className="paper-count">找到 {papers.length} 篇文献</div>
      <div className="paper-list">
        {papers.map((paper) => (
          <div
            key={paper.pmid}
            className={`paper-item ${selected?.pmid === paper.pmid ? "paper-selected" : ""}`}
            onClick={() => setSelected(paper)}
          >
            <div className="paper-title">{paper.title}</div>
            <div className="paper-meta">
              {paper.authors[0] ?? "?"} | {paper.journal ?? "?"} ({paper.year ?? "?"})
            </div>
            <div className="paper-pmid">PMID: {paper.pmid}</div>
          </div>
        ))}
      </div>

      {selected && (
        <div className="paper-abstract">
          <div className="abstract-header">📋 摘要</div>
          <div className="abstract-body">
            {selected.abstract_text ?? "(无摘要)"}
          </div>
        </div>
      )}
    </div>
  );
}
