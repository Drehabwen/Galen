import { openUrl } from "@tauri-apps/plugin-opener";

export type VerifiableSource = {
  kind: "pmid" | "doi" | "url";
  id: string;
  href: string;
  label: string;
};

function sourceProvider(source: VerifiableSource): string {
  if (source.kind === "pmid") return "PubMed";
  if (source.kind === "doi") return "DOI 注册机构 / 出版商";
  return "外部来源";
}

export function sourceFromLink(href: string, label: string): VerifiableSource | null {
  const pmidMatch = href.match(/pubmed\.ncbi\.nlm\.nih\.gov\/(\d+)/i) ?? label.match(/PMID\s*[:：]?\s*(\d+)/i);
  if (pmidMatch) {
    return { kind: "pmid", id: pmidMatch[1], href, label };
  }
  const doiMatch = href.match(/doi\.org\/(10\.\d{4,9}\/[^\s?#]+)/i) ?? label.match(/DOI\s*[:：]?\s*(10\.\d{4,9}\/[^\s]+)/i);
  if (doiMatch) {
    return { kind: "doi", id: doiMatch[1].replace(/[).,;]+$/, ""), href, label };
  }
  if (/^https?:\/\//i.test(href)) {
    return { kind: "url", id: href, href, label };
  }
  return null;
}

interface SourceInspectorProps {
  source: VerifiableSource;
  onClose: () => void;
}

export function SourceInspector({ source, onClose }: SourceInspectorProps) {
  const handleOpen = async () => {
    try {
      await openUrl(source.href);
    } catch {
      // Keep the inspector usable in the browser preview as well as in Tauri.
      window.open(source.href, "_blank", "noopener,noreferrer");
    }
  };

  return (
    <aside className="source-inspector" aria-label="来源核验面板">
      <div className="source-inspector__header">
        <div>
          <p className="eyebrow">EVIDENCE PROVENANCE</p>
          <h2>来源核验</h2>
        </div>
        <button className="source-inspector__close" type="button" aria-label="关闭" onClick={onClose}>
          ×
        </button>
      </div>

      <div className="source-inspector__badge-row">
        <span className="source-inspector__badge">{source.kind.toUpperCase()}</span>
        <span className="source-inspector__provider">{sourceProvider(source)}</span>
      </div>
      <p className="source-inspector__label">{source.label}</p>
      <p className="source-inspector__copy">Galen 从当前建议中识别到的来源。打开链接即可核对原始记录。</p>

      <div className="source-inspector__link" title={source.href}>{source.href}</div>
      <div className="source-inspector__actions">
        <button className="btn btn-primary" type="button" onClick={() => void handleOpen()}>
          打开原文
        </button>
        <button className="btn btn-ghost" type="button" onClick={onClose}>
          关闭
        </button>
      </div>
      <p className="source-inspector__note">当前仅展示可验证链接；文献元数据与 PDF 阅读将在下一步接入。</p>
    </aside>
  );
}
