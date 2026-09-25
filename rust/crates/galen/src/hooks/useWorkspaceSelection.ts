import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { isTauriRuntime } from "../tauriRuntime";

export function useWorkspaceSelection() {
  const backendAvailable = isTauriRuntime();
  const [root, setRoot] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!backendAvailable) return;
    let cancelled = false;
    invoke<string | null>("get_workspace_root")
      .then((workspaceRoot) => {
        if (!cancelled) {
          if (workspaceRoot) setRoot(workspaceRoot);
          setError(null);
        }
      })
      .catch((cause) => {
        if (!cancelled) setError(`无法恢复工作区：${String(cause)}`);
      });
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
        setError(null);
        return path;
      } catch (cause) {
        const message = `无法切换工作区：${String(cause)}`;
        setError(message);
        alert(message);
        return null;
      }
    },
    [],
  );

  const name = root
    ? root.split(/[/\\]/).pop() ?? "未命名"
    : "未选择项目";

  return { root, name, error, pick };
}
