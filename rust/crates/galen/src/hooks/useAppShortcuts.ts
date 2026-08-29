import { useEffect } from "react";
import type { ChatMode, ModeMeta } from "./useMode";

export function useAppShortcuts(
  modes: ModeMeta[],
  switchMode: (mode: ChatMode) => Promise<void>,
  clearChat: () => void,
) {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        !event.ctrlKey ||
        event.shiftKey ||
        event.altKey ||
        event.metaKey
      ) {
        return;
      }

      const modeIndex = Number(event.key) - 1;
      if (modeIndex >= 0 && modeIndex <= 2 && modes[modeIndex]) {
        void switchMode(modes[modeIndex].id as ChatMode);
        event.preventDefault();
      } else if (event.key === "l") {
        clearChat();
        event.preventDefault();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [clearChat, modes, switchMode]);
}
