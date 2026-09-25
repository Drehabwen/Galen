import type { VerifiableSource } from "./SourceInspector";
import { ResearchThreadComposer } from "./ResearchThreadComposer";
import { ConnectorConfirmation } from "./ConnectorConfirmation";
import { ResearchThreadMessages } from "./ResearchThreadMessages";
import { ResearchThreadStatus } from "./ResearchThreadStatus";
import type { ChatMessage, ChatRunSummary, ModelConfig } from "../types";
import type { ArtifactRecord } from "../domain/artifact";
import type { ToolProgress } from "../hooks/useChat";
import type { ConnectorPreview } from "../domain/connectors";
import type { RehabTimelineImportOutput } from "../domain/analysisResult";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------
interface ResearchExecutionThreadProps {
  messages: ChatMessage[];
  streaming: string;
  thinking: string;
  sending: boolean;
  latestRunMetrics: ChatRunSummary | null;
  toolProgress?: ToolProgress | null;
  toolProgressHistory?: ToolProgress[];
  error: string | null;
  backendAvailable: boolean;
  /** Search and drafting work without a workspace; persistence/export do not. */
  workspaceSelected?: boolean;
  input: string;
  onInputChange: (v: string) => void;
  onSend: () => void;
  models: ModelConfig[];
  selectedModel: string;
  onModelChange: (model: string) => void;
  thinkingLevel: string;
  onThinkingLevelChange: (level: string) => void;
  artifacts?: ArtifactRecord[];
  onOpenArtifact?: (artifactId: string) => void;
  onOpenSource?: (source: VerifiableSource) => void;
  // Callbacks for thread actions
  onApprove?: (messageId: number) => void;
  onReject?: (messageId: number) => void;
  onViewEvidence?: (messageId: number) => void;
  connectorPreview?: ConnectorPreview | null;
  connectorLoading?: boolean;
  connectorError?: string | null;
  connectorResult?: RehabTimelineImportOutput | null;
  latestAssessments?: number;
  onConfirmConnector?: () => void;
  onDismissConnector?: () => void;
}
// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------
export function ResearchExecutionThread({
  messages,
  streaming,
  thinking,
  sending,
  latestRunMetrics,
  toolProgress,
  toolProgressHistory = [],
  error,
  backendAvailable,
  workspaceSelected = true,
  input,
  onInputChange,
  onSend,
  models,
  selectedModel,
  onModelChange,
  thinkingLevel,
  onThinkingLevelChange,
  onApprove,
  onReject,
  onViewEvidence,
  artifacts = [],
  onOpenArtifact,
  onOpenSource,
  connectorPreview,
  connectorLoading = false,
  connectorError,
  connectorResult,
  latestAssessments,
  onConfirmConnector,
  onDismissConnector,
}: ResearchExecutionThreadProps) {
  // -------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------
  return (
    <div className="execution-thread">
      <ResearchThreadStatus
        backendAvailable={backendAvailable}
        workspaceSelected={workspaceSelected}
        sending={sending}
        streaming={streaming}
        thinking={thinking}
        latestRunMetrics={latestRunMetrics}
        toolProgress={toolProgress}
        toolProgressHistory={toolProgressHistory}
      />

      <ResearchThreadMessages
        messages={messages}
        thinking={thinking}
        streaming={streaming}
        error={error}
        artifacts={artifacts}
        onInputChange={onInputChange}
        onApprove={onApprove}
        onReject={onReject}
        onViewEvidence={onViewEvidence}
        onOpenArtifact={onOpenArtifact}
        onOpenSource={onOpenSource}
      >
        <ConnectorConfirmation
          preview={connectorPreview}
          loading={connectorLoading}
          error={connectorError}
          result={connectorResult}
          latestAssessments={latestAssessments}
          onConfirm={onConfirmConnector}
          onDismiss={onDismissConnector}
        />
      </ResearchThreadMessages>

      <ResearchThreadComposer
        input={input}
        onInputChange={onInputChange}
        onSend={onSend}
        messages={messages}
        models={models}
        selectedModel={selectedModel}
        onModelChange={onModelChange}
        thinkingLevel={thinkingLevel}
        onThinkingLevelChange={onThinkingLevelChange}
        disabled={!backendAvailable}
        sending={sending}
      />
    </div>
  );
}
