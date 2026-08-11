import type { FileEntry } from "../types";
import type { ProjectIdentity, ProjectKind, WorkflowStage } from "./types";
import { classifyEntries } from "./classifier";
import { getExtension, summarizeNames } from "./types";

// ---------------------------------------------------------------------------
// Project detection
// ---------------------------------------------------------------------------

export function detectProjectKind(entries: FileEntry[]): ProjectKind {
  const names = new Set(entries.map((e) => e.name.toLowerCase()));

  // Clinical study signals
  const clinicalSignals = [
    "protocol", "data", "analysis", "manuscript", "literature", "output",
  ];
  const clinicalDirCount = entries
    .filter((e) => e.is_dir && clinicalSignals.includes(e.name.toLowerCase()))
    .length;
  if (clinicalDirCount >= 2) return "clinical";

  // Software project signals
  const softwareSignals = [
    "cargo.toml", "package.json", "go.mod", "pom.xml", "build.gradle",
    "cmakelists.txt", "setup.py", "pyproject.toml", "makefile",
    ".git", "src", "tests", "node_modules", "target",
  ];
  const softwareMatch = softwareSignals.filter((s) => names.has(s));
  if (softwareMatch.length >= 1) return "software";

  // Research project signals
  const researchSignals = ["data", "analysis", "manuscript", "literature", "notes", "figures"];
  const researchDirCount = entries
    .filter((e) => e.is_dir && researchSignals.includes(e.name.toLowerCase()))
    .length;
  if (researchDirCount >= 2) return "research";

  return "generic";
}

export function describeProject(root: string | null, entries: FileEntry[]): ProjectIdentity {
  const name = root ? root.split(/[/\\]/).pop() || root : "未选择项目";
  const kind = detectProjectKind(entries);

  const classified = classifyEntries(entries);
  const fileCount = classified.filter((c) => c.entry.is_dir === false).length;
  const dirCount = classified.filter((c) => c.entry.is_dir).length;
  const sourceCount = classified.filter((c) => c.kind === "source").length;
  const dataCount = classified.filter((c) => c.kind === "data").length;
  const totalSize = classified.reduce((sum, c) => sum + c.entry.size, 0);

  const parts: string[] = [];
  if (sourceCount > 0) parts.push(`${sourceCount} 源文件`);
  if (dataCount > 0) parts.push(`${dataCount} 数据`);
  parts.push(`${fileCount} 文件`);
  parts.push(`${dirCount} 目录`);

  const summaries: Record<ProjectKind, string> = {
    clinical: `临床课题 · ${parts.join(" · ")}`,
    software: `软件项目 · ${parts.join(" · ")}`,
    research: `研究项目 · ${parts.join(" · ")}`,
    generic: `通用工作区 · ${parts.join(" · ")}`,
  };

  return { kind, name, root, summary: summaries[kind] };
}

// ---------------------------------------------------------------------------
// Software project analysis — detect languages, build system, structure
// ---------------------------------------------------------------------------

export interface SoftwareAnalysis {
  languages: string[];
  buildSystem: string | null;
  testFramework: string | null;
  hasDependencies: boolean;
  entryPoints: string[];
}

export function analyzeSoftwareProject(entries: FileEntry[]): SoftwareAnalysis {
  const names = new Set(entries.map((e) => e.name.toLowerCase()));
  const exts = new Set(
    entries.filter((e) => !e.is_dir).map((e) => getExtension(e.name)),
  );

  // Language detection
  const languages: string[] = [];
  if (exts.has("rs")) languages.push("Rust");
  if (exts.has("py")) languages.push("Python");
  if (exts.has("ts") || exts.has("tsx")) languages.push("TypeScript");
  if (exts.has("js") || exts.has("jsx")) languages.push("JavaScript");
  if (exts.has("go")) languages.push("Go");
  if (exts.has("java")) languages.push("Java");
  if (exts.has("r") || exts.has("rmd")) languages.push("R");
  if (exts.has("cpp") || exts.has("c") || exts.has("h")) languages.push("C/C++");

  // Build system
  let buildSystem: string | null = null;
  if (names.has("cargo.toml")) buildSystem = "Cargo";
  else if (names.has("package.json")) buildSystem = "npm/Node";
  else if (names.has("go.mod")) buildSystem = "Go Modules";
  else if (names.has("pyproject.toml")) buildSystem = "Python (pyproject)";
  else if (names.has("setup.py")) buildSystem = "Python (setuptools)";
  else if (names.has("makefile")) buildSystem = "Make";
  else if (names.has("cmakelists.txt")) buildSystem = "CMake";

  // Test framework
  let testFramework: string | null = null;
  const hasTestDir = entries.some((e) => e.is_dir && e.name === "tests" || e.name === "__tests__" || e.name === "spec");
  if (hasTestDir) {
    if (exts.has("rs")) testFramework = "cargo test";
    else if (exts.has("py")) testFramework = "pytest";
    else if (exts.has("ts") || exts.has("js")) testFramework = "jest/vitest";
  }

  // Entry points
  const entryPoints = entries
    .filter((e) =>
      e.name === "main.rs" || e.name === "main.ts" || e.name === "main.py" ||
      e.name === "index.ts" || e.name === "index.tsx" || e.name === "lib.rs",
    )
    .map((e) => e.name);

  return {
    languages,
    buildSystem,
    testFramework,
    hasDependencies: names.has("cargo.toml") || names.has("package.json") || names.has("go.mod"),
    entryPoints,
  };
}

