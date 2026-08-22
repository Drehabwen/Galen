import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useChat } from "./hooks/useChat";
import { ResearchExecutionThread } from "./components/ResearchExecutionThread";
import { ResearchPlanCanvas } from "./components/ResearchPlanCanvas";
import { ResearchDocumentCanvas } from "./components/ResearchDocumentCanvas";
import { ResearchWorkbench } from "./components/ResearchWorkbench";
import { SessionChat } from "./components/SessionChat";
import { SessionInspectorDrawer } from "./components/SessionInspectorDrawer";
import { GlobalResourceBar } from "./components/GlobalResourceBar";
import { ContextPanel } from "./components/ContextPanel";
import { WelcomeWizard } from "./components/WelcomeWizard";
import { useEnvironment } from "./hooks/useEnvironment";
import { useMode } from "./hooks/useMode";
import type { ChatMode } from "./hooks/useMode";
import type { ModelConfig, ModelStatus } from "./types";
import { ModelStatusPanel } from "./components/ModelStatusPanel";
import { WorkbenchRail } from "./components/WorkbenchRail";
import { StatusDot } from "./components/ui/primitives";
import type { SessionNode } from "./domain/sessionTypes";
import { extractPlan, hasPlan, planConfirmationPrompt } from "./domain/planParser";
import { useResearchTask } from "./hooks/useResearchTask";

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------
export default function App() {
  const [wsRoot, setWsRoot] = useState<string | null>(null);
  const chat = useChat(wsRoot);
  const env = useEnvironment();
  const modeState = useMode();
  const [input, setInput] = useState("");
  const [models, setModels] = useState<ModelConfig[]>([]);
  const [model, setModel] = useState("");
  const [modelStatuses, setModelStatuses] = useState<ModelStatus[]>([]);
  const [showModelStatus, setShowModelStatus] = useState(false);
  const [wizardInitialStep, setWizardInitialStep] = useState(0);
  const [thinkingLevel, setThinkingLevel] = useState<string>(
    () => localStorage.getItem("galen.thinkingLevel") || "medium",
  );
  const research = useResearchTask(chat.backendAvailable, wsRoot);
  const researchTask = research.task;
  const planNodes = research.nodes;
  const setPlanNodes = research.setNodes;
  const planConfirmed = research.confirmed;

  // Plan canvas — derived from AI responses
  const [pendingPlan, setPendingPlan] = useState<SessionNode[] | null>(null);
  const [selectedNode, setSelectedNode] = useState<SessionNode | null>(null);

  // View mode toggle
  const [activeView, setActiveView] = useState<"execution-thread" | "daily-workbench">("execution-thread");
  // Canvas sub-tab
  const [canvasTab, setCanvasTab] = useState<"plan" | "doc">("plan");
  // Session enter state
  const [enteredSession, setEnteredSession] = useState<SessionNode | null>(null);
  const completionNotifiedRef = useRef(false);
  const observedTaskIdRef = useRef<string | null>(null);
  const [artifactPreview, setArtifactPreview] = useState<{ path: string; content: string; nodeTitle?: string } | null>(null);
  const [artifactLoading, setArtifactLoading] = useState(false);
  const [artifactError, setArtifactError] = useState<string | null>(null);

  const patchNode = research.patchNode;

  // Extract key evidence points from a session summary (bullet lines)
  const extractEvidence = (summary: string): string[] => {
    const bullets = summary
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => /^[-*•]/.test(line))
      .map((line) => line.replace(/^[-*•]\s*/, ""))
      .filter(Boolean)
      .slice(0, 8);
    return bullets;
  };

  // First ready node: not completed/running and all dependencies completed
  const findNextReady = useCallback(
    (nodes: SessionNode[]): SessionNode | null =>
      nodes.find(
        (n) =>
          n.status !== "completed" &&
          n.status !== "running" &&
          (n.dependsOn ?? []).every(
            (dep) => nodes.find((d) => d.id === dep)?.status === "completed",
          ),
      ) ?? null,
    [],
  );

  // Detect plan in latest AI message
  useEffect(() => {
    const lastAssistant = [...chat.messages].reverse().find((m) => m.role === "assistant");
    if (!lastAssistant) return;
    const nodes = extractPlan(lastAssistant.content);
    if (nodes && !planConfirmed) {
      setPendingPlan(nodes);
    }
  }, [chat.messages, planConfirmed]);

  // Initialize task-level completion notification exactly once per restored or
  // newly created task. Subsequent node writes must not reset it.
  useEffect(() => {
    if (!researchTask) {
      observedTaskIdRef.current = null;
      return;
    }
    if (observedTaskIdRef.current !== researchTask.taskId) {
      observedTaskIdRef.current = researchTask.taskId;
      completionNotifiedRef.current = planNodes.every((node) => node.status === "completed");
    }
  }, [planNodes, researchTask]);

  const handleConfirmPlan = async () => {
    if (pendingPlan) {
      if (!model) {
        setShowWelcome(true);
        return;
      }
      const autonomousPlan = pendingPlan.map((node) => ({
        ...node,
        approvalRequired: false,
        status: node.status === "pending_approval" ? "pending" as const : node.status,
      }));
      const latestRequest = [...chat.messages]
        .reverse()
        .find((message) => message.role === "user")?.content.trim();
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
      // Send confirmation as a user message
      chat.send(
        "计划已确认。请开始执行第一个节点。",
        model || "",
        modeState.mode,
        "medical",
        thinkingLevel,
      );
    }
  };

  // Welcome wizard
  const [showWelcome, setShowWelcome] = useState(false);

  // Memory status
  const [memoryStatus, setMemoryStatus] = useState<{
    exists: boolean;
    size: number;
    preview: string;
  } | null>(null);

  const packageName = wsRoot
    ? wsRoot.split(/[/\\]/).pop() ?? "未命名"
    : "未选择项目";
  const completedNodes = planNodes.filter((node) => node.status === "completed").length;

  // ---- Init ----
  useEffect(() => {
    if (!chat.backendAvailable) return;
    let cancelled = false;
    // 一次性判断是否需要向导：无模型，或所有模型都缺 Key。
    // 只在首次数据就绪时判断，之后 models 变化不会自动关闭向导。
    Promise.all([
      invoke<ModelConfig[]>("get_models"),
      invoke<ModelStatus[]>("get_model_status"),
      invoke<string | null>("get_workspace_root"),
    ])
      .then(([ms, sts, ws]) => {
        if (cancelled) return;
        setModels(ms);
        setModelStatuses(sts);
        if (ws) setWsRoot(ws);
        if (!model && ms.length > 0) setModel(ms[0].name);
        const needsSetup =
          ms.length === 0 ||
          (sts.length > 0 && sts.every((s) => !s.api_key_present));
        if (needsSetup) setShowWelcome(true);
      })
      .catch(console.error);
    return () => {
      cancelled = true;
    };
  }, [chat.backendAvailable]);

  useEffect(() => {
    if (!chat.backendAvailable || !wsRoot) {
      setMemoryStatus(null);
      return;
    }
    invoke<{ exists: boolean; size: number; preview: string }>(
      "get_memory_status",
    )
      .then(setMemoryStatus)
      .catch(() => setMemoryStatus(null));
  }, [chat.backendAvailable, wsRoot]);

  // ---- Keyboard shortcuts ----
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey) {
        switch (e.key) {
          case "1":
            modeState.modes[0] &&
              modeState.switchMode(modeState.modes[0].id as ChatMode);
            e.preventDefault();
            break;
          case "2":
            modeState.modes[1] &&
              modeState.switchMode(modeState.modes[1].id as ChatMode);
            e.preventDefault();
            break;
          case "3":
            modeState.modes[2] &&
              modeState.switchMode(modeState.modes[2].id as ChatMode);
            e.preventDefault();
            break;
          case "l":
            chat.clear();
            e.preventDefault();
            break;
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [modeState.switchMode, chat.clear]);

  // ---- Actions ----
  const handleSend = () => {
    if (!input.trim() || chat.sending) return;
    if (!model) {
      setShowWelcome(true);
      return;
    }
    chat.send(
      input,
      model || "",
      modeState.mode,
      "medical",
      thinkingLevel,
    );
    setInput("");
  };

  // Enter a session: mark the node running (pending -> running)
  const enterSession = (node: SessionNode) => {
    patchNode(node.id, { status: "running" });
    setEnteredSession(node);
  };

  // Session flow-back: mark node completed, attach outcome/evidence,
  // and feed a structured context block back into the main thread loop.
  const handleFlowBack = (node: SessionNode, summary: string) => {
    const updated = planNodes.map((n) =>
      n.id === node.id
        ? {
            ...n,
            status: "completed" as SessionNode["status"],
            result: summary.trim().slice(0, 2000),
            evidence: extractEvidence(summary),
          }
        : n,
    );
    const completedCount = updated.filter((n) => n.status === "completed").length;
    const total = updated.length;
    setPlanNodes(updated);
    chat.send(
      `[Session ${node.index} 回流 · 已完成]\n` +
        `目标: ${node.title}\n` +
        `产出摘要: ${summary.trim()}\n` +
        `计划进度: ${completedCount}/${total} 完成`,
      model || "",
      modeState.mode,
      "medical",
      thinkingLevel,
    );
    // Loop output becomes project memory (GALEN.md), feeding next-task context
    invoke("append_memory", {
      entry: `${new Date().toISOString().slice(0, 10)} | Session ${node.index} ${node.title} | ${summary
        .trim()
        .slice(0, 120)} | .galen/tasks/${researchTask?.taskId || "active"}/task.json`,
    }).catch(console.error);
    // Structured evidence: 证据链落盘，供上下文注入与最终成文引用
    const evidence = {
      id: `${Date.now()}-${node.id}`,
      node_id: node.id,
      node_title: node.title,
      source: node.type || "session",
      claim: summary.trim().slice(0, 200),
      detail: summary.trim().slice(0, 1200),
      confidence: "medium",
      created_at: new Date().toISOString().slice(0, 10),
    };
    research.appendEvidence(evidence).catch(console.error);
    setEnteredSession(null);
    setSelectedNode(null);
    // Auto-advance: open the next ready node so the loop keeps moving
    const nextReady = findNextReady(updated);
    if (nextReady) enterSession(nextReady);
  };

  // Task-level loop closure: synthesize the final artifact automatically
  // after every node has returned its evidence.
  useEffect(() => {
    if (!planConfirmed || planNodes.length === 0) return;
    const allCompleted = planNodes.every((n) => n.status === "completed");
    if (allCompleted && !completionNotifiedRef.current) {
      completionNotifiedRef.current = true;
      chat.send(
        `[计划完成] 全部 ${planNodes.length} 个节点已执行完毕。` +
          "请基于各 Session 回流的证据链自动整合最终成果，将报告保存到工作区，并在回复中明确给出产物路径以便 Galen 内预览。",
        model || "",
        modeState.mode,
        "medical",
        thinkingLevel,
      );
    }
  }, [planConfirmed, planNodes, chat, model, modeState.mode, thinkingLevel]);

  const handlePreviewArtifact = useCallback(async (path: string, node: SessionNode) => {
    setArtifactLoading(true);
    setArtifactError(null);
    setCanvasTab("doc");
    try {
      const content = await invoke<string>("read_workspace_file", { path });
      setArtifactPreview({ path, content, nodeTitle: node.title });
    } catch (error) {
      setArtifactPreview(null);
      setArtifactError(String(error));
    } finally {
      setArtifactLoading(false);
    }
  }, []);

  const handleThinkingLevelChange = (level: string) => {
    setThinkingLevel(level);
    localStorage.setItem("galen.thinkingLevel", level);
  };

  const handlePickWorkspace = async (): Promise<string | null> => {
    const path = await open({
      directory: true,
      multiple: false,
      title: "选择工作区",
    });
    if (!path) return null;
    try {
      // Finish writes for the previous workspace before switching the host root.
      await research.flushWrites();
      await invoke("set_workspace", { path });
      setWsRoot(path);
      return path;
    } catch (e) {
      alert(String(e));
      return null;
    }
  };

  const handleTestConnection = async (): Promise<string> => {
    const result = await invoke<string>("test_model_connection");
    return result;
  };

  const handleSaveApiKey = async (apiKey: string, defaultModel?: string) => {
    await invoke("save_api_key", { apiKey, defaultModel });
    const [nextModels, nextStatuses] = await Promise.all([
      invoke<ModelConfig[]>("get_models"),
      invoke<ModelStatus[]>("get_model_status"),
    ]);
    setModels(nextModels);
    setModelStatuses(nextStatuses);
    setModel((current) => {
      if (current && nextModels.some((item) => item.name === current)) return current;
      return (
        nextModels.find((item) => item.name === defaultModel)?.name ??
        nextModels[0]?.name ??
        ""
      );
    });
  };

  const openWizard = (step = 0) => {
    setShowModelStatus(false);
    setWizardInitialStep(step);
    setShowWelcome(true);
  };

  const openModelStatus = () => {
    invoke<ModelStatus[]>("get_model_status")
      .then(setModelStatuses)
      .catch(console.error);
    setShowModelStatus(true);
  };

  // ---- Render ----
  return (
    <div className="galen-shell">
      {/* ════ Top Bar ════ */}
      <div className="galen-topbar">
        <div className="galen-topbar-identity">
          <span className="galen-topbar-brand">Galen</span>
          <span className="galen-topbar-discipline">康复科研工作台</span>
        </div>

        <div className="galen-topbar-study">
          <span className="galen-topbar-study-label">当前研究</span>
          <span className="galen-topbar-project">
            {planConfirmed
              ? `${researchTask?.title || "研究任务"} · ${completedNodes}/${planNodes.length}`
              : packageName}
          </span>
        </div>

        <div className="galen-topbar-spacer" />

        {/* Mode switch (click to change; Ctrl+1/2/3 also works) */}
        <div className="galen-mode-switch" role="group" aria-label="工作模式">
          {(["discuss", "plan", "auto"] as ChatMode[]).map((id) => {
            const meta = modeState.modes.find((m) => m.id === id);
            const label = meta?.label ?? (id === "discuss" ? "讨论" : id === "plan" ? "计划" : "自动");
            const description =
              meta?.description ??
              (id === "discuss"
                ? "只读顾问：检索文献、查询康复数据、追问分析"
                : id === "plan"
                  ? "制定方案，列出步骤，确认后执行"
                  : "自主分解目标，并行执行，汇总产出");
            return (
              <button
                key={id}
                className={`galen-mode-btn ${modeState.mode === id ? "active" : ""}`}
                onClick={() => modeState.switchMode(id)}
                title={description}
              >
                {label}
              </button>
            );
          })}
        </div>

        {/* Memory badge */}
        {memoryStatus?.exists && (
          <span
            title={`GALEN.md · ${memoryStatus.size} 字节`}
            style={{
              fontSize: "var(--text-xs)",
              color: "var(--success)",
              background: "var(--success-soft)",
              padding: "1px 8px",
              borderRadius: "var(--radius-pill)",
            }}
          >
            记忆已加载
          </span>
        )}

        {/* Model / key status */}
        <button
          className="btn btn-sm btn-ghost"
          onClick={openModelStatus}
          title="查看模型与密钥状态"
        >
          模型状态
          <span
            className={`model-status-dot ${
              modelStatuses.length > 0 &&
              modelStatuses.every((s) => s.api_key_present)
                ? "ok"
                : "missing"
            }`}
          />
        </button>

        <StatusDot tone={chat.sending ? "active" : "idle"}>
          {chat.sending ? "运行中" : "就绪"}
        </StatusDot>

        <button className="btn btn-sm btn-ghost" onClick={handlePickWorkspace}>
          {wsRoot ? packageName : "选择工作区"}
        </button>
      </div>

      {/* ════ Body ════ */}
      <div className="galen-body">
        <WorkbenchRail
          activeView={activeView}
          onViewChange={setActiveView}
          canvasTab={canvasTab}
          onCanvasTabChange={setCanvasTab}
          completedNodes={completedNodes}
          totalNodes={planNodes.length}
        />
        {activeView === "execution-thread" ? (
          <>
            {/* ── Left: Main Thread (Chat) ── */}
            <div className="galen-chat-panel">
              <ResearchExecutionThread
                messages={chat.messages}
                streaming={chat.streaming}
                thinking={chat.thinking}
                sending={chat.sending}
                error={chat.error}
                backendAvailable={chat.backendAvailable}
                input={input}
                onInputChange={setInput}
                onSend={handleSend}
                models={models}
                selectedModel={model}
                onModelChange={setModel}
                thinkingLevel={thinkingLevel}
                onThinkingLevelChange={handleThinkingLevelChange}
              />
            </div>

            {/* ── Right: Canvas / Drawer / Session ── */}
            <div className="galen-canvas-panel">
              {enteredSession ? (
                <SessionChat
                  node={enteredSession}
                  onClose={() => {
                    if (enteredSession.status === "running") {
                      patchNode(enteredSession.id, { status: "pending" });
                    }
                    setEnteredSession(null);
                    setSelectedNode(null);
                  }}
                  backendAvailable={chat.backendAvailable}
                  modelAlias={model}
                  thinkingLevel={thinkingLevel}
                  autoRun
                  onFlowBack={handleFlowBack}
                />
              ) : selectedNode ? (
                <SessionInspectorDrawer
                  node={selectedNode}
                  onClose={() => setSelectedNode(null)}
                  onEnterSession={enterSession}
                  onApprove={(node) => {
                    setPlanNodes((prev) =>
                      prev.map((n) =>
                        n.id === node.id
                          ? { ...n, status: "approved" as const }
                          : n,
                      ),
                    );
                    setSelectedNode(null);
                  }}
                  onAssign={(node) => {
                    setPlanNodes((prev) =>
                      prev.map((n) =>
                        n.id === node.id
                          ? { ...n, status: "assigned" as const }
                          : n,
                      ),
                    );
                    setSelectedNode(null);
                  }}
                />
              ) : (
                <>
                  {/* Canvas tab bar */}
                  <div className="canvas-tab-bar">
                    <button
                      className={`canvas-tab ${canvasTab === "plan" ? "active" : ""}`}
                      onClick={() => setCanvasTab("plan")}
                    >
                      证据脉络
                    </button>
                    <button
                      className={`canvas-tab ${canvasTab === "doc" ? "active" : ""}`}
                      onClick={() => setCanvasTab("doc")}
                    >
                      成果预览
                    </button>
                  </div>
                  {canvasTab === "plan" ? (
                    <ResearchPlanCanvas
                      nodes={planNodes}
                      planConfirmed={planConfirmed}
                      pendingPlan={pendingPlan}
                      onConfirmPlan={handleConfirmPlan}
                      onSelectNode={setSelectedNode}
                      onPreviewArtifact={handlePreviewArtifact}
                      selectedNodeId={null}
                    />
                  ) : (
                    <ResearchDocumentCanvas
                      artifact={artifactPreview}
                      loading={artifactLoading}
                      error={artifactError}
                      onBackToPlan={() => setCanvasTab("plan")}
                    />
                  )}
                </>
              )}
            </div>
          </>
        ) : (
          <ResearchWorkbench
            wsRoot={wsRoot}
            files={[]}
            currentFile={null}
            backendAvailable={chat.backendAvailable}
            onAgentPrompt={(prompt: string) => {
              setActiveView("execution-thread");
              chat.send(
                prompt,
                model || "",
                modeState.mode,
                "medical",
              );
            }}
            onReadFile={() => {}}
          />
        )}
      </div>

      {/* ════ Bottom: Context + Global Resource ════ */}
      {activeView === "execution-thread" && (
        <ContextPanel
          messages={chat.messages}
          compacted={chat.messages.length > 20}
        />
      )}
      <GlobalResourceBar />

      {/* ════ Welcome Wizard ════ */}
      {showWelcome && (
        <WelcomeWizard
          initialStep={wizardInitialStep}
          onApiKey={handleSaveApiKey}
          onPickWorkspace={handlePickWorkspace}
          onTestConnection={handleTestConnection}
          onDone={() => setShowWelcome(false)}
          hasApiKey={modelStatuses.some((s) => s.api_key_present)}
          memoryExists={memoryStatus?.exists ?? false}
          envStatus={env.status}
          mcpServers={env.mcpServers}
          mode={modeState.mode}
          modes={modeState.modes}
          onSwitchMode={modeState.switchMode}
        />
      )}
      {showModelStatus && (
        <ModelStatusPanel
          statuses={modelStatuses}
          onClose={() => setShowModelStatus(false)}
          onOpenWizard={() => openWizard(1)}
        />
      )}
    </div>
  );
}
