import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "../tauriRuntime";

export interface RuntimeInfo {
  installed: boolean;
  version: string | null;
  path: string | null;
  install_guide: string | null;
}

export interface RuntimeStatus {
  python: RuntimeInfo;
  r: RuntimeInfo;
  typst: RuntimeInfo;
  deno: RuntimeInfo;
  uvx: RuntimeInfo;
}

export interface McpServerStatus {
  name: string;
  connected: boolean;
  status: string;
  tool_count: number;
}

export interface CapabilityManifest {
  id: string;
  name: string;
  version: string;
  layer: "kernel" | "workbench" | "domain";
  description: string;
  toolNames: string[];
  uiSlots: Array<"top_bar" | "resource_bar" | "inspector" | "settings">;
  contextModules: string[];
  enabled: boolean;
}

const DEFAULT_STATUS: RuntimeStatus = {
  python: { installed: false, version: null, path: null, install_guide: null },
  r: { installed: false, version: null, path: null, install_guide: null },
  typst: { installed: false, version: null, path: null, install_guide: null },
  deno: { installed: false, version: null, path: null, install_guide: null },
  uvx: { installed: false, version: null, path: null, install_guide: null },
};

export function useEnvironment() {
  const backendAvailable = isTauriRuntime();
  const [status, setStatus] = useState<RuntimeStatus>(DEFAULT_STATUS);
  const [mcpServers, setMcpServers] = useState<McpServerStatus[]>([]);
  const [capabilities, setCapabilities] = useState<CapabilityManifest[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!backendAvailable) {
      setLoading(false);
      return;
    }

    Promise.all([
      invoke<RuntimeStatus>("get_runtime_status"),
      invoke<McpServerStatus[]>("get_mcp_status"),
      invoke<CapabilityManifest[]>("get_capabilities"),
    ])
      .then(([s, m, c]) => {
        setStatus(s);
        setMcpServers(m);
        setCapabilities(c);
      })
      .catch((e) => {
        console.error("Failed to detect environment:", e);
      })
      .finally(() => setLoading(false));
  }, [backendAvailable]);

  return { status, mcpServers, capabilities, loading };
}

export function statusLine(status: RuntimeStatus): string {
  const parts: string[] = [];
  if (status.python.installed) parts.push("Py");
  if (status.r.installed) parts.push("R");
  if (status.typst.installed) parts.push("Typst");
  return parts.length > 0 ? parts.join(" + ") : "未检测到运行时";
}

export function missingRuntimes(status: RuntimeStatus): string[] {
  const missing: string[] = [];
  if (!status.python.installed) missing.push("Python");
  if (!status.r.installed) missing.push("R");
  if (!status.typst.installed) missing.push("Typst");
  return missing;
}
