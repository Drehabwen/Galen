import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ChatMessage } from "../types";
import type { SessionNode } from "../domain/sessionTypes";
import { sendAgentMessage } from "../agentGateway";
import type { AgentContextPolicy } from "../agentGateway";

interface UseSessionChatOptions {
  node: SessionNode;
  backendAvailable: boolean;
  modelAlias: string;
  thinkingLevel?: string;
  autoRun?: boolean;
  onFlowBack?: (node: SessionNode, summary: string) => void;
}

export function buildSessionPrompt(node: SessionNode): string {
  return [
    "【节点上下文包】",
    `节点：${node.index} · ${node.title}`,
    `目标：${node.description || node.title}`,
    node.inputs?.length ? `输入：${node.inputs.join("、")}` : "输入：使用工作区中与本节点直接相关的材料",
    node.outputs?.length ? `验收产物：${node.outputs.join("、")}` : "验收产物：形成可验证的节点结果",
    node.dependsOn?.length ? `已满足依赖：${node.dependsOn.join("、")}` : "依赖：无",
    "执行边界：只处理当前节点；直接执行，不请求批准；工具结果足以满足验收条件后立即停止。",
    "无进展策略：相同方法失败两次后切换策略，不重复相同参数调用。",
    "完成协议：以 [SESSION_DONE] 开头，按“结果 / 已验证事实 / 产物路径 / 局限”输出简短结构化摘要。",
  ].join("\n");
}

export function useSessionChat({
  node,
  backendAvailable,
  modelAlias,
  thinkingLevel,
  autoRun,
  onFlowBack,
}: UseSessionChatOptions) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [streaming, setStreaming] = useState("");
  const [thinking, setThinking] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [restoring, setRestoring] = useState(true);
  const sendingRef = useRef(false);
  const bottomRef = useRef<HTMLDivElement>(null);
  const doneHandledRef = useRef(false);
  const messagesRef = useRef(messages);
  const nodeRef = useRef(node);
  const onFlowBackRef = useRef(onFlowBack);
  const autoRunRef = useRef(false);
  const tag = node.id;

  onFlowBackRef.current = onFlowBack;
  nodeRef.current = node;
  useEffect(() => {
    messagesRef.current = messages;
  }, [messages]);

  useEffect(() => {
    autoRunRef.current = false;
    doneHandledRef.current = false;
    sendingRef.current = false;
    setMessages([]);
    setStreaming("");
    setThinking("");
    setSending(false);
    setError(null);
  }, [tag]);

  useEffect(() => {
    if (!backendAvailable) {
      setRestoring(false);
      return;
    }
    let cancelled = false;
    setRestoring(true);
    invoke<ChatMessage[]>("get_chat_session", { tag })
      .then((restored) => {
        if (!cancelled) setMessages(restored);
      })
      .catch((cause) => {
        if (!cancelled) setError(`恢复节点会话失败: ${String(cause)}`);
      })
      .finally(() => {
        if (!cancelled) setRestoring(false);
      });
    return () => {
      cancelled = true;
    };
  }, [backendAvailable, tag]);

  useEffect(() => {
    if (!backendAvailable) return;
    const unlisteners: UnlistenFn[] = [];
    let cancelled = false;

    const register = async () => {
      unlisteners.push(await listen<string>(`chat-delta:${tag}`, (event) => {
          if (!cancelled) setStreaming((current) => current + event.payload);
        }));
      unlisteners.push(await listen<string>(`chat-done:${tag}`, (event) => {
          if (cancelled || doneHandledRef.current) return;
          doneHandledRef.current = true;
          const content = event.payload;
          setMessages((current) => [...current, { role: "assistant", content, timestamp: Date.now() }]);
          setStreaming("");
          setSending(false);
          sendingRef.current = false;
          if (content.includes("[SESSION_DONE]")) {
            const summary = content.replace(/\[SESSION_DONE\]\s*/, "").trim() || content;
            onFlowBackRef.current?.(nodeRef.current, summary);
          }
        }));
      unlisteners.push(await listen<string>(`chat-error:${tag}`, (event) => {
          if (!cancelled) {
            setError(event.payload);
            setSending(false);
            sendingRef.current = false;
          }
        }));
      unlisteners.push(await listen<string>(`chat-thinking-delta:${tag}`, (event) => {
          if (!cancelled) setThinking((current) => current + event.payload);
        }));
      unlisteners.push(await listen<string>(`chat-thinking-done:${tag}`, () => {
          if (!cancelled) setThinking("");
        }));
    };

    register().catch((cause) => {
      if (!cancelled) setError(`无法注册 Session 事件监听：${String(cause)}`);
    });
    return () => {
      cancelled = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [backendAvailable, tag]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streaming]);

  const sendText = useCallback(async (text: string, contextPolicy: AgentContextPolicy = "update") => {
    if (!text.trim() || sendingRef.current || !backendAvailable) return;
    const trimmed = text.trim();
    setInput("");
    setMessages((current) => [...current, { role: "user", content: trimmed, timestamp: Date.now() }]);
    setSending(true);
    sendingRef.current = true;
    doneHandledRef.current = false;
    setStreaming("");
    setThinking("");
    setError(null);

    try {
      await sendAgentMessage({
        message: trimmed,
        modelAlias,
        historyJson: JSON.stringify(messagesRef.current.slice(-4).map(({ role, content }) => ({ role, content }))),
        mode: "auto",
        personaId: "medical",
        tag,
        thinkingLevel: thinkingLevel || "low",
        contextPolicy,
      });
    } catch (cause) {
      setError(String(cause));
      setSending(false);
      sendingRef.current = false;
    }
  }, [backendAvailable, modelAlias, tag, thinkingLevel]);

  useEffect(() => {
    if (restoring || !autoRun || !backendAvailable || autoRunRef.current) return;
    if (messages.length > 0 || sendingRef.current) return;
    autoRunRef.current = true;
    const timer = setTimeout(() => void sendText(buildSessionPrompt(node), "inherit"), 800);
    return () => clearTimeout(timer);
  }, [autoRun, backendAvailable, node, messages.length, restoring, sendText]);

  const flowBack = useCallback(() => {
    const lastAssistant = [...messages].reverse().find((message) => message.role === "assistant");
    onFlowBack?.(node, lastAssistant?.content ?? "Session 完成。");
  }, [messages, node, onFlowBack]);

  return {
    messages,
    input,
    setInput,
    sending,
    streaming,
    thinking,
    error,
    bottomRef,
    sendText,
    flowBack,
  };
}
