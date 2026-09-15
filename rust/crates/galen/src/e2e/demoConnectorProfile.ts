import type { ConnectorCasePreview } from "../domain/connectors";

export type E2eConnectorMode = "cohort" | "personal";
export type E2eConnectorSource = "rehab-workbench" | "rehabgpt";

export interface E2eConnectorProfile {
  sourceId: E2eConnectorSource;
  sourceLabel: string;
  exportPath: string;
  patientCount: number;
  sessionCount: number;
  assessmentCount: number;
  timepointCount: number;
  measurementCount: number;
  cases: ConnectorCasePreview[];
  receiptPath: string;
  receiptId: string;
  receiptHash: string;
  receiptSize: number;
}

const cohortCases: ConnectorCasePreview[] = Array.from({ length: 12 }, (_, index) => {
  const caseId = `ATH-${String(index + 1).padStart(3, "0")}`;
  return {
    caseId,
    displayName: `${caseId} · 运动疲劳恢复队列`,
    sessionCount: 4,
    assessmentCount: 16,
    timepointCount: 4,
    measurementCount: 17,
  };
});

const personalRehabGptCase: ConnectorCasePreview = {
  caseId: "RID-CHILD-014",
  displayName: "RID-CHILD-014 · 居家康复连续记录",
  sessionCount: 14,
  assessmentCount: 4,
  timepointCount: 14,
  measurementCount: 68,
};

export function resolveE2eConnectorProfile(
  mode: E2eConnectorMode,
  sourceId: E2eConnectorSource,
): E2eConnectorProfile {
  if (mode === "personal") {
    return {
      sourceId: "rehabgpt",
      sourceLabel: "RehabGPT 患者端",
      exportPath: "C:\\Users\\labops\\AppData\\Local\\RehabGPT\\GalenConnector\\rid-child-014.json",
      patientCount: 1,
      sessionCount: 14,
      assessmentCount: 4,
      timepointCount: 14,
      measurementCount: 68,
      cases: [personalRehabGptCase],
      receiptPath: "output/connector-imports/rehabgpt-rid-child-014/import-receipt.json",
      receiptId: "artifact-connector-rehabgpt-personal-receipt",
      receiptHash: "connector-rehabgpt-rid-child-014-v1",
      receiptSize: 1684,
    };
  }

  return {
    sourceId,
    sourceLabel: "康复师工作台",
    exportPath: "C:\\Users\\labops\\AppData\\Local\\Rehab\\GalenConnector\\cohort-fatigue.json",
    patientCount: 12,
    sessionCount: 48,
    assessmentCount: 192,
    timepointCount: 48,
    measurementCount: 204,
    cases: cohortCases,
    receiptPath: "output/connector-imports/rehab-cohort-demo/import-receipt.json",
    receiptId: "artifact-connector-cohort-receipt",
    receiptHash: "connector-cohort-v2-receipt",
    receiptSize: 4286,
  };
}
