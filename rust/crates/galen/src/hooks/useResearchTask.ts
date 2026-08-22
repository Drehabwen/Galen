import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ResearchTask } from "../domain/researchTask";
import type { SessionNode } from "../domain/sessionTypes";

export interface TaskEvidenceInput {
  id: string;
  node_id: string;
  node_title: string;
  source: string;
  claim: string;
  detail?: string;
  confidence: string;
  created_at: string;
}

export function normalizeResearchNodes(nodes: SessionNode[]): SessionNode[] {
  return nodes.map((node) => ({
    ...node,
    approvalRequired: false,
    status: node.status === "pending_approval" ? "pending" : node.status,
  }));
}

export function useResearchTask(
  backendAvailable: boolean,
  workspaceRoot: string | null,
) {
  const [task, setTask] = useState<ResearchTask | null>(null);
  const [nodes, setNodes] = useState<SessionNode[]>([]);
  const [confirmed, setConfirmed] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const revisionRef = useRef(0);
  const workspaceScopeRef = useRef(workspaceRoot);
  workspaceScopeRef.current = workspaceRoot;
  const saveQueueRef = useRef<Promise<void>>(Promise.resolve());
  const skipNextSaveRef = useRef(false);

  const acceptSnapshot = useCallback((snapshot: ResearchTask) => {
    const normalizedNodes = normalizeResearchNodes(snapshot.nodes);
    // Snapshots emitted by the host are authoritative. Synchronize both the
    // task metadata and the canvas nodes, while preventing the mirror state
    // from being written straight back as a redundant revision.
    skipNextSaveRef.current = true;
    revisionRef.current = snapshot.revision;
    setTask({ ...snapshot, nodes: normalizedNodes });
    setNodes(normalizedNodes);
    setConfirmed(normalizedNodes.length > 0);
    setError(null);
  }, []);

  const restore = useCallback(async () => {
    if (!backendAvailable || !workspaceRoot) return null;
    const requestedWorkspace = workspaceRoot;
    const snapshot = await invoke<ResearchTask | null>("get_active_research_task");
    if (workspaceScopeRef.current !== requestedWorkspace) return null;
    if (!snapshot || snapshot.nodes.length === 0) {
      revisionRef.current = 0;
      setTask(null);
      setNodes([]);
      setConfirmed(false);
      return null;
    }
    const restoredNodes = normalizeResearchNodes(snapshot.nodes);
    const needsNormalization = snapshot.nodes.some(
      (node) => node.status === "pending_approval" || node.approvalRequired,
    );
    skipNextSaveRef.current = !needsNormalization;
    revisionRef.current = snapshot.revision;
    setTask({ ...snapshot, nodes: restoredNodes });
    setNodes(restoredNodes);
    setConfirmed(true);
    setError(null);
    return snapshot;
  }, [backendAvailable, workspaceRoot]);

  useEffect(() => {
    if (!backendAvailable || !workspaceRoot) {
      revisionRef.current = 0;
      setTask(null);
      setNodes([]);
      setConfirmed(false);
      return;
    }
    let cancelled = false;
    restore().catch((cause) => {
      if (!cancelled) setError(String(cause));
    });
    return () => {
      cancelled = true;
    };
  }, [backendAvailable, workspaceRoot, restore]);

  useEffect(() => {
    if (!confirmed || !task || nodes.length === 0) return;
    if (skipNextSaveRef.current) {
      skipNextSaveRef.current = false;
      return;
    }
    const taskId = task.taskId;
    const nextNodes = nodes;
    saveQueueRef.current = saveQueueRef.current
      .catch(() => undefined)
      .then(async () => {
        const saved = await invoke<ResearchTask>("save_research_task_nodes", {
          taskId,
          expectedRevision: revisionRef.current,
          nodes: nextNodes,
        });
        acceptSnapshot(saved);
        setError(null);
      })
      .catch(async (cause) => {
        const message = String(cause);
        setError(message);
        if (message.includes("RESEARCH_TASK_CONFLICT")) {
          await restore().catch(() => undefined);
        }
      });
  }, [acceptSnapshot, confirmed, nodes, restore, task?.taskId]);

  const createTask = useCallback(
    async (title: string, goal: string, initialNodes: SessionNode[]) => {
      const normalized = normalizeResearchNodes(initialNodes);
      const created = await invoke<ResearchTask>("create_research_task", {
        title,
        goal,
        nodes: normalized,
      });
      skipNextSaveRef.current = true;
      acceptSnapshot(created);
      setNodes(normalized);
      setConfirmed(true);
      setError(null);
      return created;
    },
    [acceptSnapshot],
  );

  const patchNode = useCallback((id: string, patch: Partial<SessionNode>) => {
    setNodes((current) =>
      current.map((node) => (node.id === id ? { ...node, ...patch } : node)),
    );
  }, []);

  const appendEvidence = useCallback(
    (evidence: TaskEvidenceInput) => {
      saveQueueRef.current = saveQueueRef.current
        .catch(() => undefined)
        .then(async () => {
          const saved = await invoke<ResearchTask>("append_evidence", { evidence });
          acceptSnapshot(saved);
          setError(null);
        })
        .catch((cause) => setError(String(cause)));
      return saveQueueRef.current;
    },
    [acceptSnapshot],
  );

  const flushWrites = useCallback(() => saveQueueRef.current, []);

  return {
    task,
    nodes,
    setNodes,
    confirmed,
    createTask,
    patchNode,
    appendEvidence,
    acceptSnapshot,
    flushWrites,
    restore,
    error,
  };
}
