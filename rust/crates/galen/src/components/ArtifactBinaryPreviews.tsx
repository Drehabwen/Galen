import { useEffect, useState } from "react";
import mammoth from "mammoth";
import { readSheet } from "read-excel-file/browser";
import type { Row } from "read-excel-file/browser";

function sanitizeDocumentHtml(html: string): string {
  const document = new DOMParser().parseFromString(html, "text/html");
  document.querySelectorAll("script, style, iframe, object, embed, link, meta, base, form, input, button").forEach((node) => node.remove());
  document.body.querySelectorAll("*").forEach((element) => {
    for (const attribute of Array.from(element.attributes)) {
      const name = attribute.name.toLowerCase();
      const value = attribute.value.trim().toLowerCase();
      if (name.startsWith("on") || name === "style" || ((name === "href" || name === "src") && value.startsWith("javascript:"))) {
        element.removeAttribute(attribute.name);
      }
    }
  });
  return document.body.innerHTML;
}

export function DocxView({ blob }: { blob: Blob }) {
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
        if (!cancelled) setHtml(sanitizeDocumentHtml(result.value));
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
      <div className="artifact-preview-content artifact-docx-body" data-testid="artifact-docx-body" dangerouslySetInnerHTML={{ __html: html }} />
    </div>
  );
}

export function XlsxView({ blob }: { blob: Blob }) {
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
                {row.map((cell, cellIndex) => <td key={cellIndex}>{cell instanceof Date ? cell.toLocaleDateString() : String(cell ?? "")}</td>)}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
