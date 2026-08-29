export type VerificationStatus = "candidate" | "verified" | "disputed";
export type CohortStatus = "included" | "excluded" | "pending_review";

export interface ClinicalEvent {
  event_id: string;
  event_type: string;
  occurred_at: string;
  collection_context: string;
  interventions: string[];
}

export interface Observation {
  observation_id: string;
  event_id: string;
  metric: string;
  region: string;
  value: number | string | null;
  unit: string;
  collection_context: string;
  verification_status: VerificationStatus;
  source_locator: {
    pdf_page: number | null;
    book_page: number | null;
    channel: string;
    figure: string | null;
  };
}

export interface ReviewDecision {
  decision_id: string;
  target_observation_id: string;
  question: string;
  status: "open" | "resolved";
  selected_option_id: string | null;
  options: Array<{
    option_id: string;
    label: string;
    value: number | string;
    channel: string;
  }>;
}

export interface RehabCaseBundle {
  revision: number;
  case_record: {
    case_id: string;
    demographics: Record<string, unknown>;
    condition: Record<string, unknown>;
    updated_at: string;
  };
  events: ClinicalEvent[];
  observations: Observation[];
  review_decisions: ReviewDecision[];
  cohort_row: {
    status: CohortStatus;
    reasons: string[];
    derived_values: Record<string, number | string>;
    source_coverage: number;
    open_review_count: number;
  };
}

export interface RehabCaseSummary {
  case_id: string;
  revision: number;
  status: CohortStatus;
  event_count: number;
  observation_count: number;
  open_review_count: number;
}

export interface RehabGoldenEvalReport {
  suite_id: string;
  generated_at: string;
  passed: boolean;
  negative_optimization_detected: boolean;
  journeys: Array<{
    journey_id: string;
    title: string;
    persona: string;
    passed: boolean;
    duration_ms: number;
    checks: Array<{
      name: string;
      passed: boolean;
      expected: string;
      actual: string;
      critical: boolean;
    }>;
  }>;
  metrics: Array<{
    id: string;
    label: string;
    value: number;
    threshold: number;
    passed: boolean;
    unit: string;
  }>;
  recommendations: string[];
}

export interface AgentBenchmarkReport {
  case_id: string;
  runs: Array<{
    profile: string;
    model: string;
    samples: number;
    pass_rate: number;
    mean_ttfr_ms: number;
    p95_ttfr_ms: number;
    mean_total_ms: number;
    p95_total_ms: number;
    mean_input_tokens: number;
    mean_output_tokens: number;
  }>;
}
