import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ChatMessage, Paper, FileEntry } from "../types";
import { isTauriRuntime } from "../tauriRuntime";

export function useChat() {
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
  const currentModel = useRef<string>("");
  const sendingRef = useRef(false);
  const doneHandledRef = useRef(false); // prevent duplicate done handling

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
      unlisteners.push(ul1, ul2, ul3, ul4, ul5, ul6, ul7, ul8);
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
          thinkingLevel: thinkingLevel || "medium",
        });
      } catch (e) {
        setError(String(e));
        setSending(false);
        sendingRef.current = false;
      }
    },
    [backendAvailable]
  );

  const clear = useCallback(() => {
    setMessages([]);
    setStreaming("");
    setThinking("");
    setThinkingHistory({});
    setError(null);
    setSearchResults([]);
  }, []);

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
    send,
    clear,
  };
}
