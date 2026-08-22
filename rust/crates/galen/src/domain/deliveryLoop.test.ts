import { describe, expect, it } from "vitest";
import type { ArtifactRecord } from "./artifact";
import type { ResearchTask } from "./researchTask";
import type { SessionNode } from "./sessionTypes";
import {
  mergeDeliveredArtifact,
  resolveArtifactNodeTitle,
  shouldSynthesizeCompletion,
} from "./deliveryLoop";

const artifact = (id: string, nodeId = "n1"): ArtifactRecord => ({
  id,
  path: `output/${id}.md`,
  kind: "document",
  mimeType: "text/markdown",
  size: 10,
  contentHash: id,
  taskId: "task-1",
  nodeId,
  createdAt: "1",
  source: "agent",
});

const nodes: SessionNode[] = [
  { id: "n1", index: "01", title: "研究问题", type: "analysis", status: "completed" },
  { id: "n2", index: "02", title: "干预设计", type: "analysis", status: "completed" },
];

describe("delivery loop contract", () => {
  it("moves the latest artifact to the front without duplicating it", () => {
    expect(mergeDeliveredArtifact([artifact("a"), artifact("b")], artifact("b")).map((x) => x.id))
      .toEqual(["b", "a"]);
  });

  it("resolves the preview title through the host task snapshot", () => {
    const task = { nodes } as ResearchTask;
    expect(resolveArtifactNodeTitle(artifact("a"), task)).toBe("研究问题");
  });

  it("requests synthesis only for a completed task without a delivered artifact", () => {
    expect(shouldSynthesizeCompletion(true, nodes, "running")).toBe(true);
    expect(shouldSynthesizeCompletion(true, nodes, "deliverable")).toBe(false);
    expect(shouldSynthesizeCompletion(false, nodes, "running")).toBe(false);
  });
});
