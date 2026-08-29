import { useState } from "react";
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
import { useModelConfiguration } from "./hooks/useModelConfiguration";
import { useResearchExecution } from "./hooks/useResearchExecution";
import { useArtifactDelivery } from "./hooks/useArtifactDelivery";
import { useConversationContext } from "./hooks/useConversationContext";
import { useAppShortcuts } from "./hooks/useAppShortcuts";
import { useWorkspaceSelection } from "./hooks/useWorkspaceSelection";
import { ModelStatusPanel } from "./components/ModelStatusPanel";
import { WorkbenchRail } from "./components/WorkbenchRail";
import { AppTopBar } from "./components/AppTopBar";
import { RehabContextPanel } from "./components/RehabContextPanel";
import type { WorkbenchView } from "./components/WorkbenchRail";
import { useRehabContext } from "./hooks/useRehabContext";

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------
export default function App() {
  const workspace = useWorkspaceSelection();
  const wsRoot = workspace.root;
  const chat = useChat(wsRoot);
  const env = useEnvironment();
  const modeState = useMode();
  const [input, setInput] = useState("");
  const modelConfiguration = useModelConfiguration(chat.backendAvailable);
  const {
    models,
    model,
    setModel,
    modelStatuses,
    showModelStatus,
    closeModelStatus,
    showWelcome,
    setShowWelcome,
    wizardInitialStep,
    thinkingLevel,
    handleThinkingLevelChange,
    handleTestConnection,
    handleSaveApiKey,
    openWizard,
    openModelStatus,
  } = modelConfiguration;
  const execution = useResearchExecution({
    backendAvailable: chat.backendAvailable,
    workspaceRoot: wsRoot,
    chat,
    model,
    mode: modeState.mode,
    thinkingLevel,
    onModelRequired: () => setShowWelcome(true),
  });
  const research = execution.research;
  const researchTask = research.task;
  const planNodes = research.nodes;
  const planConfirmed = research.confirmed;
  const pendingPlan = execution.pendingPlan;
  const selectedNode = execution.selectedNode;
  const setSelectedNode = execution.setSelectedNode;
  const enteredSession = execution.enteredSession;
  const delivery = useArtifactDelivery(
    chat.backendAvailable,
    wsRoot,
    chat,
    research,
  );
  const conversationContext = useConversationContext(
    chat.backendAvailable,
    wsRoot,
    chat.messages.length,
  );
  const rehabContext = useRehabContext(chat.backendAvailable, wsRoot);

  const [activeView, setActiveView] = useState<WorkbenchView>("execution-thread");

  const packageName = workspace.name;
  const completedNodes = planNodes.filter((node) => node.status === "completed").length;

  useAppShortcuts(modeState.modes, modeState.switchMode, chat.clear);

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

  const handlePickWorkspace = () => workspace.pick(research.flushWrites);

  // ---- Render ----
  return (
    <div className="galen-shell">
      <AppTopBar
        studyLabel={
          planConfirmed
            ? `${researchTask?.title || "研究任务"} · ${completedNodes}/${planNodes.length}`
            : packageName
        }
        modes={modeState.modes}
        mode={modeState.mode}
        onModeChange={(nextMode) => void modeState.switchMode(nextMode)}
        memorySize={
          conversationContext.memoryStatus?.exists
            ? conversationContext.memoryStatus.size
            : undefined
        }
        capabilities={env.capabilities}
        modelStatuses={modelStatuses}
        running={chat.sending}
        workspaceSelected={Boolean(wsRoot)}
        workspaceLabel={packageName}
        onOpenModelStatus={openModelStatus}
        onPickWorkspace={() => void handlePickWorkspace()}
      />

      {/* ════ Body ════ */}
      <div className="galen-body">
        <WorkbenchRail
          activeView={activeView}
          onViewChange={setActiveView}
          canvasTab={delivery.canvasTab}
          onCanvasTabChange={delivery.setCanvasTab}
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
                latestRunMetrics={chat.latestRunMetrics}
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
                  onClose={execution.closeSession}
                  backendAvailable={chat.backendAvailable}
                  modelAlias={model}
                  thinkingLevel={thinkingLevel}
                  autoRun
                  onFlowBack={execution.flowBack}
                />
              ) : selectedNode ? (
                <SessionInspectorDrawer
                  node={selectedNode}
                  onClose={() => setSelectedNode(null)}
                  onEnterSession={execution.enterSession}
                  onApprove={execution.approveNode}
                  onAssign={execution.assignNode}
                />
              ) : (
                <>
                  {/* Canvas tab bar */}
                  <div className="canvas-tab-bar">
                    <button
                      className={`canvas-tab ${delivery.canvasTab === "plan" ? "active" : ""}`}
                      onClick={() => delivery.setCanvasTab("plan")}
                    >
                      证据脉络
                    </button>
                    <button
                      className={`canvas-tab ${delivery.canvasTab === "doc" ? "active" : ""}`}
                      onClick={() => delivery.setCanvasTab("doc")}
                    >
                      成果预览
                    </button>
                  </div>
                  {delivery.canvasTab === "plan" ? (
                    <ResearchPlanCanvas
                      nodes={planNodes}
                      planConfirmed={planConfirmed}
                      pendingPlan={pendingPlan}
                      onConfirmPlan={execution.confirmPlan}
                      onSelectNode={setSelectedNode}
                      onPreviewArtifact={delivery.previewNodeArtifact}
                      selectedNodeId={null}
                    />
                  ) : (
                    <ResearchDocumentCanvas
                      artifact={delivery.preview}
                      loading={delivery.loading}
                      error={delivery.error}
                      onBackToPlan={() => delivery.setCanvasTab("plan")}
                    />
                  )}
                </>
              )}
            </div>
          </>
        ) : activeView === "daily-workbench" ? (
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
        ) : (
          <RehabContextPanel
            workspaceSelected={Boolean(wsRoot)}
            cases={rehabContext.cases}
            activeCase={rehabContext.activeCase}
            loading={rehabContext.loading}
            error={rehabContext.error}
            evalReport={rehabContext.evalReport}
            agentBenchmark={rehabContext.agentBenchmark}
            onOpenCase={(caseId) => void rehabContext.openCase(caseId)}
            onImportCase={(sourcePath, caseId) => void rehabContext.importCase(sourcePath, caseId)}
            onResolveReview={(decisionId, optionId) => void rehabContext.resolveReview(decisionId, optionId)}
            onRunGoldenJourneys={(sourcePath) => void rehabContext.runGoldenJourneys(sourcePath)}
          />
        )}
      </div>

      {/* ════ Bottom: Context + Global Resource ════ */}
      {activeView === "execution-thread" && (
        <ContextPanel
          messages={chat.messages}
          compacted={chat.messages.length > 20}
          decisions={conversationContext.decisions}
          onReviseDecision={conversationContext.reviseDecision}
          onDismissDecision={conversationContext.dismissDecision}
        />
      )}
      <GlobalResourceBar
        artifacts={delivery.artifacts}
        onOpenArtifact={(artifact) => {
          setActiveView("execution-thread");
          void delivery.openRegisteredArtifact(artifact);
        }}
      />

      {/* ════ Welcome Wizard ════ */}
      {showWelcome && (
        <WelcomeWizard
          initialStep={wizardInitialStep}
          onApiKey={handleSaveApiKey}
          onPickWorkspace={handlePickWorkspace}
          onTestConnection={handleTestConnection}
          onDone={() => setShowWelcome(false)}
          hasApiKey={modelStatuses.some((s) => s.api_key_present)}
          memoryExists={conversationContext.memoryStatus?.exists ?? false}
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
          onClose={closeModelStatus}
          onOpenWizard={() => openWizard(1)}
          onSaveApiKey={handleSaveApiKey}
          onTestConnection={handleTestConnection}
        />
      )}
    </div>
  );
}
