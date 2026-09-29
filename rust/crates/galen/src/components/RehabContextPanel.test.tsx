// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RehabContextPanel } from "./RehabContextPanel";

afterEach(cleanup);

describe("RehabContextPanel import controls", () => {
  it("starts without a hidden sample dataset or case selection", () => {
    render(
      <RehabContextPanel
        workspaceSelected
        cases={[]}
        activeCase={null}
        loading={false}
        error={null}
        evalReport={null}
        agentBenchmark={null}
        onOpenCase={vi.fn()}
        onImportCase={vi.fn()}
        onResolveReview={vi.fn()}
        onReviewObservation={vi.fn()}
        onRunGoldenJourneys={vi.fn()}
      />,
    );

    expect((screen.getByLabelText("病例集相对路径") as HTMLInputElement).value).toBe("");
    expect((screen.getByLabelText("病例 ID") as HTMLInputElement).value).toBe("");
    expect(screen.getByRole("button", { name: "导入病例" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "运行黄金旅程" })).toHaveProperty("disabled", true);
  });

  it("submits an explicit auditable decision for a candidate observation", () => {
    const onReviewObservation = vi.fn();
    render(
      <RehabContextPanel
        workspaceSelected
        cases={[]}
        activeCase={{
          revision: 2,
          case_record: { case_id: "RID-001", demographics: {}, condition: {}, updated_at: "1" },
          events: [{ event_id: "baseline", event_type: "assessment", occurred_at: "baseline", collection_context: "surface_assessment", interventions: [] }],
          observations: [{
            observation_id: "obs-1",
            event_id: "baseline",
            metric: "adams_atr_deg",
            region: "whole_body",
            value: 7,
            unit: "deg",
            collection_context: "surface_assessment",
            verification_status: "candidate",
            source_locator: { pdf_page: null, book_page: null, channel: "governed_dataset", figure: null },
            protocol: {
              registryId: "rehab-screening-core",
              registryVersion: "1.0.0",
              originalMetric: "adams_atrDegrees",
              evidenceKind: "observed",
              allowedUse: "specialty_evidence",
              expectedUnit: "deg",
              unitMatches: true,
              registered: true,
            },
          }],
          review_decisions: [],
          observation_reviews: [],
          cohort_row: { status: "pending_review", reasons: [], derived_values: {}, source_coverage: 0, open_review_count: 1 },
        }}
        loading={false}
        error={null}
        evalReport={null}
        agentBenchmark={null}
        onOpenCase={vi.fn()}
        onImportCase={vi.fn()}
        onResolveReview={vi.fn()}
        onReviewObservation={onReviewObservation}
        onRunGoldenJourneys={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "审核观察" }));
    fireEvent.change(screen.getByLabelText("后续研究动作"), { target: { value: "recapture" } });
    fireEvent.change(screen.getByLabelText("观察审核理由"), { target: { value: "已核对原始测角仪记录与单位" } });
    fireEvent.click(screen.getByRole("button", { name: "提交审核" }));

    expect(onReviewObservation).toHaveBeenCalledWith(
      "obs-1",
      "accept",
      "已核对原始测角仪记录与单位",
      undefined,
      undefined,
      "recapture",
    );
  });
});
