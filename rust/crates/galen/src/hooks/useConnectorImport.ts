import { useCallback, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ArtifactRecord } from "../domain/artifact";
import type { RehabTimelineImportOutput } from "../domain/analysisResult";
import {
  parseConnectorIntent,
  type ConnectorIntent,
  type ConnectorPreview,
} from "../domain/connectors";

interface LocalMessage {
  role: "user" | "assistant";
  content: string;
  timestamp: number;
}

interface ConnectorImportOptions {
  backendAvailable: boolean;
  workspaceRoot: string | null;
  appendLocalMessage: (message: LocalMessage) => void;
  acceptArtifact: (artifact: ArtifactRecord) => void;
  openCase: (caseId: string) => Promise<void>;
}

export function useConnectorImport({
  backendAvailable,
  workspaceRoot,
  appendLocalMessage,
  acceptArtifact,
  openCase,
}: ConnectorImportOptions) {
  const [intent, setIntent] = useState<ConnectorIntent | null>(null);
  const [preview, setPreview] = useState<ConnectorPreview | null>(null);
  const [result, setResult] = useState<RehabTimelineImportOutput | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const discoveryRevision = useRef(0);

  const reset = useCallback(() => {
    discoveryRevision.current += 1;
    setIntent(null);
    setPreview(null);
    setResult(null);
    setError(null);
    setLoading(false);
  }, []);

  const clearCanvas = useCallback(() => {
    setPreview(null);
    setResult(null);
  }, []);

  const tryHandlePrompt = useCallback(
    async (message: string): Promise<boolean> => {
      const nextIntent = parseConnectorIntent(message);
      if (!nextIntent) return false;
      const revision = ++discoveryRevision.current;

      setIntent(nextIntent);
      setPreview(null);
      setResult(null);
      setError(null);
      appendLocalMessage({ role: "user", content: message, timestamp: Date.now() });
      if (!backendAvailable) {
        setError("当前是浏览器预览模式；请启动 Galen 桌面端后再读取本地工作台数据。");
        return true;
      }

      setLoading(true);
      try {
        const nextPreview = await invoke<ConnectorPreview>("discover_research_data_source", {
            sourceId: nextIntent.sourceId,
            caseHint: nextIntent.caseHint ?? null,
          });
        if (revision === discoveryRevision.current) setPreview(nextPreview);
      } catch (cause) {
        if (revision === discoveryRevision.current) setError(String(cause));
      } finally {
        if (revision === discoveryRevision.current) setLoading(false);
      }
      return true;
    },
    [appendLocalMessage, backendAvailable],
  );

  const confirm = useCallback(async () => {
    if (!intent || !preview || !preview.canImport) return;
    if (!workspaceRoot) {
      setError("请先选择研究工作区，再将数据写入 RehabID。");
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const imported = await invoke<RehabTimelineImportOutput>("import_research_data_source", {
        request: {
          sourceId: intent.sourceId,
          exportPath: preview.exportPath,
          caseIds: preview.cases.map((item) => item.caseId),
          latestAssessments: intent.latestAssessments ?? null,
        },
      });
      setResult(imported);
      acceptArtifact(imported.receipt);
      if (imported.caseIds[0]) await openCase(imported.caseIds[0]);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  }, [acceptArtifact, intent, openCase, preview, workspaceRoot]);

  return {
    intent,
    preview,
    result,
    loading,
    error,
    tryHandlePrompt,
    confirm,
    reset,
    clearCanvas,
  };
}
