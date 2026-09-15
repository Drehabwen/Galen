import { describe, expect, it } from "vitest";
import { FIRST_PARTY_CONNECTORS, getFirstPartyConnector, parseConnectorIntent } from "./connectors";

describe("first-party connector registry", () => {
  it("lists the two local exports and the RehabGPT API adapter", () => {
    expect(FIRST_PARTY_CONNECTORS.map((connector) => connector.id)).toEqual([
      "rehab-workbench",
      "qingyue-workbench",
      "rehabgpt",
    ]);
    expect(getFirstPartyConnector("rehabgpt")?.endpoints).toContain("/api/integration/tracking/:patientId");
  });

  it("marks the RehabGPT bridge as callable", () => {
    expect(getFirstPartyConnector("rehab-workbench")?.status).toBe("ready");
    expect(getFirstPartyConnector("qingyue-workbench")?.status).toBe("adapter_needed");
    expect(getFirstPartyConnector("rehabgpt")?.status).toBe("ready");
  });

  it("recognizes a conversation-driven workbench request", () => {
    expect(parseConnectorIntent("从康复师工作台获取 ATH-001 最近三次评估数据")).toEqual({
      sourceId: "rehab-workbench",
      caseHint: "ATH-001",
      latestAssessments: 3,
    });
    expect(parseConnectorIntent("从 RehabGPT 获取 RID-CHILD-014 最近两周的训练、疼痛和量表记录")).toEqual({
      sourceId: "rehabgpt",
      caseHint: "RID-CHILD-014",
      latestAssessments: undefined,
    });
    expect(parseConnectorIntent("帮我设计一个康复研究方案")).toBeNull();
  });
});
