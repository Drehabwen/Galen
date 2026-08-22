import { describe, expect, it } from "vitest";
import type { SessionNode } from "../domain/sessionTypes";
import { normalizeResearchNodes } from "./useResearchTask";

describe("normalizeResearchNodes", () => {
  it("removes legacy approval gates without mutating the source", () => {
    const source: SessionNode[] = [
      {
        id: "s01",
        index: "01",
        title: "数据质检",
        type: "data",
        status: "pending_approval",
        approvalRequired: true,
      },
    ];

    const normalized = normalizeResearchNodes(source);

    expect(normalized[0].status).toBe("pending");
    expect(normalized[0].approvalRequired).toBe(false);
    expect(source[0].status).toBe("pending_approval");
    expect(source[0].approvalRequired).toBe(true);
  });

  it("preserves active and completed node states", () => {
    const nodes: SessionNode[] = [
      { id: "s01", index: "01", title: "分析", type: "analysis", status: "running" },
      { id: "s02", index: "02", title: "报告", type: "writing", status: "completed" },
    ];

    expect(normalizeResearchNodes(nodes).map((node) => node.status)).toEqual([
      "running",
      "completed",
    ]);
  });
});
