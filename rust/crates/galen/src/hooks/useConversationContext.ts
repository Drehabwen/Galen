import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DecisionRecord } from "../types";

interface MemoryStatus {
  exists: boolean;
  size: number;
  preview: string;
}

export function useConversationContext(
  backendAvailable: boolean,
  workspaceRoot: string | null,
  messageCount: number,
) {
  const [memoryStatus, setMemoryStatus] = useState<MemoryStatus | null>(null);
  const [decisions, setDecisions] = useState<DecisionRecord[]>([]);

  useEffect(() => {
    if (!backendAvailable || !workspaceRoot) {
      setMemoryStatus(null);
      return;
    }
    invoke<MemoryStatus>("get_memory_status")
      .then(setMemoryStatus)
      .catch(() => setMemoryStatus(null));
  }, [backendAvailable, workspaceRoot]);

  const refreshDecisions = useCallback(async () => {
    if (!backendAvailable || !workspaceRoot) {
      setDecisions([]);
      return;
    }
    const next = await invoke<DecisionRecord[]>("get_conversation_decisions");
    setDecisions(next);
  }, [backendAvailable, workspaceRoot]);

  useEffect(() => {
    refreshDecisions().catch(() => setDecisions([]));
  }, [messageCount, refreshDecisions]);

  const reviseDecision = useCallback(
    async (id: string, statement: string) => {
      await invoke("revise_conversation_decision", { id, statement });
      await refreshDecisions();
    },
    [refreshDecisions],
  );

  const dismissDecision = useCallback(
    async (id: string) => {
      await invoke("dismiss_conversation_decision", { id });
      await refreshDecisions();
    },
    [refreshDecisions],
  );

  return { memoryStatus, decisions, reviseDecision, dismissDecision };
}
