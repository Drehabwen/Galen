import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "../tauriRuntime";

export type ChatMode = "plan" | "auto";

export interface ModeMeta {
  id: string;
  label: string;
  description: string;
}

export function useMode() {
  const backendAvailable = isTauriRuntime();
  const [mode, setMode] = useState<ChatMode>("auto");
  const [modes, setModes] = useState<ModeMeta[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!backendAvailable) return;
    let cancelled = false;
    Promise.all([
      invoke<ModeMeta[]>("get_modes"),
      invoke<ChatMode>("get_mode"),
    ])
      .then(([nextModes, nextMode]) => {
        if (cancelled) return;
        setModes(nextModes);
        setMode(nextMode);
        setError(null);
      })
      .catch((cause) => {
        if (!cancelled) setError(`无法加载工作模式：${String(cause)}`);
      });
    return () => {
      cancelled = true;
    };
  }, [backendAvailable]);

  const switchMode = useCallback(
    async (newMode: ChatMode) => {
      if (!backendAvailable) return;
      try {
        await invoke("set_mode", { mode: newMode });
        setMode(newMode);
        setError(null);
      } catch (e) {
        setError(`无法切换工作模式：${String(e)}`);
      }
    },
    [backendAvailable],
  );

  const meta = modes.find((m) => m.id === mode);
  const label = meta?.label ?? mode;
  const description = meta?.description ?? "";

  return { mode, modes, label, description, error, switchMode };
}
