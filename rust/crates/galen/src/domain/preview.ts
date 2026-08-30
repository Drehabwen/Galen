/**
 * Preview-kind classification for workspace artifacts.
 *
 * The delivery contract registers every artifact with a kind/mime type, but the
 * in-app preview must decide HOW to render a file. This module is the single
 * source of truth for that mapping so the canvas, the read path and the tests
 * agree on the same rules.
 */
export type PreviewKind =
  | "markdown"
  | "text"
  | "csv"
  | "code"
  | "pdf"
  | "docx"
  | "xlsx"
  | "image"
  | "other";

const MARKDOWN = new Set(["md", "markdown"]);
const TEXT = new Set(["txt", "json", "html", "typ"]);
const CSV = new Set(["csv", "tsv"]);
const CODE = new Set([
  "py", "r", "js", "jsx", "ts", "tsx", "rs", "go", "java", "c", "cpp", "h", "hpp",
  "sh", "bash", "ps1", "css", "scss", "sql", "toml", "yaml", "yml", "xml",
]);
const PDF = new Set(["pdf"]);
const DOCX = new Set(["docx"]);
const XLSX = new Set(["xlsx", "xls"]);
const IMAGE = new Set(["png", "jpg", "jpeg", "svg", "webp", "gif"]);

/** Binary formats that must be read through `read_artifact_bytes` (ArrayBuffer). */
export const BINARY_PREVIEW_KINDS: ReadonlySet<PreviewKind> = new Set([
  "pdf",
  "docx",
  "xlsx",
  "image",
]);

/** Kinds that can be displayed directly as text/markdown content. */
export const TEXT_PREVIEW_KINDS: ReadonlySet<PreviewKind> = new Set([
  "markdown",
  "text",
  "csv",
  "code",
]);

export function extensionOf(path: string): string {
  const base = path.split(/[\\/]/).pop() ?? "";
  const dot = base.lastIndexOf(".");
  return dot <= 0 ? "" : base.slice(dot + 1).toLowerCase();
}

export function classifyPreviewKind(path: string): PreviewKind {
  const ext = extensionOf(path);
  if (MARKDOWN.has(ext)) return "markdown";
  if (TEXT.has(ext)) return "text";
  if (CSV.has(ext)) return "csv";
  if (CODE.has(ext)) return "code";
  if (PDF.has(ext)) return "pdf";
  if (DOCX.has(ext)) return "docx";
  if (XLSX.has(ext)) return "xlsx";
  if (IMAGE.has(ext)) return "image";
  return "other";
}

const PRISM_LANGUAGES: Record<string, string> = {
  py: "python",
  r: "r",
  js: "javascript",
  jsx: "jsx",
  ts: "typescript",
  tsx: "tsx",
  rs: "rust",
  go: "go",
  java: "java",
  c: "c",
  cpp: "cpp",
  h: "c",
  hpp: "cpp",
  sh: "bash",
  bash: "bash",
  ps1: "powershell",
  css: "css",
  scss: "scss",
  sql: "sql",
  toml: "toml",
  yaml: "yaml",
  yml: "yaml",
  xml: "xml",
  json: "json",
};

/** Prism language id for a code artifact path (used by the code viewer). */
export function codeLanguageOf(path: string): string {
  return PRISM_LANGUAGES[extensionOf(path)] ?? "text";
}

/** Human-readable label used in the canvas title bar. */
export function previewKindLabel(kind: PreviewKind): string {
  switch (kind) {
    case "markdown":
      return "MARKDOWN";
    case "text":
      return "TEXT";
    case "csv":
      return "CSV";
    case "code":
      return "CODE";
    case "pdf":
      return "PDF";
    case "docx":
      return "DOCX";
    case "xlsx":
      return "XLSX";
    case "image":
      return "IMAGE";
    default:
      return "DOCUMENT";
  }
}

/**
 * The resolved preview payload handed from the read path to the canvas.
 * Text kinds carry `content`; binary kinds carry `blob` (from
 * `read_artifact_bytes`). Exactly one of the two is set per kind.
 */
export interface ArtifactPreview {
  path: string;
  kind: PreviewKind;
  content?: string;
  blob?: Blob;
  nodeTitle?: string;
}
