/** Deterministic, model-independent features for the sports-fatigue demo.
 *
 * This module deliberately does not diagnose a person. It compares repeated
 * measurements with that person's baseline so the PI workflow has a
 * reproducible numeric result to cite in a research report.
 */
export interface FatigueTimepoint {
  timepoint: string;
  hrvMs?: number;
  cmjCm?: number;
  rpe?: number;
  pain?: number;
}

export interface FatigueFeature {
  metric: "hrvMs" | "cmjCm" | "rpe" | "pain";
  baseline: number | null;
  current: number | null;
  deltaPercent: number | null;
  direction: "improved" | "worsened" | "stable" | "missing";
}

export interface FatigueAnalysis {
  caseId: string;
  baselineTimepoint: string;
  currentTimepoint: string;
  features: FatigueFeature[];
  qualityFlags: string[];
  summary: string;
}

const METRICS: Array<FatigueFeature["metric"]> = ["hrvMs", "cmjCm", "rpe", "pain"];

function percentChange(baseline: number, current: number): number | null {
  if (baseline === 0) return null;
  return Number((((current - baseline) / Math.abs(baseline)) * 100).toFixed(1));
}

function directionFor(metric: FatigueFeature["metric"], delta: number | null): FatigueFeature["direction"] {
  if (delta === null) return "missing";
  const worsens = metric === "hrvMs" || metric === "cmjCm" ? delta < -5 : delta > 5;
  const improves = metric === "hrvMs" || metric === "cmjCm" ? delta > 5 : delta < -5;
  if (worsens) return "worsened";
  if (improves) return "improved";
  return "stable";
}

export function analyzeSportsFatigue(
  caseId: string,
  timepoints: FatigueTimepoint[],
): FatigueAnalysis {
  if (timepoints.length < 2) throw new Error("运动疲劳分析至少需要两个时间点。");
  const baseline = timepoints[0];
  const current = timepoints[timepoints.length - 1];
  const features = METRICS.map((metric) => {
    const baselineValue = baseline[metric];
    const currentValue = current[metric];
    const delta = baselineValue !== undefined && currentValue !== undefined
      ? percentChange(baselineValue, currentValue)
      : null;
    return {
      metric,
      baseline: baselineValue ?? null,
      current: currentValue ?? null,
      deltaPercent: delta,
      direction: directionFor(metric, delta),
    } satisfies FatigueFeature;
  });
  const qualityFlags = features
    .filter((feature) => feature.direction === "missing")
    .map((feature) => `${feature.metric} 缺少可比时间点`);
  const worsened = features.filter((feature) => feature.direction === "worsened").length;
  const summary = worsened >= 2
    ? `相较 ${baseline.timepoint}，${current.timepoint} 有 ${worsened} 项指标朝疲劳方向变化；结果仅表示相对个人基线的变化。`
    : `相较 ${baseline.timepoint}，${current.timepoint} 未观察到两项及以上指标同时朝疲劳方向变化。`;
  return {
    caseId,
    baselineTimepoint: baseline.timepoint,
    currentTimepoint: current.timepoint,
    features,
    qualityFlags,
    summary,
  };
}
