import { describe, expect, it } from "vitest";
import { analyzeSportsFatigue } from "./sportsFatigue";

describe("analyzeSportsFatigue", () => {
  it("compares the latest timepoint with the personal baseline", () => {
    const result = analyzeSportsFatigue("ATH-001", [
      { timepoint: "2026-09-01T08:00:00+08:00", hrvMs: 72, cmjCm: 42, rpe: 3, pain: 1 },
      { timepoint: "2026-09-03T08:00:00+08:00", hrvMs: 55, cmjCm: 37, rpe: 7, pain: 3 },
    ]);
    expect(result.caseId).toBe("ATH-001");
    expect(result.features.find((item) => item.metric === "hrvMs")?.direction).toBe("worsened");
    expect(result.features.filter((item) => item.direction === "worsened")).toHaveLength(4);
    expect(result.summary).toContain("4 项指标");
  });

  it("keeps missing measurements explicit", () => {
    const result = analyzeSportsFatigue("ATH-001", [
      { timepoint: "2026-09-01", hrvMs: 72, cmjCm: 42 },
      { timepoint: "2026-09-03", hrvMs: 70 },
    ]);
    expect(result.qualityFlags).toContain("cmjCm 缺少可比时间点");
  });
});
