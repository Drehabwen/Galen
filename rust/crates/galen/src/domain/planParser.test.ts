import { describe, expect, it } from "vitest";
import { extractPlan, hasPlan, planConfirmationPrompt } from "./planParser";

describe("extractPlan", () => {
  it("parses pipe-delimited nodes without introducing approval gates", () => {
    const markdown = `
前置说明
<!-- PLAN_START -->
01 | 数据质检 | 检查缺失值 | -
02 | 描述统计 | 形成基线表 | 01
<!-- PLAN_END -->
`;

    const nodes = extractPlan(markdown);

    expect(nodes).toHaveLength(2);
    expect(nodes?.map((node) => node.status)).toEqual(["pending", "pending"]);
    expect(nodes?.[1].dependsOn).toEqual(["s01"]);
  });

  it("parses numbered fallback plans as autonomous pending work", () => {
    const markdown = `
<!-- PLAN_START -->
1. 数据质检 — 检查字段与缺失值
2. 统计分析 — 生成可验证结果
<!-- PLAN_END -->
`;

    const nodes = extractPlan(markdown);

    expect(nodes?.map((node) => node.id)).toEqual(["s01", "s02"]);
    expect(nodes?.every((node) => node.status === "pending")).toBe(true);
  });

  it("ignores responses without a plan contract", () => {
    expect(hasPlan("普通研究讨论")).toBe(false);
    expect(extractPlan("普通研究讨论")).toBeNull();
  });
});

describe("planConfirmationPrompt", () => {
  it("describes task locking and autonomous execution instead of canvas approval", () => {
    const nodes = extractPlan(`
<!-- PLAN_START -->
01 | 数据质检 | 检查缺失值 | -
<!-- PLAN_END -->
`)!;

    const prompt = planConfirmationPrompt(nodes);

    expect(prompt).toContain("锁定任务契约");
    expect(prompt).toContain("自动执行");
    expect(prompt).not.toContain("画布将生成");
    expect(prompt).not.toContain("要确认吗");
  });
});
