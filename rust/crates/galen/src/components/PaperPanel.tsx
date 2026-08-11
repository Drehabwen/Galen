import { useState, useCallback } from "react";
import type { Paper } from "../types";

interface Props {
  papers: Paper[];
}

function formatVancouver(paper: Paper): string {
  const authors = paper.authors.slice(0, 3).join(", ");
  const etAl = paper.authors.length > 3 ? " et al." : "";
  const year = paper.year ?? "?";
  const journal = paper.journal ?? "?";
  return `${authors}${etAl}. ${paper.title}. ${journal}. ${year}. PMID: ${paper.pmid}.`;
}

function CopyButton({ text, label }: { text: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }, [text]);
  return (
    <button
      className={`paper-detail-copy ${copied ? "copied" : ""}`}
      onClick={handleCopy}
    >
      {copied ? "已复制" : label ?? "复制引用"}
    </button>
  );
}

export function PaperPanel({ papers }: Props) {
  const [selected, setSelected] = useState<Paper | null>(null);

  if (papers.length === 0) {
    return (
      <div className="placeholder">
        <p>暂无文献</p>
        <p>通过 Agent 检索 PubMed 获取文献。</p>
      </div>
    );
  }

  return (
    <div className="paper-panel">
      <div className="paper-count">{papers.length} 篇文献</div>
      <div className="paper-list">
        {papers.map((paper) => (
          <div
            key={paper.pmid}
            className={`paper-item ${selected?.pmid === paper.pmid ? "paper-selected" : ""}`}
            onClick={() => setSelected(paper)}
          >
            <div className="paper-title">{paper.title}</div>
            <div className="paper-meta">
              {paper.journal && (
                <span className="paper-journal-badge">{paper.journal}</span>
              )}
              <span className="paper-year">{paper.year ?? "?"}</span>
              <span>· {paper.authors[0] ?? "?"}</span>
            </div>
            <div className="paper-pmid">PMID: {paper.pmid}</div>
          </div>
        ))}
      </div>

      {selected && (
        <div className="paper-detail">
          <div className="paper-detail-header">
            <div className="paper-detail-title">{selected.title}</div>
            <CopyButton text={selected.title} label="复制标题" />
          </div>
          <div className="paper-authors">
            {selected.authors.join(", ")}
          </div>
          <div className="paper-citation-row">
            <span className="citation-tag">
              {selected.journal ?? "?"} · {selected.year ?? "?"}
            </span>
            <span className="citation-tag">PMID: {selected.pmid}</span>
            {selected.doi && (
              <span className="citation-tag">DOI: {selected.doi}</span>
            )}
          </div>
          <CopyButton text={formatVancouver(selected)} label="复制引用" />
          <div style={{ marginTop: 14 }}>
            <div className="abstract-header">摘要</div>
            <div className="abstract-body">
              {selected.abstract_text ?? "(无摘要)"}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
