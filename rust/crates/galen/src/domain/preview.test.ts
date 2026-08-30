import { describe, expect, it } from "vitest";
import {
  BINARY_PREVIEW_KINDS,
  TEXT_PREVIEW_KINDS,
  classifyPreviewKind,
  codeLanguageOf,
  extensionOf,
  previewKindLabel,
} from "./preview";

describe("extensionOf", () => {
  it("handles forward and backslash paths and dotfiles", () => {
    expect(extensionOf("output/report.md")).toBe("md");
    expect(extensionOf("C:\\dir\\data.xlsx")).toBe("xlsx");
    expect(extensionOf("no-extension")).toBe("");
    expect(extensionOf(".hidden")).toBe("");
    expect(extensionOf("UPPER.PDF")).toBe("pdf");
  });
});

describe("classifyPreviewKind", () => {
  it("maps markdown and text formats", () => {
    expect(classifyPreviewKind("brief.md")).toBe("markdown");
    expect(classifyPreviewKind("notes.markdown")).toBe("markdown");
    expect(classifyPreviewKind("raw.txt")).toBe("text");
    expect(classifyPreviewKind("data.json")).toBe("text");
    expect(classifyPreviewKind("page.html")).toBe("text");
    expect(classifyPreviewKind("paper.typ")).toBe("text");
  });

  it("maps csv/tabular formats", () => {
    expect(classifyPreviewKind("table.csv")).toBe("csv");
    expect(classifyPreviewKind("table.tsv")).toBe("csv");
  });

  it("maps code formats for in-app audit", () => {
    expect(classifyPreviewKind("analysis.py")).toBe("code");
    expect(classifyPreviewKind("stats.R")).toBe("code");
    expect(classifyPreviewKind("run.js")).toBe("code");
    expect(classifyPreviewKind("main.rs")).toBe("code");
    expect(classifyPreviewKind("style.css")).toBe("code");
    expect(classifyPreviewKind("config.toml")).toBe("code");
    expect(codeLanguageOf("analysis.py")).toBe("python");
    expect(codeLanguageOf("main.rs")).toBe("rust");
    expect(codeLanguageOf("data.json")).toBe("json");
    expect(codeLanguageOf("unknown.xyz")).toBe("text");
  });

  it("maps binary report formats", () => {
    expect(classifyPreviewKind("brief.pdf")).toBe("pdf");
    expect(classifyPreviewKind("letter.docx")).toBe("docx");
    expect(classifyPreviewKind("cohort.xlsx")).toBe("xlsx");
    expect(classifyPreviewKind("cohort.xls")).toBe("xlsx");
  });

  it("maps image formats", () => {
    expect(classifyPreviewKind("fig.png")).toBe("image");
    expect(classifyPreviewKind("fig.JPG")).toBe("image");
    expect(classifyPreviewKind("fig.svg")).toBe("image");
    expect(classifyPreviewKind("fig.webp")).toBe("image");
  });

  it("falls back to other for unknown formats", () => {
    expect(classifyPreviewKind("run.py")).toBe("code");
    expect(classifyPreviewKind("archive.zip")).toBe("other");
    expect(classifyPreviewKind("script.bin")).toBe("other");
    expect(classifyPreviewKind("report.docx_")).toBe("other");
  });
});

describe("kind sets", () => {
  it("binary kinds must be disjoint from text kinds", () => {
    for (const kind of BINARY_PREVIEW_KINDS) {
      expect(TEXT_PREVIEW_KINDS.has(kind)).toBe(false);
    }
    expect(TEXT_PREVIEW_KINDS.has("markdown")).toBe(true);
    expect(BINARY_PREVIEW_KINDS.has("pdf")).toBe(true);
    expect(BINARY_PREVIEW_KINDS.has("docx")).toBe(true);
    expect(BINARY_PREVIEW_KINDS.has("xlsx")).toBe(true);
    expect(BINARY_PREVIEW_KINDS.has("image")).toBe(true);
  });
});

describe("previewKindLabel", () => {
  it("returns uppercase labels", () => {
    expect(previewKindLabel("pdf")).toBe("PDF");
    expect(previewKindLabel("docx")).toBe("DOCX");
    expect(previewKindLabel("markdown")).toBe("MARKDOWN");
    expect(previewKindLabel("other")).toBe("DOCUMENT");
  });
});
