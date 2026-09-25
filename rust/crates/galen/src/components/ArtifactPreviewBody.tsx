import { lazy, Suspense, type ReactNode } from "react";
import type { ArtifactPreview } from "../domain/preview";
import { CsvView, MarkdownView } from "./ArtifactTextPreviews";
import { ImageView } from "./ArtifactImagePreview";

const PdfView = lazy(() => import("./ArtifactPdfPreview").then((module) => ({ default: module.PdfView })));
const CodeView = lazy(() => import("./ArtifactCodePreview").then((module) => ({ default: module.CodeView })));
const DocxView = lazy(() => import("./ArtifactBinaryPreviews").then((module) => ({ default: module.DocxView })));
const XlsxView = lazy(() => import("./ArtifactBinaryPreviews").then((module) => ({ default: module.XlsxView })));

function DeferredPreview({ children }: { children: ReactNode }) {
  return <Suspense fallback={<div className="artifact-empty">正在加载预览器…</div>}>{children}</Suspense>;
}

export function ArtifactPreviewBody({ artifact }: { artifact: ArtifactPreview }) {
  switch (artifact.kind) {
    case "pdf":
      return artifact.blob ? <DeferredPreview><PdfView blob={artifact.blob} /></DeferredPreview> : <div className="artifact-empty">缺少 PDF 数据</div>;
    case "docx":
      return artifact.blob ? <DeferredPreview><DocxView blob={artifact.blob} /></DeferredPreview> : <div className="artifact-empty">缺少 DOCX 数据</div>;
    case "xlsx":
      return artifact.blob ? <DeferredPreview><XlsxView blob={artifact.blob} /></DeferredPreview> : <div className="artifact-empty">缺少 XLSX 数据</div>;
    case "image":
      return artifact.blob ? <ImageView blob={artifact.blob} path={artifact.path} /> : <div className="artifact-empty">缺少图片数据</div>;
    case "csv":
      return artifact.content !== undefined ? <CsvView content={artifact.content} /> : <div className="artifact-empty">缺少表格数据</div>;
    case "markdown":
    case "text":
      return artifact.content !== undefined ? <MarkdownView content={artifact.content} /> : <div className="artifact-empty">缺少文档内容</div>;
    case "code":
      return artifact.content !== undefined ? <DeferredPreview><CodeView content={artifact.content} path={artifact.path} /></DeferredPreview> : <div className="artifact-empty">缺少代码内容</div>;
    default:
      return (
        <div className="artifact-empty artifact-error">
          <h3>暂不支持该格式预览</h3>
          <p>文件已登记在工作区产物库：{artifact.path}</p>
        </div>
      );
  }
}
