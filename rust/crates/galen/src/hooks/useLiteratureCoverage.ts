import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type LiteratureCoverageState =
  | "searched"
  | "failed"
  | "connected_not_searched"
  | "configured_disabled"
  | "unavailable"
  | "not_configured";

export type LiteratureErrorClass =
  | "timeout"
  | "authentication"
  | "rate_limited"
  | "unavailable"
  | "protocol"
  | "invalid_response"
  | "other";

export interface LiteratureProviderCoverage {
  providerId: string;
  displayName: string;
  state: LiteratureCoverageState;
  hasSuccessfulHistory: boolean;
  latestQuery: string | null;
  latestFinishedAt: string | null;
  resultCount: number | null;
  errorClass: LiteratureErrorClass | null;
}

export interface LiteratureCoverage {
  taskId: string | null;
  providers: LiteratureProviderCoverage[];
  hasLimitations: boolean;
  limitation: string | null;
}

export function useLiteratureCoverage(
  backendAvailable: boolean,
  workspaceRoot: string | null,
  taskId: string | undefined,
  refreshKey: number,
) {
  const [coverage, setCoverage] = useState<LiteratureCoverage | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let stale = false;

    if (!backendAvailable || !workspaceRoot) {
      setCoverage(null);
      setLoading(false);
      setError(null);
      return () => {
        stale = true;
      };
    }

    setCoverage(null);
    setLoading(true);
    setError(null);

    invoke<LiteratureCoverage>("get_literature_coverage")
      .then((nextCoverage) => {
        if (!stale) setCoverage(nextCoverage);
      })
      .catch(() => {
        if (!stale) setError("无法加载文献来源覆盖范围。");
      })
      .finally(() => {
        if (!stale) setLoading(false);
      });

    return () => {
      stale = true;
    };
  }, [backendAvailable, workspaceRoot, taskId, refreshKey]);

  return { coverage, loading, error };
}
