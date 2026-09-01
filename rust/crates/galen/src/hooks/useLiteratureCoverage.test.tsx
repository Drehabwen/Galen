// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useLiteratureCoverage } from "./useLiteratureCoverage";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

function deferred<T>() {
  let resolve: (value: T) => void;
  const promise = new Promise<T>((nextResolve) => {
    resolve = nextResolve;
  });
  return { promise, resolve: resolve! };
}

describe("useLiteratureCoverage", () => {
  afterEach(() => vi.resetAllMocks());

  it("ignores an earlier workspace response after the coverage scope changes", async () => {
    const first = deferred<unknown>();
    const second = deferred<unknown>();
    invoke.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    const { result, rerender } = renderHook(
      ({ workspaceRoot, taskId }) => useLiteratureCoverage(true, workspaceRoot, taskId, 0),
      { initialProps: { workspaceRoot: "D:\\research-a", taskId: "task-a" } },
    );

    rerender({ workspaceRoot: "D:\\research-b", taskId: "task-b" });

    await act(async () => {
      second.resolve({ taskId: "task-b", providers: [], hasLimitations: false, limitation: null });
      await Promise.resolve();
    });
    await act(async () => {
      first.resolve({ taskId: "task-a", providers: [], hasLimitations: true, limitation: "stale" });
      await Promise.resolve();
    });

    await waitFor(() => expect(result.current.coverage?.taskId).toBe("task-b"));
  });
});
