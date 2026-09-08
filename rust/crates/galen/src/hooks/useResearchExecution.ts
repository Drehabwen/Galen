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
      await research.createTask(title, goal, autonomousPlan);
    } catch (error) {
      console.error(error);
      alert(`无法创建研究任务：${String(error)}`);
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
    );
  }, [chat, mode, model, onModelRequired, pendingPlan, research, thinkingLevel]);

  const enterSession = useCallback(
    async (node: SessionNode) => {
      try {
        const snapshot = await research.startNode(node.id);
        const running =
          snapshot.task.nodes.find((candidate) => candidate.id === node.id) ??
          node;
        setEnteredSession(running);
        setSelectedNode(null);
      } catch (error) {
        alert("PI 无法启动该节点：" + String(error));
      }
    },
    [research],
  );

  const closeSession = useCallback(() => {
    setEnteredSession(null);
    setSelectedNode(null);
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
        await research.approveNode(node.id);
        setSelectedNode(null);
      } catch (error) {
        alert("PI 无法批准该节点：" + String(error));
      }
    },
    [research],
  );

  const assignNode = useCallback(
    async (node: SessionNode) => {
      try {
        await research.assignNode(node.id, node.owner);
        setSelectedNode(null);
      } catch (error) {
        alert("PI 无法分派该节点：" + String(error));
      }
    },
    [research],
  );

  const flowBack = useCallback(
    async (node: SessionNode, summary: string) => {
      const evidence = extractEvidence(summary);
      let snapshot;
      try {
        snapshot = await research.completeNode(node.id, summary, evidence);
      } catch (error) {
        alert("PI 无法接收节点回流：" + String(error));
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
      );
      invoke("append_memory", {
        entry: `${new Date().toISOString().slice(0, 10)} | Session ${node.index} ${node.title} | ${summary
          .trim()
          .slice(0, 120)} | .galen/tasks/${research.task?.taskId || "active"}/task.json`,
      }).catch(console.error);
      await research
        .appendEvidence({
          id: `${Date.now()}-${node.id}`,
          node_id: node.id,
          node_title: node.title,
          source: node.type || "session",
          claim: summary.trim().slice(0, 200),
          detail: summary.trim().slice(0, 1200),
          confidence: "medium",
          created_at: new Date().toISOString().slice(0, 10),
        })
        .catch(console.error);
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
      );
    }
  }, [chat, mode, model, research.confirmed, research.nodes, research.task?.status, thinkingLevel]);

  return {
    research,
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
