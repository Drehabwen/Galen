// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
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
        onRunGoldenJourneys={vi.fn()}
      />,
    );

    expect((screen.getByLabelText("病例集相对路径") as HTMLInputElement).value).toBe("");
    expect((screen.getByLabelText("病例 ID") as HTMLInputElement).value).toBe("");
    expect(screen.getByRole("button", { name: "导入病例" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "运行黄金旅程" })).toHaveProperty("disabled", true);
  });
});
