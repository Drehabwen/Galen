import { useEffect, useMemo, useRef, useState } from "react";
import { getDocument, GlobalWorkerOptions, type PDFDocumentProxy } from "pdfjs-dist/legacy/build/pdf.mjs";
import pdfWorker from "pdfjs-dist/build/pdf.worker.min.mjs?url";

GlobalWorkerOptions.workerSrc = pdfWorker;

export function PdfView({ blob }: { blob: Blob }) {
  const pdfBlob = useMemo(
    () => blob.type === "application/pdf" ? blob : blob.slice(0, blob.size, "application/pdf"),
    [blob],
  );
  const [document, setDocument] = useState<PDFDocumentProxy | null>(null);
  const [numPages, setNumPages] = useState(0);
  const [currentPage, setCurrentPage] = useState(1);
  const [loadError, setLoadError] = useState<string | null>(null);
  const canvasRefs = useRef<Record<number, HTMLCanvasElement | null>>({});
  const scrollRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    let cancelled = false;
    setDocument(null);
    setNumPages(0);
    setCurrentPage(1);
    setLoadError(null);
    pdfBlob
      .arrayBuffer()
      .then((data) => getDocument({ data }).promise)
      .then((loaded) => {
        if (cancelled) {
          void loaded.destroy();
          return;
        }
        setDocument(loaded);
        setNumPages(loaded.numPages);
      })
      .catch((cause) => {
        if (!cancelled) setLoadError(String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [pdfBlob]);

  useEffect(() => {
    if (!document || numPages === 0) return;
    let cancelled = false;
    const renderPages = async () => {
      try {
        for (let pageNumber = 1; pageNumber <= numPages; pageNumber += 1) {
          const page = await document.getPage(pageNumber);
          if (cancelled) return;
          const canvas = canvasRefs.current[pageNumber];
          const context = canvas?.getContext("2d");
          if (!canvas || !context) continue;
          const viewport = page.getViewport({ scale: 1.2 });
          canvas.width = viewport.width;
          canvas.height = viewport.height;
          await page.render({ canvasContext: context, viewport }).promise;
        }
      } catch (cause) {
        if (!cancelled) setLoadError(String(cause));
      }
    };
    void renderPages();
    return () => {
      cancelled = true;
    };
  }, [document, numPages]);

  const jumpToPage = (page: number) => {
    const target = Math.min(Math.max(page, 1), numPages);
    setCurrentPage(target);
    const container = scrollRef.current;
    const pageElement = canvasRefs.current[target]?.closest<HTMLElement>(".artifact-pdf-page");
    if (container && pageElement && typeof container.scrollTo === "function") {
      container.scrollTo({ top: Math.max(pageElement.offsetTop - 12, 0), behavior: "smooth" });
      return;
    }
    canvasRefs.current[target]?.scrollIntoView?.({ behavior: "smooth", block: "start" });
  };

  const syncPageFromScroll = () => {
    const container = scrollRef.current;
    if (!container || numPages === 0) return;
    const marker = container.scrollTop + 24;
    let nearestPage = 1;
    let nearestDistance = Number.POSITIVE_INFINITY;
    for (let pageNumber = 1; pageNumber <= numPages; pageNumber += 1) {
      const pageElement = canvasRefs.current[pageNumber]?.closest<HTMLElement>(".artifact-pdf-page");
      if (!pageElement) continue;
      const distance = Math.abs(pageElement.offsetTop - marker);
      if (distance < nearestDistance) {
        nearestDistance = distance;
        nearestPage = pageNumber;
      }
    }
    setCurrentPage((current) => current === nearestPage ? current : nearestPage);
  };

  if (loadError) return <div className="artifact-empty artifact-error">PDF 解析失败：{loadError}</div>;
  if (!document) return <div className="artifact-empty">正在解析 PDF…</div>;
  return (
    <div className="artifact-pdf-viewer">
      <div className="artifact-pdf-controls" aria-label="PDF 页码控制">
        <button type="button" className="btn btn-ghost btn-sm" onClick={() => jumpToPage(currentPage - 1)} disabled={currentPage <= 1}>上一页</button>
        <span>第 {currentPage} / {numPages} 页</span>
        <button type="button" className="btn btn-ghost btn-sm" onClick={() => jumpToPage(currentPage + 1)} disabled={currentPage >= numPages}>下一页</button>
      </div>
      <div ref={scrollRef} className="artifact-preview-scroll artifact-pdf-pages" data-testid="artifact-pdf-document" onScroll={syncPageFromScroll}>
        {Array.from({ length: numPages }, (_, index) => {
          const pageNumber = index + 1;
          return (
            <div className="artifact-pdf-page" key={pageNumber}>
              <span className="artifact-pdf-page-label">第 {pageNumber} 页</span>
              <canvas ref={(canvas) => { canvasRefs.current[pageNumber] = canvas; }} aria-label={`PDF 第 ${pageNumber} 页`} />
            </div>
          );
        })}
      </div>
    </div>
  );
}
