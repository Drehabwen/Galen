import type { FileEntry } from "../types";
import {
  type ArtifactKind,
  type ClassifiedEntry,
  SOURCE_EXTENSIONS,
  CONFIG_EXTENSIONS,
  DOC_EXTENSIONS,
  DATA_EXTENSIONS,
  SCRIPT_EXTENSIONS,
  ANALYSIS_EXTENSIONS,
  getExtension,
} from "./types";

// ---------------------------------------------------------------------------
// Generic file classifier — no domain knowledge, just file types
// ---------------------------------------------------------------------------

export function classifyFile(entry: FileEntry): ArtifactKind {
  if (entry.is_dir) return "directory";

  const ext = getExtension(entry.name);
  const name = entry.name.toLowerCase();

  // Tests
  if (name.includes(".test.") || name.includes(".spec.") || name.startsWith("test_")) return "test";

  // Config / lock files
  if (CONFIG_EXTENSIONS.has(ext)) return "config";
  if (name === "makefile" || name === "dockerfile" || name === "containerfile") return "config";
  if (name.endsWith(".lock") || name.startsWith(".git")) return "config";

  // Source code
  if (SOURCE_EXTENSIONS.has(ext)) return "source";

  // Scripts
  if (SCRIPT_EXTENSIONS.has(ext)) return "script";

  // Data / analysis
  if (DATA_EXTENSIONS.has(ext)) return "data";
  if (ANALYSIS_EXTENSIONS.has(ext)) return "script";

  // Documents
  if (DOC_EXTENSIONS.has(ext)) return "doc";

  // Output / build artifacts
  if (["o", "obj", "class", "pyc", "exe", "dll", "so", "dylib", "wasm"].includes(ext)) return "output";

  return "other";
}

export function classifyEntries(entries: FileEntry[]): ClassifiedEntry[] {
  return entries.map((entry) => ({ entry, kind: classifyFile(entry) }));
}

export function artifactTypeLabel(kind: ArtifactKind): string {
  const labels: Record<ArtifactKind, string> = {
    source: "源码",
    config: "配置",
    dependency: "依赖",
    test: "测试",
    doc: "文档",
    data: "数据",
    script: "脚本",
    output: "产物",
    directory: "目录",
    other: "其他",
  };
  return labels[kind];
}
