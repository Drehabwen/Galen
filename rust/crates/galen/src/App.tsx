import { useState, useEffect } from "react";
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
import type { ModelConfig } from "./types";
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

  // Detect plan in latest AI message
  useEffect(() => {
    const lastAssistant = [...chat.messages].reverse().find((m) => m.role === "assistant");
    if (!lastAssistant) return;
    const nodes = extractPlan(lastAssistant.content);
    if (nodes && !planConfirmed) {
      setPendingPlan(nodes);
    }
  }, [chat.messages, planConfirmed]);

  const handleConfirmPlan = () => {
    if (pendingPlan) {
      setPlanNodes(pendingPlan);
      setPendingPlan(null);
      setPlanConfirmed(true);
      // Send confirmation as a user message
      chat.send(
        "计划已确认。请开始执行第一个节点。",
        model || "",
        modeState.mode,
        personaState.persona?.id ?? "dev",
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
    chat.send(
      input,
      model || "",
      modeState.mode,
      personaState.persona?.id ?? "dev",
    );
    setInput("");
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
              />
            </div>

            {/* ── Right: Canvas / Drawer / Session ── */}
            <div className="galen-canvas-panel">
              {enteredSession ? (
                <SessionChat
                  node={enteredSession}
                  onClose={() => {
                    setEnteredSession(null);
                    setSelectedNode(null);
                  }}
                  backendAvailable={chat.backendAvailable}
                  modelAlias={model}
                  onFlowBack={(node, summary) => {
                    chat.send(
                      `[Session ${node.index} 回流]\n${node.title}:\n${summary}`,
                      model || "",
                      modeState.mode,
                      personaState.persona?.id ?? "dev",
                    );
                    setEnteredSession(null);
                    setSelectedNode(null);
                  }}
                />
              ) : selectedNode ? (
                <SessionInspectorDrawer
                  node={selectedNode}
                  onClose={() => setSelectedNode(null)}
                  onEnterSession={(node) => setEnteredSession(node)}
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
    </div>
  );
}