// ---------------------------------------------------------------------------
// Generic software workflow stages
// ---------------------------------------------------------------------------

export function getSoftwareStages(entries: FileEntry[]): WorkflowStage[] {
  const analysis = analyzeSoftwareProject(entries);
  const classified = classifyEntries(entries);
  const sourceFiles = classified.filter((c) => c.kind === "source");
  const testFiles = classified.filter((c) => c.kind === "test");
  const configFiles = classified.filter((c) => c.kind === "config");

  return [
    {
      title: "项目结构",
      state: configFiles.length > 0 ? "ready" : "incomplete",
      detail: analysis.buildSystem
        ? `${analysis.languages.join("+")} · ${analysis.buildSystem}`
        : "未检测到构建系统",
      prompt: "请分析当前项目的目录结构，给出架构概览和关键模块说明。",
    },
    {
      title: "代码质量",
      state: sourceFiles.length > 0 ? "ready" : "incomplete",
      detail: sourceFiles.length > 0
        ? `${sourceFiles.length} 源文件可审查`
        : "无源文件",
      prompt: "请审查当前项目的代码质量：命名规范、模块划分、错误处理、潜在问题。",
    },
    {
      title: "测试覆盖",
      state: testFiles.length > 0 ? "ready" : "incomplete",
      detail: testFiles.length > 0
        ? `${testFiles.length} 测试文件 · ${analysis.testFramework || "未知框架"}`
        : "未发现测试",
      prompt: "请检查测试覆盖情况，指出缺少测试的关键模块，并生成测试用例。",
    },
    {
      title: "依赖分析",
      state: analysis.hasDependencies ? "ready" : "incomplete",
      detail: analysis.hasDependencies
        ? "可检查依赖版本和安全性"
        : "无依赖声明",
      prompt: "请分析项目依赖：版本是否过时、是否有已知漏洞、是否可以精简。",
    },
    {
      title: "文档与部署",
      state: "ready",
      detail: "可生成 README、API 文档、构建说明",
      prompt: "请为当前项目生成或更新文档：README、构建说明、API 概览。",
    },
  ];
}

export function getSoftwareAgentTasks(entries: FileEntry[]): { label: string; prompt: string }[] {
  const analysis = analyzeSoftwareProject(entries);

  return [
    {
      label: "代码审查",
      prompt: "请审查当前项目代码，指出：架构问题、重复代码、错误处理缺失、性能瓶颈。",
    },
    {
      label: analysis.testFramework ? "运行测试" : "添加测试",
      prompt: analysis.testFramework
        ? `请运行 ${analysis.testFramework} 并分析测试结果。`
        : "请为关键模块生成测试用例。",
    },
    {
      label: "重构建议",
      prompt: "请分析代码结构，给出 3-5 个最值得做的重构建议，按优先级排列。",
    },
    {
      label: "生成文档",
      prompt: "请生成项目 README，包含：项目概述、架构说明、构建运行指南、API 概览。",
    },
    {
      label: "依赖审查",
      prompt: "请检查依赖是否有已知漏洞，建议升级或替换的方案。",
    },
    {
      label: "性能分析",
      prompt: "请分析代码中可能的性能问题：不必要的分配、同步阻塞、N+1 查询等。",
    },
  ];
}
