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

  useEffect(() => {
    if (!backendAvailable) return;
    invoke<ModeMeta[]>("get_modes").then(setModes).catch(() => {});
    invoke<ChatMode>("get_mode").then(setMode).catch(() => {});
  }, [backendAvailable]);

  const switchMode = useCallback(
    async (newMode: ChatMode) => {
      if (!backendAvailable) return;
      try {
        await invoke("set_mode", { mode: newMode });
        setMode(newMode);
      } catch (e) {
        console.error("Failed to set mode:", e);
      }
    },
    [backendAvailable],
  );

  const meta = modes.find((m) => m.id === mode);
  const label = meta?.label ?? mode;
  const description = meta?.description ?? "";

  return { mode, modes, label, description, switchMode };
}
