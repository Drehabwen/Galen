import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { extractPlan } from "../domain/planParser";
import { shouldSynthesizeCompletion } from "../domain/deliveryLoop";
import type { SessionNode } from "../domain/sessionTypes";
import type { ChatMode } from "./useMode";
import { useResearchTask } from "./useResearchTask";
import type { useChat } from "./useChat";

type ChatController = ReturnType<typeof useChat>;

interface ResearchExecutionOptions {
  backendAvailable: boolean;
  workspaceRoot: string | null;
  chat: ChatController;
  model: string;
  mode: ChatMode;
  thinkingLevel: string;
  onModelRequired: () => void;
}

function extractEvidence(summary: string): string[] {
  return summary
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => /^[-*•]/.test(line))
    .map((line) => line.replace(/^[-*•]\s*/, ""))
    .filter(Boolean)
    .slice(0, 8);
}

export function useResearchExecution({
  backendAvailable,
  workspaceRoot,
  chat,
  model,
  mode,
  thinkingLevel,
  onModelRequired,
}: ResearchExecutionOptions) {
  const research = useResearchTask(backendAvailable, workspaceRoot);
  const [pendingPlan, setPendingPlan] = useState<SessionNode[] | null>(null);
  const [selectedNode, setSelectedNode] = useState<SessionNode | null>(null);
  const [enteredSession, setEnteredSession] = useState<SessionNode | null>(null);
  const [error, setError] = useState<string | null>(null);
  const completionNotifiedRef = useRef(false);
  const observedTaskIdRef = useRef<string | null>(null);

  useEffect(() => {
    const lastAssistant = [...chat.messages]
      .reverse()
      .find((message) => message.role === "assistant");
    if (!lastAssistant) return;
    const nodes = extractPlan(lastAssistant.content);
    if (nodes && !research.confirmed) setPendingPlan(nodes);
  }, [chat.messages, research.confirmed]);

  useEffect(() => {
    if (!research.task) {
      observedTaskIdRef.current = null;
      return;
    }
    if (observedTaskIdRef.current !== research.task.taskId) {
      observedTaskIdRef.current = research.task.taskId;
      completionNotifiedRef.current = research.nodes.every(
        (node) => node.status === "completed",
      );
    }
  }, [research.nodes, research.task]);

  const confirmPlan = useCallback(async () => {
    if (!pendingPlan) return;
    if (!model) {
      onModelRequired();
      return;
    }

    const autonomousPlan = pendingPlan.map((node) => ({
      ...node,
      approvalRequired: false,
      status:
        node.status === "pending_approval"
          ? ("pending" as const)
          : node.status,
    }));
    const latestRequest = [...chat.messages]
      .reverse()
      .find((message) => message.role === "user")
      ?.content.trim();
    const goal = latestRequest || "完成当前康复科研任务";
    const title = goal.replace(/\s+/g, " ").slice(0, 48);

    try {
      setError(null);
      await research.createTask(title, goal, autonomousPlan);
    } catch (error) {
      const message = `无法创建研究任务：${String(error)}`;
      setError(message);
      alert(message);
      return;
    }

    setPendingPlan(null);
    completionNotifiedRef.current = false;
    chat.send(
      "计划已确认。请开始执行第一个节点。",
      model,
      mode,
      "medical",
      thinkingLevel,
      "inherit",
    );
  }, [chat, mode, model, onModelRequired, pendingPlan, research, thinkingLevel]);

  const enterSession = useCallback(
    async (node: SessionNode) => {
      try {
        setError(null);
        const snapshot = await research.startNode(node.id);
        const running =
          snapshot.task.nodes.find((candidate) => candidate.id === node.id) ??
          node;
        setEnteredSession(running);
        setSelectedNode(null);
      } catch (error) {
        const message = "PI 无法启动该节点：" + String(error);
        setError(message);
        alert(message);
      }
    },
    [research],
  );

  const closeSession = useCallback(() => {
    setEnteredSession(null);
    setSelectedNode(null);
    setError(null);
  }, []);

  const resetForNewTopic = useCallback(() => {
    completionNotifiedRef.current = false;
    observedTaskIdRef.current = null;
    setPendingPlan(null);
    setSelectedNode(null);
    setEnteredSession(null);
    research.reset();
  }, [research]);

  const approveNode = useCallback(
    async (node: SessionNode) => {
      try {
        setError(null);
        await research.approveNode(node.id);
        setSelectedNode(null);
      } catch (error) {
        const message = "PI 无法批准该节点：" + String(error);
        setError(message);
        alert(message);
      }
    },
    [research],
  );

  const assignNode = useCallback(
    async (node: SessionNode) => {
      try {
        setError(null);
        await research.assignNode(node.id, node.owner);
        setSelectedNode(null);
      } catch (error) {
        const message = "PI 无法分派该节点：" + String(error);
        setError(message);
        alert(message);
      }
    },
    [research],
  );

  const flowBack = useCallback(
    async (node: SessionNode, summary: string) => {
      const evidence = extractEvidence(summary);
      let snapshot;
      try {
        setError(null);
        snapshot = await research.completeNode(node.id, summary, evidence);
      } catch (error) {
        const message = "PI 无法接收节点回流：" + String(error);
        setError(message);
        alert(message);
        return;
      }
      const completedCount = snapshot.task.nodes.filter(
        (candidate) => candidate.status === "completed",
      ).length;
      chat.send(
        `[Session ${node.index} 回流 · 已完成]\n` +
          `目标: ${node.title}\n` +
          `产出摘要: ${summary.trim()}\n` +
          `计划进度: ${completedCount}/${snapshot.task.nodes.length} 完成`,
        model,
        mode,
        "medical",
        thinkingLevel,
        "inherit",
      );
      const persistence = await Promise.allSettled([
        invoke("append_memory", {
        entry: `${new Date().toISOString().slice(0, 10)} | Session ${node.index} ${node.title} | ${summary
          .trim()
          .slice(0, 120)} | .galen/tasks/${research.task?.taskId || "active"}/task.json`,
        }),
        research.appendEvidence({
          id: `${Date.now()}-${node.id}`,
          node_id: node.id,
          node_title: node.title,
          source: node.type || "session",
          claim: summary.trim().slice(0, 200),
          detail: summary.trim().slice(0, 1200),
          confidence: "medium",
          created_at: new Date().toISOString().slice(0, 10),
        }),
      ]);
      const failedPersistence = persistence.filter(
        (result): result is PromiseRejectedResult => result.status === "rejected",
      );
      if (failedPersistence.length > 0) {
        setError(
          `节点已完成，但有 ${failedPersistence.length} 项研究记录未持久化：${failedPersistence
            .map((result) => String(result.reason))
            .join("；")}`,
        );
      }
      setEnteredSession(null);
      setSelectedNode(null);

    },
    [chat, mode, model, research, thinkingLevel],
  );

  useEffect(() => {
    if (
      shouldSynthesizeCompletion(
        research.confirmed,
        research.nodes,
        research.task?.status,
      ) &&
      !completionNotifiedRef.current
    ) {
      completionNotifiedRef.current = true;
      chat.send(
        `[计划完成] 全部 ${research.nodes.length} 个节点已执行完毕。` +
          "请基于各 Session 回流的证据链自动整合最终成果，将报告保存到工作区，并在回复中明确给出产物路径以便 Galen 内预览。",
        model,
        mode,
        "medical",
        thinkingLevel,
        "inherit",
      );
    }
  }, [chat, mode, model, research.confirmed, research.nodes, research.task?.status, thinkingLevel]);

  return {
    research,
    error,
    pendingPlan,
    selectedNode,
    setSelectedNode,
    enteredSession,
    confirmPlan,
    enterSession,
    closeSession,
    approveNode,
    assignNode,
    flowBack,
    resetForNewTopic,
  };
}
