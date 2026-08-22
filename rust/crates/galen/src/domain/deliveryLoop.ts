import type { ArtifactRecord } from "./artifact";
import type { ResearchTask, ResearchTaskStatus } from "./researchTask";
import type { SessionNode } from "./sessionTypes";

export function mergeDeliveredArtifact(
  current: ArtifactRecord[],
  artifact: ArtifactRecord,
): ArtifactRecord[] {
  return [artifact, ...current.filter((item) => item.id !== artifact.id)];
}

export function resolveArtifactNodeTitle(
  artifact: ArtifactRecord,
  task: ResearchTask | null | undefined,
): string | undefined {
  return task?.nodes.find((node) => node.id === artifact.nodeId)?.title;
}

export function shouldSynthesizeCompletion(
  confirmed: boolean,
  nodes: SessionNode[],
  status: ResearchTaskStatus | undefined,
): boolean {
  return (
    confirmed &&
    nodes.length > 0 &&
    status !== "deliverable" &&
    nodes.every((node) => node.status === "completed")
  );
}
