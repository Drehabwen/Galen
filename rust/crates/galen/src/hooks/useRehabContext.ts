import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { RehabCaseBundle, RehabCaseSummary, RehabGoldenEvalReport } from "../domain/rehabContext";

export function useRehabContext(backendAvailable: boolean, workspaceRoot: string | null) {
  const [cases, setCases] = useState<RehabCaseSummary[]>([]);
  const [activeCase, setActiveCase] = useState<RehabCaseBundle | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [evalReport, setEvalReport] = useState<RehabGoldenEvalReport | null>(null);

  const refresh = useCallback(async () => {
    if (!backendAvailable || !workspaceRoot) {
      setCases([]);
      setActiveCase(null);
      return;
    }
    try {
      const nextCases = await invoke<RehabCaseSummary[]>("list_rehab_cases");
      setCases(nextCases);
      if (nextCases.length > 0) {
        const selected = activeCase && nextCases.some((item) => item.case_id === activeCase.case_record.case_id)
          ? activeCase.case_record.case_id
          : nextCases[0].case_id;
        setActiveCase(await invoke<RehabCaseBundle>("get_rehab_case", { caseId: selected }));
      } else {
        setActiveCase(null);
      }
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }, [activeCase, backendAvailable, workspaceRoot]);

  useEffect(() => {
    void refresh();
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
        reviewer: "local-human-reviewer",
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

  return { cases, activeCase, loading, error, evalReport, openCase, importCase, resolveReview, runGoldenJourneys };
}
