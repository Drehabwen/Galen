import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ChatMessage, Paper, FileEntry } from "../types";

export function useChat() {
  const [messages, setMessages] = useState<ChatMessage[]>([
    {
      role: "assistant",
      content:
        "欢迎使用 VIBE Paper！我是你的科研助手。\n\n你可以直接问我问题，我会帮你检索文献、解释术语、格式化引用。\n\n试试问我：\n• 帮我查一下阿尔茨海默病的最新研究\n• 解释一下什么是单核苷酸多态性\n• 用 Vancouver 格式引用这篇 PMID: 12345678",
    },
  ]);
  const [streaming, setStreaming] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchResults, setSearchResults] = useState<Paper[]>([]);
  const [wsFileList, setWsFileList] = useState<FileEntry[]>([]);
  const [wsFileContent, setWsFileContent] = useState<{
    path: string;
    content: string;
  } | null>(null);

  const send = useCallback(
    async (text: string, modelAlias: string) => {
      if (!text.trim() || sending) return;

      setMessages((prev) => [...prev, { role: "user", content: text }]);
      setSending(true);
      setStreaming("");
      setError(null);

      // Set up event listeners before the invoke call
      const p1 = listen<string>("chat-delta", (e) => {
        setStreaming((prev) => prev + e.payload);
      });
      const p2 = listen<string>("chat-done", (e) => {
        setMessages((prev) => [
          ...prev,
          { role: "assistant", content: e.payload },
        ]);
        setStreaming("");
        setSending(false);
      });
      const p3 = listen<string>("chat-error", (e) => {
        setError(e.payload);
        setSending(false);
      });
      const p4 = listen<Paper[]>("search-results", (e) => {
        setSearchResults(e.payload);
      });
      const p5 = listen<FileEntry[]>("workspace-file-list", (e) => {
        setWsFileList(e.payload);
      });
      const p6 = listen<{ path: string; content: string }>(
        "workspace-file-content",
        (e) => {
          setWsFileContent(e.payload);
        }
      );

      try {
        await invoke("send_message", {
          message: text,
          modelAlias: modelAlias,
        });
      } catch (e) {
        setError(String(e));
        setSending(false);
      }
    },
    [sending]
  );

  const clear = useCallback(() => {
    setMessages([
      {
        role: "assistant",
        content: "新对话已开始。有什么可以帮你的？",
      },
    ]);
    setStreaming("");
    setError(null);
    setSearchResults([]);
  }, []);

  return {
    messages,
    streaming,
    sending,
    error,
    searchResults,
    wsFileList,
    wsFileContent,
    send,
    clear,
  };
}
