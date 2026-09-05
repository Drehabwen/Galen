// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SourceInspector, sourceFromLink } from "./SourceInspector";

const { openUrl } = vi.hoisted(() => ({ openUrl: vi.fn(async () => undefined) }));

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl }));

describe("SourceInspector", () => {
  afterEach(() => {
    cleanup();
    openUrl.mockClear();
  });

  it("shows a verifiable PMID and opens it with the system browser", async () => {
    render(
      <SourceInspector
        source={{
          kind: "pmid",
          id: "32946039",
          href: "https://pubmed.ncbi.nlm.nih.gov/32946039/",
          label: "PMID: 32946039",
        }}
        onClose={() => undefined}
      />,
    );

    expect(screen.getByRole("heading", { name: "来源核验" })).toBeTruthy();
    expect(screen.getByText("PubMed")).toBeTruthy();
    expect(screen.getByText("PMID: 32946039")).toBeTruthy();
    expect(screen.getByText("https://pubmed.ncbi.nlm.nih.gov/32946039/")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "打开原文" }));
    expect(openUrl).toHaveBeenCalledWith("https://pubmed.ncbi.nlm.nih.gov/32946039/");
  });

  it("normalizes DOI links as a publisher-verifiable source", () => {
    expect(sourceFromLink("https://doi.org/10.1000/example.1", "DOI: 10.1000/example.1")).toEqual({
      kind: "doi",
      id: "10.1000/example.1",
      href: "https://doi.org/10.1000/example.1",
      label: "DOI: 10.1000/example.1",
    });
  });
});
