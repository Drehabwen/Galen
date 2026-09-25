import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { sendAgentMessage } from "./agentGateway";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

describe("agentGateway", () => {
  beforeEach(() => vi.mocked(invoke).mockClear());

  it("updates backend context for direct user messages by default", async () => {
    await sendAgentMessage({
      message: "继续修改",
      modelAlias: "research-fast",
      historyJson: "[]",
      mode: "auto",
      personaId: "medical",
      thinkingLevel: "low",
    });

    expect(invoke).toHaveBeenCalledWith("send_message", expect.objectContaining({
      contextUpdate: true,
      tag: null,
    }));
  });

  it("marks orchestration messages as non-authoritative context updates", async () => {
    await sendAgentMessage({
      message: "计划已确认。请执行节点。",
      modelAlias: "research-fast",
      historyJson: "[]",
      mode: "auto",
      personaId: "medical",
      thinkingLevel: "low",
      tag: "node-01",
      contextPolicy: "inherit",
    });

    expect(invoke).toHaveBeenCalledWith("send_message", expect.objectContaining({
      contextUpdate: false,
      tag: "node-01",
    }));
  });
});
