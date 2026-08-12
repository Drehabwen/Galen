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
import { usePersona } from "./hooks/usePersona";
import type { ModelConfig, ModelStatus } from "./types";
import { ModelStatusPanel } from "./components/ModelStatusPanel";
import { StatusDot } from "./components/ui/primitives";
import type { SessionNode } from "./domain/sessionTypes";
import { extractPlan, hasPlan, planConfirmationPrompt } from "./domain/planParser";

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------
export default function App() {
  const chat = useChat();
  const env = useEnvironment();
  const modeState = useMode();
  const personaState = usePersona();
  const [input, setInput] = useState("");
  const [models, setModels] = useState<ModelConfig[]>([]);
  const [model, setModel] = useState("");
  const [modelStatuses, setModelStatuses] = useState<ModelStatus[]>([]);
  const [showModelStatus, setShowModelStatus] = useState(false);
  const [thinkingLevel, setThinkingLevel] = useState<string>(
    () => localStorage.getItem("galen.thinkingLevel") || "medium",
  );
  const [wsRoot, setWsRoot] = useState<string | null>(null);

  // Plan canvas — derived from AI responses
  const [planConfirmed, setPlanConfirmed] = useState(false);
  const [planNodes, setPlanNodes] = useState<SessionNode[]>([]);
  const [pendingPlan, setPendingPlan] = useState<SessionNode[] | null>(null);
  const [selectedNode, setSelectedNode] = useState<SessionNode | null>(null);

  // View mode toggle
  const [activeView, setActiveView] = useState<"execution-thread" | "daily-workbench">("execution-thread");
  // Canvas sub-tab
  const [canvasTab, setCanvasTab] = useState<"plan" | "doc">("plan");
  // Session enter state
  const [enteredSession, setEnteredSession] = useState<SessionNode | null>(null);
  const completionNotifiedRef = useRef(false);

  // Patch a plan node by id (loop state transition helper)
  const patchNode = useCallback((id: string, patch: Partial<SessionNode>) => {
    setPlanNodes((prev) => prev.map((n) => (n.id === id ? { ...n, ...patch } : n)));
  }, []);

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

  // Restore a persisted plan on startup (loop state survives restarts)
  useEffect(() => {
    if (!chat.backendAvailable) return;
    invoke<string | null>("load_plan")
      .then((planJson) => {
        if (!planJson) return;
        try {
          const nodes = JSON.parse(planJson) as SessionNode[];
          if (Array.isArray(nodes) && nodes.length > 0) {
            setPlanNodes(nodes);
            setPlanConfirmed(true);
            // Don't re-trigger the completion signal for an already-finished plan
            completionNotifiedRef.current = nodes.every(
              (n) => n.status === "completed",
            );
          }
        } catch {
          // Corrupt plan.json: ignore and let the user start fresh
        }
      })
      .catch(console.error);
  }, [chat.backendAvailable]);

  // Persist plan state whenever it changes (loop output -> plan.json)
  useEffect(() => {
    if (!planConfirmed || planNodes.length === 0) return;
    invoke("save_plan", { planJson: JSON.stringify(planNodes) }).catch(
      console.error,
    );
  }, [planConfirmed, planNodes]);

  const handleConfirmPlan = () => {
    if (pendingPlan) {
      if (!model) {
        setShowWelcome(true);
        return;
      }
      setPlanNodes(pendingPlan);
      setPendingPlan(null);
      setPlanConfirmed(true);
      // Send confirmation as a user message
      chat.send(
        "计划已确认。请开始执行第一个节点。",
        model || "",
        modeState.mode,
        personaState.persona?.id ?? "dev",
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

  // ---- Init ----
  useEffect(() => {
    if (!chat.backendAvailable) return;
    invoke<ModelConfig[]>("get_models")
      .then(setModels)
      .catch(console.error);
    invoke<ModelStatus[]>("get_model_status")
      .then(setModelStatuses)
      .catch(console.error);
    invoke<string | null>("get_workspace_root")
      .then(setWsRoot)
      .catch(console.error);
  }, [chat.backendAvailable]);

  useEffect(() => {
    if (!model && models.length > 0) setModel(models[0].name);
    if (models.length === 0 && chat.backendAvailable && !env.loading) {
      setShowWelcome(true);
    }
  }, [models, chat.backendAvailable, env.loading]);

  // The key is configured once in ~/.galen/models.toml and auto-loaded on
  // every start. If models are present, never keep the wizard open: the user
  // must not be asked to re-enter the key on each launch.
  useEffect(() => {
    if (showWelcome && models.length > 0) {
      setShowWelcome(false);
    }
  }, [showWelcome, models]);

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
      personaState.persona?.id ?? "dev",
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
      personaState.persona?.id ?? "dev",
      thinkingLevel,
    );
    // Loop output becomes project memory (GALEN.md), feeding next-task context
    invoke("append_memory", {
      entry: `${new Date().toISOString().slice(0, 10)} | Session ${node.index} ${node.title} | ${summary
        .trim()
        .slice(0, 120)} | plan.json`,
    }).catch(console.error);
    setEnteredSession(null);
    setSelectedNode(null);
    // Auto-advance: open the next ready node so the loop keeps moving
    const nextReady = findNextReady(updated);
    if (nextReady) enterSession(nextReady);
  };

  // Task-level loop closure: when every node has flowed back, the main
  // thread receives a completion signal so it can synthesize the final
  // artifact (report / paper) from the accumulated evidence chain.
  useEffect(() => {
    if (!planConfirmed || planNodes.length === 0) return;
    const allCompleted = planNodes.every((n) => n.status === "completed");
    if (allCompleted && !completionNotifiedRef.current) {
      completionNotifiedRef.current = true;
      chat.send(
        `[计划完成] 全部 ${planNodes.length} 个节点已执行完毕。` +
          "请基于各 Session 回流的证据链，整合生成最终成果（研究报告/论文/报告），并列出仍需人工签核的内容。",
        model || "",
        modeState.mode,
        personaState.persona?.id ?? "dev",
        thinkingLevel,
      );
    }
  }, [planConfirmed, planNodes, chat, model, modeState.mode, personaState.persona?.id, thinkingLevel]);

  const handleThinkingLevelChange = (level: string) => {
    setThinkingLevel(level);
    localStorage.setItem("galen.thinkingLevel", level);
  };

  const handlePickWorkspace = async () => {
    const path = await open({
      directory: true,
      multiple: false,
      title: "选择工作区",
    });
    if (!path) return;
    try {
      await invoke("set_workspace", { path });
      setWsRoot(path);
    } catch (e) {
      alert(String(e));
    }
  };

  const handleSaveApiKey = async (apiKey: string) => {
    await invoke("save_api_key", { apiKey });
    invoke<ModelConfig[]>("get_models").then(setModels).catch(console.error);
    invoke<ModelStatus[]>("get_model_status")
      .then(setModelStatuses)
      .catch(console.error);
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
        <span className="galen-topbar-brand">Galen</span>

        {/* View toggle */}
        <div className="galen-topbar-view-toggle">
          <button
            className={`galen-view-btn ${activeView === "execution-thread" ? "active" : ""}`}
            onClick={() => setActiveView("execution-thread")}
          >
            执行线程
          </button>
          <button
            className={`galen-view-btn ${activeView === "daily-workbench" ? "active" : ""}`}
            onClick={() => setActiveView("daily-workbench")}
          >
            日常工作台
          </button>
        </div>

        <span className="galen-topbar-project">
          {planConfirmed ? "科研计划已确认" : packageName}
        </span>

        <div className="galen-topbar-spacer" />

        {/* Persona */}
        <div className="model-chips">
          {personaState.allPersonas.map((p) => (
            <button
              key={p.id}
              className={`model-chip ${p.id === (personaState.persona?.id ?? "dev") ? "active" : ""}`}
              onClick={() => personaState.switchPersona(p.id)}
              title={p.description}
            >
              {p.label}
            </button>
          ))}
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
                      科研计划画布
                    </button>
                    <button
                      className={`canvas-tab ${canvasTab === "doc" ? "active" : ""}`}
                      onClick={() => setCanvasTab("doc")}
                    >
                      文档画布
                    </button>
                  </div>
                  {canvasTab === "plan" ? (
                    <ResearchPlanCanvas
                      nodes={planNodes}
                      planConfirmed={planConfirmed}
                      pendingPlan={pendingPlan}
                      onConfirmPlan={handleConfirmPlan}
                      onSelectNode={setSelectedNode}
                      selectedNodeId={null}
                    />
                  ) : (
                    <ResearchDocumentCanvas
                      onRevisionRequest={(actionId, selectedText) => {
                        setActiveView("execution-thread");
                        chat.send(
                          `[选区修订: ${actionId}]\n选中文本: ${selectedText}`,
                          model || "",
                          modeState.mode,
                          personaState.persona?.id ?? "dev",
                        );
                      }}
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
                personaState.persona?.id ?? "dev",
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
          onApiKey={handleSaveApiKey}
          onPickWorkspace={handlePickWorkspace}
          onDone={() => setShowWelcome(false)}
          envStatus={env.status}
          mcpServers={env.mcpServers}
        />
      )}
      {showModelStatus && (
        <ModelStatusPanel
          statuses={modelStatuses}
          onClose={() => setShowModelStatus(false)}
        />
      )}
    </div>
  );
}
