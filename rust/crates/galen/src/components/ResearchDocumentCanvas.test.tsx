// @vitest-environment jsdom
import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ResearchDocumentCanvas } from "./ResearchDocumentCanvas";
import type { ArtifactPreview } from "../domain/preview";
import { codeLanguageOf } from "../domain/preview";

vi.mock("mammoth", () => ({
  default: {
    convertToHtml: vi.fn(async () => ({ value: "<p>DOCX_BODY</p>", messages: [] })),
  },
}));

vi.mock("read-excel-file/browser", () => ({
  readSheet: vi.fn(async () => [
    ["患者", "6MWT (m)"],
    ["A", 350],
    ["B", 402],
  ]),
}));

const renderPage = vi.fn(() => ({ promise: Promise.resolve() }));
const getPage = vi.fn(async () => ({
  getViewport: ({ scale }: { scale: number }) => ({ width: 300 * scale, height: 400 * scale }),
  render: renderPage,
}));

vi.mock("pdfjs-dist/legacy/build/pdf.mjs", () => ({
  GlobalWorkerOptions: { workerSrc: "" },
  getDocument: vi.fn(() => ({
    promise: Promise.resolve({ numPages: 2, getPage }),
  })),
}));

function preview(partial: Partial<ArtifactPreview>): ArtifactPreview {
  return { path: "output/result.md", kind: "markdown", content: "", ...partial };
}

describe("ResearchDocumentCanvas preview dispatch", () => {
  afterEach(() => cleanup());
  beforeEach(() => {
    // jsdom lacks object URL support; stub it for image viewers.
    globalThis.URL.createObjectURL = vi.fn(() => "blob:mock-object-url");
    globalThis.URL.revokeObjectURL = vi.fn();
    HTMLCanvasElement.prototype.getContext = vi.fn(() => ({})) as unknown as typeof HTMLCanvasElement.prototype.getContext;
    // jsdom 23 Blob lacks arrayBuffer(); polyfill via FileReader for DOCX/XLSX.
    if (typeof Blob.prototype.arrayBuffer !== "function") {
      Blob.prototype.arrayBuffer = function arrayBuffer() {
        return new Promise<ArrayBuffer>((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = () => resolve(reader.result as ArrayBuffer);
          reader.onerror = () => reject(reader.error);
          reader.readAsArrayBuffer(this);
        });
      };
    }
  });

  it("renders markdown content through ReactMarkdown", () => {
    render(
      <ResearchDocumentCanvas
        artifact={preview({ path: "output/brief.md", kind: "markdown", content: "# 标题\n\n正文" })}
      />,
    );
    const article = screen.getByTestId("artifact-rendered-preview");
    expect(article.querySelector("h1")?.textContent).toBe("标题");
    expect(article.parentElement?.classList.contains("artifact-preview-scroll")).toBe(true);
  });

  it("renders plain text through ReactMarkdown", () => {
    render(
      <ResearchDocumentCanvas
        artifact={preview({ path: "output/log.txt", kind: "text", content: "plain text" })}
      />,
    );
    expect(screen.getByTestId("artifact-rendered-preview").textContent).toContain("plain text");
  });

  it("renders CSV as a table", () => {
    render(
      <ResearchDocumentCanvas
        artifact={preview({ path: "output/data.csv", kind: "csv", content: "a,b\n1,2" })}
      />,
    );
    const table = screen.getByTestId("artifact-csv-table");
    expect(table.textContent).toContain("a");
    expect(table.textContent).toContain("2");
  });

  it("renders PDF pages through PDF.js inside the app", async () => {
    render(
      <ResearchDocumentCanvas
        artifact={preview({ path: "output/brief.pdf", kind: "pdf", blob: new Blob(["%PDF"]) })}
      />,
    );
    const document = await screen.findByTestId("artifact-pdf-document");
    expect(document.classList.contains("artifact-preview-scroll")).toBe(true);
    expect(document.querySelectorAll("canvas")).toHaveLength(2);
    expect(screen.getByText("第 1 / 2 页")).toBeTruthy();
    expect(renderPage).toHaveBeenCalledTimes(2);
    fireEvent.click(screen.getByRole("button", { name: "下一页" }));
    expect(screen.getByText("第 2 / 2 页")).toBeTruthy();

    const pages = document.querySelectorAll<HTMLElement>(".artifact-pdf-page");
    Object.defineProperty(pages[0], "offsetTop", { configurable: true, value: 0 });
    Object.defineProperty(pages[1], "offsetTop", { configurable: true, value: 500 });
    Object.defineProperty(document, "scrollTop", { configurable: true, value: 0 });
    fireEvent.scroll(document);
    expect(screen.getByText("第 1 / 2 页")).toBeTruthy();
  });

  it("renders images through an img tag", () => {
    render(
      <ResearchDocumentCanvas
        artifact={preview({ path: "output/fig.png", kind: "image", blob: new Blob([new Uint8Array([1])]) })}
      />,
    );
    const img = screen.getByTestId("artifact-image-view") as HTMLImageElement;
    expect(img.src).toBe("blob:mock-object-url");
  });

  it("parses DOCX via mammoth and injects sanitized HTML", async () => {
    render(
      <ResearchDocumentCanvas
        artifact={preview({ path: "output/letter.docx", kind: "docx", blob: new Blob(["docx-bytes"]) })}
      />,
    );
    await waitFor(() => expect(screen.getByTestId("artifact-docx-body").innerHTML).toContain("DOCX_BODY"));
  });

  it("parses XLSX via readSheet and renders rows", async () => {
    render(
      <ResearchDocumentCanvas
        artifact={preview({ path: "output/cohort.xlsx", kind: "xlsx", blob: new Blob(["xlsx-bytes"]) })}
      />,
    );
    const table = await screen.findByTestId("artifact-xlsx-table");
    expect(table.textContent).toContain("6MWT");
    expect(table.textContent).toContain("402");
  });

  it("renders code artifacts with syntax highlighting", () => {
    render(
      <ResearchDocumentCanvas
        artifact={preview({ path: "output/analysis.py", kind: "code", content: "import pandas as pd\nprint('ok')\n" })}
      />,
    );
    const view = screen.getByTestId("artifact-code-view");
    expect(view.textContent).toContain("import pandas as pd");
    expect(codeLanguageOf("output/analysis.py")).toBe("python");
  });

  it("shows an unsupported message for other kinds", () => {
    render(
      <ResearchDocumentCanvas
        artifact={preview({ path: "output/run.py", kind: "other" })}
      />,
    );
    expect(screen.getByText("暂不支持该格式预览")).toBeTruthy();
  });

  it("shows loading and error states", () => {
    const { rerender } = render(<ResearchDocumentCanvas loading />);
    expect(screen.getByText("正在载入产物")).toBeTruthy();
    rerender(<ResearchDocumentCanvas error="读取出错" />);
    expect(screen.getByText("暂时无法预览")).toBeTruthy();
    expect(screen.getByText("读取出错")).toBeTruthy();
    rerender(<ResearchDocumentCanvas />);
    expect(screen.getByText("产物在 Galen 内直接展开")).toBeTruthy();
  });
});
