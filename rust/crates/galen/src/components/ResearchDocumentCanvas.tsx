import { useEffect, useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

interface ResearchDocumentCanvasProps {
  artifact?: { path: string; content: string; nodeTitle?: string } | null;
  loading?: boolean;
  error?: string | null;
  onBackToPlan?: () => void;
}

export function ResearchDocumentCanvas({ artifact, loading, error, onBackToPlan }: ResearchDocumentCanvasProps) {
  useEffect(() => {}, [artifact]);
  const kind = useMemo(() => artifact?.path.split(".").pop()?.toUpperCase() || "DOCUMENT", [artifact]);

  return (
    <div className="doc-canvas artifact-canvas">
      <div className="doc-toolbar">
        <div className="doc-toolbar-left">
          <span className="artifact-live-dot" />
          <span className="artifact-kicker">GALEN INTERNAL PREVIEW</span>
          {artifact && <span className="galen-tag galen-tag-evidence">{kind}</span>}
        </div>
        <button className="artifact-back" type="button" onClick={onBackToPlan}>返回证据流</button>
      </div>
      {loading ? (
        <div className="artifact-empty"><div className="artifact-loader" /><h3>正在载入产物</h3><p>文件留在当前工作区，不会跳转到外部应用。</p></div>
      ) : error ? (
        <div className="artifact-empty artifact-error"><h3>暂时无法预览</h3><p>{error}</p></div>
      ) : !artifact ? (
        <div className="artifact-empty">
          <div className="artifact-empty-icon">⌁</div>
          <h3>产物在 Galen 内直接展开</h3>
          <p>从科研证据流选择任一节点产物，即可在这里审阅 Markdown、数据、脚本与研究记录。</p>
        </div>
      ) : (
        <>
          <header className="artifact-titlebar">
            <span className="artifact-origin">来自 {artifact.nodeTitle || "科研节点"}</span>
            <h2>{artifact.path.split(/[\\/]/).pop()}</h2>
            <span className="artifact-path">{artifact.path}</span>
          </header>
          <div className="artifact-preview-scroll">
            <article className="artifact-preview-content" data-testid="artifact-rendered-preview">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{artifact.content}</ReactMarkdown>
            </article>
          </div>
          <footer className="doc-footer">
            <span className="doc-footer-stat">{artifact.content.length} 字符 · 工作区内预览</span>
            <span className="doc-footer-hint">无需打开外部编辑器</span>
          </footer>
        </>
      )}
    </div>
  );
}
