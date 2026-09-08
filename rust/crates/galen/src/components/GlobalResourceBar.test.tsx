// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { GlobalResourceBar } from "./GlobalResourceBar";
import type { ArtifactRecord } from "../domain/artifact";

const artifact: ArtifactRecord = {
  id: "artifact-1",
  path: "output/report.pdf",
  kind: "document",
  mimeType: "application/pdf",
  size: 1024,
  contentHash: "hash",
  taskId: "task-1",
  nodeId: null,
  createdAt: "2026-09-08T00:00:00Z",
  source: "agent",
};

describe("GlobalResourceBar", () => {
  afterEach(() => cleanup());

  it("closes the artifact drawer after opening an artifact", () => {
    const onOpenArtifact = vi.fn();
    render(<GlobalResourceBar artifacts={[artifact]} onOpenArtifact={onOpenArtifact} />);

    fireEvent.click(screen.getByRole("button", { name: /产物库/ }));
    expect(screen.getByText("交付记录")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /output\/report\.pdf/ }));

    expect(onOpenArtifact).toHaveBeenCalledWith(artifact);
    expect(screen.queryByText("交付记录")).toBeNull();
  });
});
