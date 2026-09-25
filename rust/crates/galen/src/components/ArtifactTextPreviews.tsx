import { useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

export function MarkdownView({ content }: { content: string }) {
  return (
    <div className="artifact-preview-scroll">
      <article className="artifact-preview-content" data-testid="artifact-rendered-preview">
        <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
      </article>
    </div>
  );
}

export function CsvView({ content }: { content: string }) {
  const rows = useMemo(() => content
    .split(/\r?\n/)
    .filter((line) => line.trim().length > 0)
    .slice(0, 500)
    .map((line) => line.split(/[,\t]/)), [content]);

  return (
    <div className="artifact-preview-scroll">
      <div className="artifact-table-wrap" data-testid="artifact-csv-table">
        <table className="artifact-table">
          <tbody>
            {rows.map((row, index) => (
              <tr key={index}>
                {row.map((cell, cellIndex) => <td key={cellIndex}>{cell}</td>)}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
