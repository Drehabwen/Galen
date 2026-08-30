import { useEffect, useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import mammoth from "mammoth";
import { readSheet } from "read-excel-file/browser";
import type { Row } from "read-excel-file/browser";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneLight } from "react-syntax-highlighter/dist/esm/styles/prism";
import type { ArtifactPreview } from "../domain/preview";
import { codeLanguageOf, previewKindLabel } from "../domain/preview";

interface ResearchDocumentCanvasProps {
  artifact?: ArtifactPreview | null;
  loading?: boolean;
  error?: string | null;
  onBackToPlan?: () => void;
}

function MarkdownView({ content }: { content: string }) {
  return (
    <article className="artifact-preview-content" data-testid="artifact-rendered-preview">
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
    </article>
  );
}

function CsvView({ content }: { content: string }) {
  const rows = useMemo(() => {
    return content
      .split(/\r?\n/)
      .filter((line) => line.trim().length > 0)
      .slice(0, 500)
      .map((line) => line.split(/[,\t]/));
  }, [content]);
  return (
    <div className="artifact-preview-scroll">
      <div className="artifact-table-wrap" data-testid="artifact-csv-table">
        <table className="artifact-table">
          <tbody>
            {rows.map((row, index) => (
              <tr key={index}>
                {row.map((cell, cellIndex) => (
                  <td key={cellIndex}>{cell}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function CodeView({ content, path }: { content: string; path: string }) {
  const language = codeLanguageOf(path);
  return (
    <div className="artifact-preview-scroll">
      <div className="artifact-code-view" data-testid="artifact-code-view">
        <SyntaxHighlighter
          language={language}
          style={oneLight}
          showLineNumbers
          customStyle={{ margin: 0, fontSize: "12px", lineHeight: 1.65, borderRadius: 8 }}
        >
          {content}
        </SyntaxHighlighter>
      </div>
    </div>
  );
}

function useObjectUrl(blob?: Blob): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!blob) {
      setUrl(null);
      return;
    }
    const objectUrl = URL.createObjectURL(blob);
    setUrl(objectUrl);
    return () => URL.revokeObjectURL(objectUrl);
  }, [blob]);
  return url;
}

function PdfView({ blob }: { blob: Blob }) {
  const url = useObjectUrl(blob);
  return (
    <div className="artifact-preview-scroll artifact-embed-frame">
      {url ? (
        <iframe title="PDF 预览" src={url} data-testid="artifact-pdf-frame" />
      ) : (
        <div className="artifact-empty">正在准备 PDF 预览…</div>
      )}
    </div>
  );
}

function ImageView({ blob, path }: { blob: Blob; path: string }) {
  const url = useObjectUrl(blob);
  if (!url) return <div className="artifact-empty">正在准备图片预览…</div>;
  return (
    <div className="artifact-preview-scroll">
      <img className="artifact-image" src={url} alt={path.split(/[\\/]/).pop() ?? "产物图片"} data-testid="artifact-image-view" />
    </div>
  );
}

function DocxView({ blob }: { blob: Blob }) {
  const [html, setHtml] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    setHtml(null);
    setLoadError(null);
    blob
      .arrayBuffer()
      .then((buffer) => mammoth.convertToHtml({ arrayBuffer: buffer }))
      .then((result) => {
        if (!cancelled) setHtml(result.value);
      })
      .catch((cause) => {
        if (!cancelled) setLoadError(String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [blob]);
  if (loadError) return <div className="artifact-empty artifact-error">DOCX 解析失败：{loadError}</div>;
  if (html === null) return <div className="artifact-empty">正在解析 DOCX…</div>;
  return (
    <div className="artifact-preview-scroll">
      <div
        className="artifact-preview-content artifact-docx-body"
        data-testid="artifact-docx-body"
        // mammoth output is sanitized library HTML, not raw model content.
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </div>
  );
}

function XlsxView({ blob }: { blob: Blob }) {
  const [rows, setRows] = useState<Row[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    setRows(null);
    setLoadError(null);
    readSheet(blob)
      .then((allRows) => {
        if (!cancelled) setRows(allRows.slice(0, 300));
      })
      .catch((cause: unknown) => {
        if (!cancelled) setLoadError(String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [blob]);
  if (loadError) return <div className="artifact-empty artifact-error">XLSX 解析失败：{loadError}</div>;
  if (rows === null) return <div className="artifact-empty">正在解析 XLSX…</div>;
  return (
    <div className="artifact-preview-scroll">
      <div className="artifact-table-wrap" data-testid="artifact-xlsx-table">
        <table className="artifact-table">
          <tbody>
            {rows.map((row, rowIndex) => (
              <tr key={rowIndex}>
                {row.map((cell, cellIndex) => (
                  <td key={cellIndex}>
                    {cell instanceof Date ? cell.toLocaleDateString() : String(cell ?? "")}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function PreviewBody({ artifact }: { artifact: ArtifactPreview }) {
  switch (artifact.kind) {
    case "pdf":
      return artifact.blob ? <PdfView blob={artifact.blob} /> : <div className="artifact-empty">缺少 PDF 数据</div>;
    case "docx":
      return artifact.blob ? <DocxView blob={artifact.blob} /> : <div className="artifact-empty">缺少 DOCX 数据</div>;
    case "xlsx":
      return artifact.blob ? <XlsxView blob={artifact.blob} /> : <div className="artifact-empty">缺少 XLSX 数据</div>;
    case "image":
      return artifact.blob ? <ImageView blob={artifact.blob} path={artifact.path} /> : <div className="artifact-empty">缺少图片数据</div>;
    case "csv":
      return artifact.content !== undefined ? <CsvView content={artifact.content} /> : <div className="artifact-empty">缺少表格数据</div>;
    case "markdown":
    case "text":
      return artifact.content !== undefined ? <MarkdownView content={artifact.content} /> : <div className="artifact-empty">缺少文档内容</div>;
    case "code":
      return artifact.content !== undefined ? <CodeView content={artifact.content} path={artifact.path} /> : <div className="artifact-empty">缺少代码内容</div>;
    default:
      return (
        <div className="artifact-empty artifact-error">
          <h3>暂不支持该格式预览</h3>
          <p>文件已登记在工作区产物库：{artifact.path}</p>
        </div>
      );
  }
}

export function ResearchDocumentCanvas({ artifact, loading, error, onBackToPlan }: ResearchDocumentCanvasProps) {
  const kindLabel = useMemo(
    () => (artifact ? previewKindLabel(artifact.kind) : "DOCUMENT"),
    [artifact],
  );

  return (
    <div className="doc-canvas artifact-canvas">
      <div className="doc-toolbar">
        <div className="doc-toolbar-left">
          <span className="artifact-live-dot" />
          <span className="artifact-kicker">GALEN INTERNAL PREVIEW</span>
          {artifact && <span className="galen-tag galen-tag-evidence">{kindLabel}</span>}
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
          <p>从科研证据流选择任一节点产物，即可在这里审阅 Markdown、PDF、DOCX、表格、图表与研究记录。</p>
        </div>
      ) : (
        <>
          <header className="artifact-titlebar">
            <span className="artifact-origin">来自 {artifact.nodeTitle || "科研节点"}</span>
            <h2>{artifact.path.split(/[\\/]/).pop()}</h2>
            <span className="artifact-path">{artifact.path}</span>
          </header>
          <PreviewBody artifact={artifact} />
          <footer className="doc-footer">
            <span className="doc-footer-stat">
              {artifact.content !== undefined
                ? `${artifact.content.length} 字符`
                : artifact.blob
                  ? `${(artifact.blob.size / 1024).toFixed(1)} KB`
                  : ""}
              {" · 工作区内预览"}
            </span>
            <span className="doc-footer-hint">无需打开外部编辑器</span>
          </footer>
        </>
      )}
    </div>
  );
}
