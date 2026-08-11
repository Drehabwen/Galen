import type { FileEntry } from "../types";
import type { ProjectIdentity, ProjectKind, WorkflowStage } from "./types";
import { detectProjectKind, describeProject, getSoftwareStages, getSoftwareAgentTasks } from "./project";
import { getClinicalStages, getClinicalAgentTasks, getClinicalMetrics } from "./clinical";
import { classifyFile, classifyEntries, artifactTypeLabel } from "./classifier";
import { formatSize, getBaseName, getExtension, summarizeNames } from "./types";

// ---------------------------------------------------------------------------
// Domain registry — loads the appropriate domain for a workspace
// ---------------------------------------------------------------------------

export interface ActiveDomain {
  identity: ProjectIdentity;
  stages: WorkflowStage[];
  agentTasks: { label: string; prompt: string }[];
}

export function resolveDomain(wsRoot: string | null, files: FileEntry[]): ActiveDomain {
  const entries = [...files].sort(
    (a, b) => Number(b.is_dir) - Number(a.is_dir) || a.name.localeCompare(b.name),
  );
  const identity = describeProject(wsRoot, entries);

  switch (identity.kind) {
    case "clinical":
      return {
        identity,
        stages: getClinicalStages(entries),
        agentTasks: getClinicalAgentTasks(entries),
      };
    case "software":
      return {
        identity,
        stages: getSoftwareStages(entries),
        agentTasks: getSoftwareAgentTasks(entries),
      };
    default:
      return {
        identity,
        stages: getSoftwareStages(entries),
        agentTasks: getSoftwareAgentTasks(entries),
      };
  }
}

// Re-export everything for convenience
export {
  detectProjectKind,
  describeProject,
  classifyFile,
  classifyEntries,
  artifactTypeLabel,
  formatSize,
  getBaseName,
  getExtension,
  summarizeNames,
  getClinicalStages,
  getClinicalAgentTasks,
  getClinicalMetrics,
  getSoftwareStages,
  getSoftwareAgentTasks,
};
export type { ProjectKind, ProjectIdentity, WorkflowStage };
