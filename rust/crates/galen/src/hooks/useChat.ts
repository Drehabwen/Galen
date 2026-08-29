import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ChatMessage, Paper, FileEntry, ChatRunSummary } from "../types";
import type { ArtifactRecord } from "../domain/artifact";
import type { ResearchTask } from "../domain/researchTask";
import { isTauriRuntime } from "../tauriRuntime";

export function useChat(workspaceRoot: string | null) {
  const backendAvailable = isTauriRuntime();
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [streaming, setStreaming] = useState("");
  const [thinking, setThinking] = useState("");
  const [thinkingHistory, setThinkingHistory] = useState<Record<number, string>>({});
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchResults, setSearchResults] = useState<Paper[]>([]);
  const [wsFileList, setWsFileList] = useState<FileEntry[]>([]);
  const [wsFileContent, setWsFileContent] = useState<{
    path: string;
    content: string;
  } | null>(null);
  const [latestArtifact, setLatestArtifact] = useState<ArtifactRecord | null>(null);
  const [researchTaskUpdate, setResearchTaskUpdate] = useState<ResearchTask | null>(null);
  const [latestRunMetrics, setLatestRunMetrics] = useState<ChatRunSummary | null>(null);
  const currentModel = useRef<string>("");
  const sendingRef = useRef(false);
  const doneHandledRef = useRef(false); // prevent duplicate done handling

  // Restore the workspace-scoped durable main session. The backend remains
  // authoritative; React state is only the current rendering projection.
  useEffect(() => {
    if (!backendAvailable || !workspaceRoot) {
      setMessages([]);
      setLatestArtifact(null);
      setResearchTaskUpdate(null);
      setLatestRunMetrics(null);
      return;
    }
    let cancelled = false;
    invoke<ChatMessage[]>("get_chat_session", { tag: null })
      .then((restored) => {
        if (!cancelled && !sendingRef.current) setMessages(restored);
      })
      .catch((cause) => {
        if (!cancelled) setError(`恢复主会话失败: ${String(cause)}`);
      });
    return () => {
      cancelled = true;
    };
  }, [backendAvailable, workspaceRoot]);

  // Register event listeners once on mount, clean up on unmount
  useEffect(() => {
    if (!backendAvailable) return;

    // Track unlisteners synchronously to avoid cleanup race with async registration
    const unlisteners: UnlistenFn[] = [];
    let cancelled = false;

    const register = async () => {
      const ul1 = await listen<string>("chat-delta", (e) => {
        if (!cancelled) setStreaming((prev) => prev + e.payload);
      });
      const ul2 = await listen<string>("chat-done", (e) => {
        if (cancelled) return;
        // Guard against duplicate done events from listener leaks
        if (doneHandledRef.current) return;
        doneHandledRef.current = true;
        setMessages((prev) => [
          ...prev,
          {
            role: "assistant",
            content: e.payload,
            timestamp: Date.now(),
            model: currentModel.current,
          },
        ]);
        setStreaming("");
        setThinking("");
        setSending(false);
        sendingRef.current = false;
      });
      const ul3 = await listen<string>("chat-error", (e) => {
        if (!cancelled) {
          setError(e.payload);
          setSending(false);
          sendingRef.current = false;
          doneHandledRef.current = false; // allow retry after error
        }
      });
      const ul4 = await listen<Paper[]>("search-results", (e) => {
        if (!cancelled) setSearchResults(e.payload);
      });
      const ul5 = await listen<FileEntry[]>("workspace-file-list", (e) => {
        if (!cancelled) setWsFileList(e.payload);
      });
      const ul6 = await listen<{ path: string; content: string }>(
        "workspace-file-content",
        (e) => {
          if (!cancelled) setWsFileContent(e.payload);
        }
      );
      const ul7 = await listen<string>("chat-thinking-delta", (e) => {
        if (!cancelled) setThinking((prev) => prev + e.payload);
      });
      const ul8 = await listen<string>("chat-thinking-done", (e) => {
        if (!cancelled) {
          setThinkingHistory((prev) => ({ ...prev, [Date.now()]: e.payload }));
          setThinking("");
        }
      });
      const ul9 = await listen<ArtifactRecord>("artifact-created", (e) => {
        if (!cancelled) setLatestArtifact(e.payload);
      });
      const ul10 = await listen<ResearchTask>("research-task-updated", (e) => {
        if (!cancelled) setResearchTaskUpdate(e.payload);
      });
      const ul11 = await listen<ChatRunSummary>("chat-run-metrics", (e) => {
        if (!cancelled) setLatestRunMetrics(e.payload);
      });
      unlisteners.push(ul1, ul2, ul3, ul4, ul5, ul6, ul7, ul8, ul9, ul10, ul11);
    };

    register().catch((e) => {
      setError(`Agent 后端事件监听失败: ${String(e)}`);
    });

    return () => {
      cancelled = true;
      unlisteners.forEach((fn) => fn());
    };
  }, [backendAvailable]);

  const send = useCallback(
    async (
      text: string,
      modelAlias: string,
      mode?: string,
      personaId?: string,
      thinkingLevel?: string,
    ) => {
      if (!text.trim() || sendingRef.current) return;

      if (!backendAvailable) {
        setError("当前是浏览器预览模式，Tauri 后端未连接。请用 `npm run tauri dev` 启动桌面应用后再使用 Agent。");
        return;
      }

      currentModel.current = modelAlias;

      setMessages((prev) => [
        ...prev,
        { role: "user", content: text, timestamp: Date.now() },
      ]);
      setSending(true);
      sendingRef.current = true;
      doneHandledRef.current = false; // reset for new message
      setStreaming("");
      setThinking("");
      setError(null);

      try {
        const recentMessages = messages.slice(-4).map((m) => ({
          role: m.role,
          content: m.content,
        }));
        const historyJson = JSON.stringify(recentMessages);
        await invoke("send_message", {
          message: text,
          modelAlias: modelAlias,
          historyJson: historyJson,
          mode: mode || "discuss",
          personaId: personaId || "medical",
          thinkingLevel: thinkingLevel || "low",
        });
      } catch (e) {
        setError(String(e));
        setSending(false);
        sendingRef.current = false;
      }
    },
    // `messages` is intentionally a dependency. The backend session is the
    // durable source of truth, but this recent-history fallback must reflect
    // the latest rendered conversation if persistence is unavailable or a
    // legacy session is being migrated.
    [backendAvailable, messages]
  );

  const clear = useCallback(() => {
    setMessages([]);
    setStreaming("");
    setThinking("");
    setThinkingHistory({});
    setLatestRunMetrics(null);
    setError(null);
    setSearchResults([]);
    if (backendAvailable && workspaceRoot) {
      invoke("clear_chat_session", { tag: null }).catch((cause) => {
        setError(`归档主会话失败: ${String(cause)}`);
      });
    }
  }, [backendAvailable, workspaceRoot]);

  return {
    messages,
    streaming,
    thinking,
    thinkingHistory,
    sending,
    error,
    backendAvailable,
    searchResults,
    wsFileList,
    wsFileContent,
    latestArtifact,
    researchTaskUpdate,
    latestRunMetrics,
    send,
    clear,
  };
}
