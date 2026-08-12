import type { RuntimeStatus, McpServerStatus } from "../hooks/useEnvironment";
import type { ChatMode } from "../hooks/useMode";

interface Props {
  statusMessage: string;
  environment?: RuntimeStatus | null;
  mcpServers?: McpServerStatus[];
  mode?: ChatMode;
  modeLabel?: string;
}

export function StatusBar({ statusMessage, environment, mcpServers, mode, modeLabel }: Props) {
  const mcpOnline = mcpServers?.filter((s) => s.connected).length ?? 0;
  const mcpTotal = mcpServers?.length ?? 0;

  const envParts: string[] = [];
  if (environment) {
    if (environment.python.installed) envParts.push("Python");
    if (environment.r.installed) envParts.push("R");
    if (environment.typst.installed) envParts.push("Typst");
  }

  return (
    <div className="bottom-bar status-bar-compact">
      {mode && modeLabel && (
        <>
          <span className={`status-mode mode-${mode}`} title="点击顶栏模式按钮或 Ctrl+1/2/3 切换模式">
            {modeLabel}
          </span>
          <span className="status-separator">|</span>
        </>
      )}
      <span className="status-stat">{envParts.length > 0 ? envParts.join(" + ") : "环境检测中"}</span>
      {mcpTotal > 0 && (
        <>
          <span className="status-separator">|</span>
          <span className="status-stat">MCP {mcpOnline}/{mcpTotal}</span>
        </>
      )}
      <span className="status-spacer" />
      <span className="status-message">{statusMessage}</span>
    </div>
  );
}
