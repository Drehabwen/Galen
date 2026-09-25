import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AgentBenchmarkReport, RehabCaseBundle, RehabCaseSummary, RehabGoldenEvalReport } from "../domain/rehabContext";

export function useRehabContext(backendAvailable: boolean, workspaceRoot: string | null) {
  const [cases, setCases] = useState<RehabCaseSummary[]>([]);
  const [activeCase, setActiveCase] = useState<RehabCaseBundle | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [evalReport, setEvalReport] = useState<RehabGoldenEvalReport | null>(null);
  const [agentBenchmark, setAgentBenchmark] = useState<AgentBenchmarkReport | null>(null);
  const refreshRevision = useRef(0);

  const refresh = useCallback(async () => {
    const revision = ++refreshRevision.current;
    if (!backendAvailable || !workspaceRoot) {
      setCases([]);
      setActiveCase(null);
      return;
    }
    try {
      const nextCases = await invoke<RehabCaseSummary[]>("list_rehab_cases");
      if (revision !== refreshRevision.current) return;
      setCases(nextCases);
      if (nextCases.length > 0) {
        const selected = activeCase && nextCases.some((item) => item.case_id === activeCase.case_record.case_id)
          ? activeCase.case_record.case_id
          : nextCases[0].case_id;
        const nextActiveCase = await invoke<RehabCaseBundle>("get_rehab_case", { caseId: selected });
        if (revision !== refreshRevision.current) return;
        setActiveCase(nextActiveCase);
      } else {
        setActiveCase(null);
      }
      setError(null);
    } catch (cause) {
      if (revision === refreshRevision.current) setError(String(cause));
    }
  }, [activeCase, backendAvailable, workspaceRoot]);

  useEffect(() => {
    let cancelled = false;
    void refresh();
    if (backendAvailable && workspaceRoot) {
      invoke<AgentBenchmarkReport>("get_agent_benchmark_report")
        .then((report) => {
          if (!cancelled) setAgentBenchmark(report);
        })
        .catch(() => {
          if (!cancelled) setAgentBenchmark(null);
        });
    }
    return () => {
      cancelled = true;
      refreshRevision.current += 1;
    };
    // Active case is deliberately excluded: refresh itself selects it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [backendAvailable, workspaceRoot]);

  const openCase = async (caseId: string) => {
    setLoading(true);
    try {
      setActiveCase(await invoke<RehabCaseBundle>("get_rehab_case", { caseId }));
      setError(null);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  };

  const importCase = async (sourcePath: string, caseId: string) => {
    setLoading(true);
    try {
      const bundle = await invoke<RehabCaseBundle>("import_rehab_case", { sourcePath, caseId });
      setActiveCase(bundle);
      const nextCases = await invoke<RehabCaseSummary[]>("list_rehab_cases");
      setCases(nextCases);
      setError(null);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  };

  const resolveReview = async (decisionId: string, optionId: string) => {
    if (!activeCase) return;
    setLoading(true);
    try {
      const bundle = await invoke<RehabCaseBundle>("resolve_rehab_review", {
        caseId: activeCase.case_record.case_id,
        decisionId,
        optionId,
        reviewer: null,
      });
      setActiveCase(bundle);
      setCases(await invoke<RehabCaseSummary[]>("list_rehab_cases"));
      setError(null);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  };

  const runGoldenJourneys = async (sourcePath: string) => {
    setLoading(true);
    try {
      setEvalReport(await invoke<RehabGoldenEvalReport>("run_rehab_golden_journeys", { sourcePath }));
      setError(null);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  };

  return { cases, activeCase, loading, error, evalReport, agentBenchmark, openCase, importCase, resolveReview, runGoldenJourneys };
}
