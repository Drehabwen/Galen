import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ArtifactRecord } from "../domain/artifact";
import {
  mergeDeliveredArtifact,
  resolveArtifactNodeTitle,
} from "../domain/deliveryLoop";
import type { SessionNode } from "../domain/sessionTypes";
import { BINARY_PREVIEW_KINDS, classifyPreviewKind } from "../domain/preview";
import type { ArtifactPreview } from "../domain/preview";
import type { useChat } from "./useChat";
import type { useResearchTask } from "./useResearchTask";

type ChatController = ReturnType<typeof useChat>;
type ResearchController = ReturnType<typeof useResearchTask>;
export type CanvasTab = "plan" | "doc";

export function useArtifactDelivery(
  backendAvailable: boolean,
  workspaceRoot: string | null,
  chat: ChatController,
  research: ResearchController,
) {
  const [canvasTab, setCanvasTab] = useState<CanvasTab>("plan");
  const [preview, setPreview] = useState<ArtifactPreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [artifacts, setArtifacts] = useState<ArtifactRecord[]>([]);
  const readRevisionRef = useRef(0);

  useEffect(() => {
    if (!backendAvailable || !workspaceRoot) {
      setArtifacts([]);
      return;
    }
    invoke<ArtifactRecord[]>("get_artifacts")
      .then(setArtifacts)
      .catch(() => setArtifacts([]));
  }, [backendAvailable, workspaceRoot]);

  useEffect(() => {
    if (chat.researchTaskUpdate) {
      research.acceptSnapshot(chat.researchTaskUpdate);
    }
  }, [chat.researchTaskUpdate, research.acceptSnapshot]);

  const readArtifact = useCallback(
    async (path: string, nodeTitle?: string) => {
      const revision = ++readRevisionRef.current;
      setLoading(true);
      setError(null);
      setCanvasTab("doc");
      try {
        const kind = classifyPreviewKind(path);
        if (BINARY_PREVIEW_KINDS.has(kind)) {
          const buffer = await invoke<ArrayBuffer>("read_artifact_bytes", { path });
          if (revision === readRevisionRef.current) {
            setPreview({
              path,
              kind,
              blob: new Blob([buffer], {
                type: kind === "pdf" ? "application/pdf" : "",
              }),
              nodeTitle,
            });
          }
        } else {
          const content = await invoke<string>("read_workspace_file", { path });
          if (revision === readRevisionRef.current) {
            setPreview({ path, kind, content, nodeTitle });
          }
        }
      } catch (cause) {
        if (revision === readRevisionRef.current) {
          setPreview(null);
          setError(String(cause));
        }
      } finally {
        if (revision === readRevisionRef.current) setLoading(false);
      }
    },
    [],
  );

  useEffect(() => {
    const artifact = chat.latestArtifact;
    if (!artifact) return;
    setArtifacts((current) => mergeDeliveredArtifact(current, artifact));
    const nodeTitle = resolveArtifactNodeTitle(
      artifact,
      chat.researchTaskUpdate,
    );
    void readArtifact(artifact.path, nodeTitle);
  }, [chat.latestArtifact, chat.researchTaskUpdate, readArtifact]);

  const previewNodeArtifact = useCallback(
    (path: string, node: SessionNode) => readArtifact(path, node.title),
    [readArtifact],
  );

  const openRegisteredArtifact = useCallback(
    (artifact: ArtifactRecord) => {
      const nodeTitle = research.nodes.find(
        (node) => node.id === artifact.nodeId,
      )?.title;
      return readArtifact(artifact.path, nodeTitle);
    },
    [readArtifact, research.nodes],
  );

  return {
    canvasTab,
    setCanvasTab,
    preview,
    loading,
    error,
    artifacts,
    previewNodeArtifact,
    openRegisteredArtifact,
  };
}
