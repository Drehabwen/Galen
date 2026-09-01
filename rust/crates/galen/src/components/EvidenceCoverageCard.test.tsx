// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import {
  EvidenceCoverageCard,
  type LiteratureCoverage,
} from "./EvidenceCoverageCard";

const coverage: LiteratureCoverage = {
  taskId: "task-42",
  hasLimitations: true,
  limitation: "一个或多个已配置的文献来源未成功检索。最终结论必须说明“基于已检索来源”，不得暗示覆盖完整。",
  providers: [
    {
      providerId: "pubmed",
      displayName: "PubMed",
      state: "searched",
      hasSuccessfulHistory: true,
      latestQuery: "stroke rehabilitation",
      latestFinishedAt: "2026-08-31T10:00:00Z",
      resultCount: 0,
      errorClass: null,
    },
    {
      providerId: "crossref",
      displayName: "Crossref",
      state: "failed",
      hasSuccessfulHistory: false,
      latestQuery: null,
      latestFinishedAt: "2026-08-31T10:01:00Z",
      resultCount: null,
      errorClass: "timeout",
    },
    {
      providerId: "semantic-scholar",
      displayName: "Semantic Scholar",
      state: "configured_disabled",
      hasSuccessfulHistory: false,
      latestQuery: null,
      latestFinishedAt: null,
      resultCount: null,
      errorClass: null,
    },
    {
      providerId: "cnki",
      displayName: "CNKI",
      state: "unavailable",
      hasSuccessfulHistory: false,
      latestQuery: null,
      latestFinishedAt: null,
      resultCount: null,
      errorClass: null,
    },
    {
      providerId: "openalex",
      displayName: "OpenAlex",
      state: "connected_not_searched",
      hasSuccessfulHistory: false,
      latestQuery: null,
      latestFinishedAt: null,
      resultCount: null,
      errorClass: null,
    },
    {
      providerId: "local-index",
      displayName: "Local index",
      state: "not_configured",
      hasSuccessfulHistory: false,
      latestQuery: null,
      latestFinishedAt: null,
      resultCount: null,
      errorClass: null,
    },
  ],
};

describe("EvidenceCoverageCard", () => {
  afterEach(() => cleanup());

  it("distinguishes searched zero, failure, disabled, unavailable, and unsearched provider states", () => {
    render(<EvidenceCoverageCard coverage={coverage} />);

    expect(screen.getByText("已检索 · 0 条")).toBeTruthy();
    expect(screen.getByText("失败 · timeout")).toBeTruthy();
    expect(screen.getByText("已禁用")).toBeTruthy();
    expect(screen.getByText("不可用 · 不代表没有中文证据")).toBeTruthy();
    expect(screen.getByText("尚未检索")).toBeTruthy();
    expect(screen.getByText("未配置")).toBeTruthy();
    expect(screen.getByText(coverage.limitation!)).toBeTruthy();
  });

  it("keeps a safe query hidden until a provider row is expanded", () => {
    render(<EvidenceCoverageCard coverage={coverage} />);

    expect(screen.queryByText("stroke rehabilitation")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: /PubMed/ }));

    expect(screen.getByText("stroke rehabilitation")).toBeTruthy();
  });

  it("does not treat a failed CNKI search as absence of Chinese evidence", () => {
    render(
      <EvidenceCoverageCard
        coverage={{
          ...coverage,
          providers: coverage.providers.map((provider) => (
            provider.providerId === "cnki"
              ? { ...provider, state: "failed", errorClass: "unavailable" }
              : provider
          )),
        }}
      />,
    );

    expect(screen.getByText("失败 · 不代表没有中文证据")).toBeTruthy();
  });
});
