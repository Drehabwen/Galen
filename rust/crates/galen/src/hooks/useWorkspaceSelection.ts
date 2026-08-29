import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { isTauriRuntime } from "../tauriRuntime";

export function useWorkspaceSelection() {
  const backendAvailable = isTauriRuntime();
  const [root, setRoot] = useState<string | null>(null);

  useEffect(() => {
    if (!backendAvailable) return;
    let cancelled = false;
    invoke<string | null>("get_workspace_root")
      .then((workspaceRoot) => {
        if (!cancelled && workspaceRoot) setRoot(workspaceRoot);
      })
      .catch(console.error);
    return () => {
      cancelled = true;
    };
  }, [backendAvailable]);

  const pick = useCallback(
    async (beforeSwitch?: () => Promise<void>): Promise<string | null> => {
      const path = await open({
        directory: true,
        multiple: false,
        title: "选择工作区",
      });
      if (!path) return null;
      try {
        await beforeSwitch?.();
        await invoke("set_workspace", { path });
        setRoot(path);
        return path;
      } catch (cause) {
        alert(String(cause));
        return null;
      }
    },
    [],
  );

  const name = root
    ? root.split(/[/\\]/).pop() ?? "未命名"
    : "未选择项目";

  return { root, name, pick };
}
