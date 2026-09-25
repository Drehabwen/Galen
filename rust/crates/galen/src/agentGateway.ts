import { invoke } from "@tauri-apps/api/core";

export type AgentContextPolicy = "update" | "inherit";

export interface AgentMessageRequest {
  message: string;
  modelAlias: string;
  historyJson: string;
  mode: string;
  personaId: string;
  thinkingLevel: string;
  tag?: string | null;
  contextPolicy?: AgentContextPolicy;
}

export function sendAgentMessage(request: AgentMessageRequest): Promise<void> {
  return invoke("send_message", {
    message: request.message,
    modelAlias: request.modelAlias,
    historyJson: request.historyJson,
    mode: request.mode,
    personaId: request.personaId,
    thinkingLevel: request.thinkingLevel,
    tag: request.tag ?? null,
    contextUpdate: (request.contextPolicy ?? "update") === "update",
  });
}
