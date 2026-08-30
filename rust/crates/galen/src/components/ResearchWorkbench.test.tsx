// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ResearchWorkbench } from "./ResearchWorkbench";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => []),
}));

describe("ResearchWorkbench", () => {
  afterEach(() => cleanup());

  it("presents the research lifecycle, evidence alignment, and outputs as the primary workspace", () => {
    render(
      <ResearchWorkbench
        wsRoot="D:\\DEV\\fatigue-shortcut-audit"
        files={[]}
        currentFile={null}
        backendAvailable
        onAgentPrompt={() => {}}
        onReadFile={() => {}}
      />,
    );

    expect(screen.getByRole("heading", { name: "运动疲劳中的时间捷径与跨受试者泛化" })).toBeTruthy();
    expect(screen.getByText("误差解释")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "跨受试者误差分布" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "关键发现" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "受试者 27 · 证据对齐" })).toBeTruthy();
    expect(screen.getByRole("complementary", { name: "证据链与研究产物" })).toBeTruthy();
    expect(screen.queryByText(/独立临床诊断|非临床诊断/)).toBeNull();
  });

  it("turns researcher decisions and the command bar into agent prompts", () => {
    const onAgentPrompt = vi.fn();
    render(
      <ResearchWorkbench
        wsRoot="D:\\DEV\\fatigue-shortcut-audit"
        files={[]}
        currentFile={null}
        backendAvailable
        onAgentPrompt={onAgentPrompt}
        onReadFile={() => {}}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "保留并分层分析" }));
    expect(onAgentPrompt).toHaveBeenCalledWith(expect.stringContaining("分层分析"));

    fireEvent.change(screen.getByPlaceholderText("询问数据、运行分析或生成研究产物…"), {
      target: { value: "重新运行 LOSO 验证" },
    });
    fireEvent.click(screen.getByRole("button", { name: "发送研究指令" }));
    expect(onAgentPrompt).toHaveBeenLastCalledWith("重新运行 LOSO 验证");
  });

  it("opens the latest registered PDF instead of sending a chat prompt", () => {
    const onOpenReport = vi.fn();
    render(
      <ResearchWorkbench
        wsRoot="D:\\DEV\\fatigue-shortcut-audit"
        files={[]}
        currentFile={null}
        backendAvailable
        reportAvailable
        onOpenReport={onOpenReport}
        onAgentPrompt={() => {}}
        onReadFile={() => {}}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "打开可复现报告" }));
    expect(onOpenReport).toHaveBeenCalledOnce();
  });

  it("disables report opening when no PDF is registered", () => {
    render(
      <ResearchWorkbench
        wsRoot="D:\\DEV\\fatigue-shortcut-audit"
        files={[]}
        currentFile={null}
        backendAvailable
        reportAvailable={false}
        onOpenReport={() => {}}
        onAgentPrompt={() => {}}
        onReadFile={() => {}}
      />,
    );

    expect(screen.getByRole("button", { name: "尚未生成 PDF 报告" })).toHaveProperty("disabled", true);
  });
});
