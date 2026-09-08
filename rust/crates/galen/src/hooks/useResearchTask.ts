import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PiSnapshot, ResearchTask } from "../domain/researchTask";
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
  return nodes.map((node) => ({ ...node }));
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

  const acceptSnapshot = useCallback((snapshot: ResearchTask) => {
    const normalizedNodes = normalizeResearchNodes(snapshot.nodes);
    // Snapshots emitted by the host are authoritative. The webview mirrors
    // them but never writes node transitions back implicitly.
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

  const createTask = useCallback(
    async (title: string, goal: string, initialNodes: SessionNode[]) => {
      const normalized = normalizeResearchNodes(initialNodes);
      const created = await invoke<ResearchTask>("create_research_task", {
        title,
        goal,
        nodes: normalized,
      });
      acceptSnapshot(created);
      return created;
    },
    [acceptSnapshot],
  );

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

  const runPiCommand = useCallback(
    (
      command: string,
      args: Record<string, unknown>,
    ): Promise<PiSnapshot> => {
      const operation = saveQueueRef.current
        .catch(() => undefined)
        .then(async () => {
          const snapshot = await invoke<PiSnapshot>(command, args);
          acceptSnapshot(snapshot.task);
          setError(null);
          return snapshot;
        })
        .catch(async (cause) => {
          const message = String(cause);
          setError(message);
          if (
            message.includes("RESEARCH_TASK_CONFLICT") ||
            message.includes("PI_TASK_CONFLICT")
          ) {
            await restore().catch(() => undefined);
          }
          throw cause;
        });
      saveQueueRef.current = operation.then(
        () => undefined,
        () => undefined,
      );
      return operation;
    },
    [acceptSnapshot, restore],
  );

  const startNode = useCallback(
    (nodeId: string) => {
      if (!task) return Promise.reject(new Error("当前没有活动研究任务"));
      return runPiCommand("pi_start_node", {
        taskId: task.taskId,
        expectedRevision: revisionRef.current,
        nodeId,
        idempotencyKey: crypto.randomUUID(),
      });
    },
    [runPiCommand, task],
  );

  const completeNode = useCallback(
    (
      nodeId: string,
      result: string,
      evidence: string[],
      outputs: string[] = [],
    ) => {
      if (!task) return Promise.reject(new Error("当前没有活动研究任务"));
      return runPiCommand("pi_complete_node", {
        taskId: task.taskId,
        expectedRevision: revisionRef.current,
        nodeId,
        result,
        evidence,
        outputs,
        idempotencyKey: crypto.randomUUID(),
      });
    },
    [runPiCommand, task],
  );

  const approveNode = useCallback(
    (nodeId: string) => {
      if (!task) return Promise.reject(new Error("当前没有活动研究任务"));
      return runPiCommand("pi_approve_node", {
        taskId: task.taskId,
        expectedRevision: revisionRef.current,
        nodeId,
        idempotencyKey: crypto.randomUUID(),
      });
    },
    [runPiCommand, task],
  );

  const assignNode = useCallback(
    (nodeId: string, owner?: string) => {
      if (!task) return Promise.reject(new Error("当前没有活动研究任务"));
      return runPiCommand("pi_assign_node", {
        taskId: task.taskId,
        expectedRevision: revisionRef.current,
        nodeId,
        owner,
        idempotencyKey: crypto.randomUUID(),
      });
    },
    [runPiCommand, task],
  );

  const flushWrites = useCallback(() => saveQueueRef.current, []);

  const reset = useCallback(() => {
    revisionRef.current = 0;
    setTask(null);
    setNodes([]);
    setConfirmed(false);
    setError(null);
  }, []);

  return {
    task,
    nodes,
    confirmed,
    createTask,
    startNode,
    completeNode,
    approveNode,
    assignNode,
    appendEvidence,
    acceptSnapshot,
    flushWrites,
    restore,
    reset,
    error,
  };
}
