import type { FileEntry } from "../types";

// ---------------------------------------------------------------------------
// Project type detection
// ---------------------------------------------------------------------------

export type ProjectKind = "clinical" | "software" | "research" | "generic";

export interface ProjectIdentity {
  kind: ProjectKind;
  name: string;
  root: string | null;
  summary: string;
}

// ---------------------------------------------------------------------------
// Artifact classification (cross-domain)
// ---------------------------------------------------------------------------

export type ArtifactKind =
  | "source"
  | "config"
  | "dependency"
  | "test"
  | "doc"
  | "data"
  | "script"
  | "output"
  | "directory"
  | "other";

export interface ClassifiedEntry {
  entry: FileEntry;
  kind: ArtifactKind;
}

// ---------------------------------------------------------------------------
// Workflow stage (domain-agnostic)
// ---------------------------------------------------------------------------

export interface WorkflowStage {
  title: string;
  state: "ready" | "incomplete";
  detail: string;
  prompt: string;
}

// ---------------------------------------------------------------------------
// Domain capability — what a domain module provides
// ---------------------------------------------------------------------------

export interface DomainCapability {
  /** Identify whether this domain applies to the given workspace. */
  detect: (entries: FileEntry[]) => boolean;
  /** Return workflow stages for this domain. */
  getStages: (entries: FileEntry[]) => WorkflowStage[];
  /** Return agent task suggestions. */
  getAgentTasks: (entries: FileEntry[]) => { label: string; prompt: string }[];
  /** Classify a single entry. */
  classifyEntry: (entry: FileEntry) => ArtifactKind;
  /** Human-readable label for an artifact kind. */
  artifactLabel: (kind: ArtifactKind) => string;
}

// ---------------------------------------------------------------------------
// Shared file-classification constants
// ---------------------------------------------------------------------------

export const SOURCE_EXTENSIONS = new Set([
  "rs", "py", "ts", "tsx", "js", "jsx", "go", "java", "c", "cpp", "h", "hpp",
  "swift", "kt", "scala", "rb", "php", "cs", "fs", "hs", "elm", "vue", "svelte",
]);

export const CONFIG_EXTENSIONS = new Set([
  "json", "toml", "yaml", "yml", "ini", "cfg", "conf", "env",
]);

export const DOC_EXTENSIONS = new Set([
  "md", "mdx", "rst", "txt", "pdf", "docx", "rtf",
]);

export const DATA_EXTENSIONS = new Set([
  "csv", "tsv", "xlsx", "xls", "sav", "dta", "parquet", "feather", "sqlite", "db",
]);

export const SCRIPT_EXTENSIONS = new Set([
  "sh", "bash", "zsh", "ps1", "bat", "cmd",
]);

export const ANALYSIS_EXTENSIONS = new Set([
  "r", "rmd", "ipynb", "sql", "do",
]);

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

export function getExtension(name: string): string {
  const parts = name.split(".");
  return parts.length > 1 ? parts.pop()?.toLowerCase() || "" : "";
}

export function getBaseName(path: string): string {
  const normalized = path.replace(/[/\\]+$/, "");
  return normalized.split(/[/\\]/).pop() || normalized;
}

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function summarizeNames(items: ClassifiedEntry[], max = 3): string {
  return items.length > 0
    ? items.slice(0, max).map(({ entry }) => entry.name).join("、")
    : "未发现";
}
