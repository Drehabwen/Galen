// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { ConnectorDataCanvas } from "./ConnectorDataCanvas";
import type { ConnectorPreview } from "../domain/connectors";

const preview: ConnectorPreview = {
  sourceId: "rehab-workbench",
  sourceLabel: "康复师工作台",
  exportPath: "rehab-backup.json",
  exportedAt: 1789000000000,
  connectionMode: "live_bridge",
  patientCount: 1,
  sessionCount: 3,
  assessmentCount: 12,
  timepointCount: 3,
  measurementCount: 12,
  cases: [{ caseId: "ATH-001", displayName: "ATH-001", sessionCount: 3, assessmentCount: 12, timepointCount: 3, measurementCount: 12 }],
  canImport: true,
  message: "已发现数据",
};

describe("ConnectorDataCanvas", () => {
  afterEach(() => cleanup());

  it("moves from preview to an auditable RehabID receipt", () => {
    const { rerender } = render(<ConnectorDataCanvas preview={preview} result={null} latestAssessments={3} />);
    expect(screen.getByText("工作台数据预检")).toBeTruthy();
    expect(screen.getByText("ATH-001")).toBeTruthy();
    expect(screen.getByText("等待确认")).toBeTruthy();

    rerender(<ConnectorDataCanvas preview={preview} latestAssessments={3} result={{
      caseIds: ["ATH-001"],
      importedEventCount: 3,
      importedObservationCount: 12,
      skippedObservationCount: 0,
      receipt: {
        id: "receipt-1",
        path: "output/import-receipt.json",
        kind: "file",
        mimeType: "application/json",
        size: 20,
        contentHash: "hash",
        taskId: null,
        nodeId: null,
        createdAt: "2026-09-10",
        source: "agent",
      },
    }} />);
    expect(screen.getByText("研究数据已就位")).toBeTruthy();
    expect(screen.getByText("可调用")).toBeTruthy();
    expect(screen.getByText("import-receipt.json")).toBeTruthy();
  });
});
