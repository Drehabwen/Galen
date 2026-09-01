// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ArtifactMarkdown, linkifyEvidenceIdentifiers } from "./ArtifactMarkdown";

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

  it("turns explicit PMID and DOI identifiers into verification links", () => {
    render(
      <ArtifactMarkdown>
        {"运动功能改善见 PMID: 32946039；机制研究见 DOI: 10.1000/example.1。"}
      </ArtifactMarkdown>,
    );

    expect(screen.getByRole("link", { name: "PMID: 32946039" }).getAttribute("href"))
      .toBe("https://pubmed.ncbi.nlm.nih.gov/32946039/");
    expect(screen.getByRole("link", { name: "DOI: 10.1000/example.1" }).getAttribute("href"))
      .toBe("https://doi.org/10.1000/example.1");
  });

  it("does not turn an unlabelled number into a citation", () => {
    expect(linkifyEvidenceIdentifiers("样本量为 32946039 例")).toBe("样本量为 32946039 例");
  });
});
