// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ConnectorPreview } from "../domain/connectors";
import { ConnectorConfirmation } from "./ConnectorConfirmation";

afterEach(cleanup);

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
  cases: [{
    caseId: "CASE-001",
    displayName: "研究对象",
    sessionCount: 3,
    assessmentCount: 12,
    timepointCount: 3,
    measurementCount: 12,
  }],
  canImport: true,
  message: "已发现数据",
};

describe("ConnectorConfirmation", () => {
  it("requires an importable preview before confirmation", () => {
    const onConfirm = vi.fn();
    const { rerender } = render(
      <ConnectorConfirmation preview={{ ...preview, canImport: false }} loading={false} onConfirm={onConfirm} />,
    );

    expect((screen.getByRole("button", { name: "确认获取" }) as HTMLButtonElement).disabled).toBe(true);

    rerender(<ConnectorConfirmation preview={preview} loading={false} onConfirm={onConfirm} />);
    fireEvent.click(screen.getByRole("button", { name: "确认获取" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("dismisses a completed import receipt", () => {
    const onDismiss = vi.fn();
    render(
      <ConnectorConfirmation
        loading={false}
        result={{
          caseIds: ["CASE-001"],
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
            createdAt: "2026-09-24",
            source: "agent",
          },
        }}
        onDismiss={onDismiss}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "完成" }));
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });
});
