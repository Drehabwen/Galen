// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ArtifactMarkdown } from "./ArtifactMarkdown";

describe("ArtifactMarkdown", () => {
  afterEach(() => cleanup());

  it("opens galen artifact links inside the preview canvas", () => {
    const onOpenArtifact = vi.fn();
    render(
      <ArtifactMarkdown onOpenArtifact={onOpenArtifact}>
        {"已生成：[预览研究报告](galen-artifact://artifact-42)"}
      </ArtifactMarkdown>,
    );

    fireEvent.click(screen.getByRole("link", { name: "预览研究报告" }));
    expect(onOpenArtifact).toHaveBeenCalledWith("artifact-42");
  });

  it("leaves ordinary web links as normal links", () => {
    render(
      <ArtifactMarkdown onOpenArtifact={() => {}}>
        {"[查看来源](https://example.com/paper)"}
      </ArtifactMarkdown>,
    );

    expect(screen.getByRole("link", { name: "查看来源" }).getAttribute("href"))
      .toBe("https://example.com/paper");
  });
});
