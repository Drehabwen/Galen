import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ModelConfig, ModelStatus } from "../types";

export function useModelConfiguration(backendAvailable: boolean) {
  const [models, setModels] = useState<ModelConfig[]>([]);
  const [model, setModel] = useState("");
  const [modelStatuses, setModelStatuses] = useState<ModelStatus[]>([]);
  const [showModelStatus, setShowModelStatus] = useState(false);
  const [showWelcome, setShowWelcome] = useState(false);
  const [wizardInitialStep, setWizardInitialStep] = useState(0);
  const [thinkingLevel, setThinkingLevel] = useState<string>(() => {
    const saved = localStorage.getItem("galen.thinkingLevel");
    return !saved || saved === "medium" ? "low" : saved;
  });

  useEffect(() => {
    if (!backendAvailable) return;
    let cancelled = false;

    Promise.all([
      invoke<ModelConfig[]>("get_models"),
      invoke<ModelStatus[]>("get_model_status"),
    ])
      .then(([nextModels, nextStatuses]) => {
        if (cancelled) return;
        setModels(nextModels);
        setModelStatuses(nextStatuses);
        setModel((current) => current || nextModels[0]?.name || "");

        const needsSetup =
          nextModels.length === 0 ||
          (nextStatuses.length > 0 &&
            nextStatuses.every((status) => !status.api_key_present));
        if (needsSetup) setShowWelcome(true);
      })
      .catch(console.error);

    return () => {
      cancelled = true;
    };
  }, [backendAvailable]);

  const handleThinkingLevelChange = useCallback((level: string) => {
    setThinkingLevel(level);
    localStorage.setItem("galen.thinkingLevel", level);
  }, []);

  const handleTestConnection = useCallback(
    () => invoke<string>("test_model_connection"),
    [],
  );

  const handleSaveApiKey = useCallback(
    async (apiKey: string, defaultModel?: string) => {
      await invoke("save_api_key", { apiKey, defaultModel });
      const [nextModels, nextStatuses] = await Promise.all([
        invoke<ModelConfig[]>("get_models"),
        invoke<ModelStatus[]>("get_model_status"),
      ]);
      setModels(nextModels);
      setModelStatuses(nextStatuses);
      setModel((current) => {
        if (
          defaultModel &&
          nextModels.some((item) => item.name === defaultModel)
        ) {
          return defaultModel;
        }
        if (current && nextModels.some((item) => item.name === current)) {
          return current;
        }
        return nextModels[0]?.name ?? "";
      });
    },
    [],
  );

  const openWizard = useCallback((step = 0) => {
    setShowModelStatus(false);
    setWizardInitialStep(step);
    setShowWelcome(true);
  }, []);

  const openModelStatus = useCallback(() => {
    invoke<ModelStatus[]>("get_model_status")
      .then(setModelStatuses)
      .catch(console.error);
    setShowModelStatus(true);
  }, []);

  return {
    models,
    model,
    setModel,
    modelStatuses,
    showModelStatus,
    closeModelStatus: () => setShowModelStatus(false),
    showWelcome,
    setShowWelcome,
    wizardInitialStep,
    thinkingLevel,
    handleThinkingLevelChange,
    handleTestConnection,
    handleSaveApiKey,
    openWizard,
    openModelStatus,
  };
}
