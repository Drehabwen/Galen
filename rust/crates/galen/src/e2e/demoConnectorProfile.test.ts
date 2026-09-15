import { describe, expect, it } from "vitest";
import { resolveE2eConnectorProfile } from "./demoConnectorProfile";

describe("resolveE2eConnectorProfile", () => {
  it("keeps the RehabGPT personal connector scoped to one RehabID", () => {
    expect(resolveE2eConnectorProfile("personal", "rehabgpt")).toMatchObject({
      sourceId: "rehabgpt",
      sourceLabel: "RehabGPT 患者端",
      patientCount: 1,
      sessionCount: 14,
      timepointCount: 14,
      measurementCount: 68,
      cases: [{ caseId: "RID-CHILD-014" }],
    });
  });

  it("keeps the therapist workbench cohort profile separate from personal data", () => {
    const profile = resolveE2eConnectorProfile("cohort", "rehab-workbench");

    expect(profile.patientCount).toBe(12);
    expect(profile.sessionCount).toBe(48);
    expect(profile.measurementCount).toBe(204);
    expect(profile.cases).toHaveLength(12);
    expect(profile.cases[0]?.caseId).toBe("ATH-001");
  });
});
