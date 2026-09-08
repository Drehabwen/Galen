export type InsightDirection = "up" | "down" | "flat";

export interface InsightMetric {
  id: string;
  label: string;
  unit: string;
  baseline: number;
  current: number;
  deltaPercent: number | null;
  direction: InsightDirection;
  values: number[];
  labels: string[];
  favorable: "up" | "down";
}

export interface AnalysisResult {
  title: string;
  generatedAt: string;
  recordCount: number;
  fieldCount: number;
  dataSource: string;
  qualityReport: string;
  metrics: InsightMetric[];
  findings: string[];
  dataNotes: string[];
  timelineImport: RehabTimelineImportPayload;
}

export interface RehabTimelineMeasurement {
  caseId: string;
  timepoint: string;
  metric: string;
  value: number;
  unit: string;
}

export interface RehabTimelineImportPayload {
  datasetPath: string;
  qualityReportPath: string;
  suggestedCaseId: string;
  measurements: RehabTimelineMeasurement[];
}

export interface RehabTimelineImportOutput {
  caseIds: string[];
  importedEventCount: number;
  importedObservationCount: number;
  skippedObservationCount: number;
  receipt: import("./artifact").ArtifactRecord;
}
