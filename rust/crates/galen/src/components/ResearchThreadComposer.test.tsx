// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ResearchThreadComposer } from "./ResearchThreadComposer";

afterEach(cleanup);

function renderComposer(overrides: Partial<React.ComponentProps<typeof ResearchThreadComposer>> = {}) {
  const onSend = vi.fn();
  render(
    <ResearchThreadComposer
      input="分析这组数据"
      onInputChange={() => undefined}
      onSend={onSend}
      messages={[]}
      models={[{ name: "research", model_id: "provider-model" }]}
      selectedModel="research"
      onModelChange={() => undefined}
      thinkingLevel="medium"
      onThinkingLevelChange={() => undefined}
      disabled={false}
      sending={false}
      {...overrides}
    />,
  );
  return onSend;
}

describe("ResearchThreadComposer", () => {
  it("sends on Enter but preserves Shift+Enter for line breaks", () => {
    const onSend = renderComposer();
    const input = screen.getByRole("textbox");

    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
    expect(onSend).not.toHaveBeenCalled();

    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSend).toHaveBeenCalledTimes(1);
  });

  it("shows and disables the model selector when no model is configured", () => {
    renderComposer({ models: [], selectedModel: "" });

    expect((screen.getByRole("button", { name: /未配置模型/ }) as HTMLButtonElement).disabled).toBe(true);
  });
});
