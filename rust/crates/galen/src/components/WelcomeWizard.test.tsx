// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { WelcomeWizard } from "./WelcomeWizard";

afterEach(cleanup);

function renderWizard(models: Array<{ name: string; model_id: string; description?: string }> = []) {
  const onApiKey = vi.fn(async () => undefined);
  render(
    <WelcomeWizard
      models={models}
      onApiKey={onApiKey}
      onPickWorkspace={async () => null}
      onTestConnection={async () => "ok"}
      onDone={() => undefined}
      hasApiKey={false}
    />,
  );
  return onApiKey;
}

describe("WelcomeWizard model configuration", () => {
  it("uses backend-provided model options", () => {
    renderWizard([{ name: "research-fast", model_id: "provider-model", description: "快速研究" }]);

    expect(screen.getByRole("button", { name: /research-fast/ })).toBeTruthy();
    expect(screen.queryByText("deepseek-v4-flash")).toBeNull();
  });

  it("lets the backend choose its authoritative default when no models are configured", async () => {
    const onApiKey = renderWizard();
    fireEvent.change(screen.getByLabelText("DeepSeek API Key"), { target: { value: "sk-local" } });
    fireEvent.click(screen.getByRole("button", { name: "保存并测试连接" }));

    await waitFor(() => expect(onApiKey).toHaveBeenCalledWith("sk-local", undefined));
  });
});
