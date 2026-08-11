import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "../tauriRuntime";

export interface Persona {
  id: string;
  label: string;
  description: string;
}

export function usePersona() {
  const backendAvailable = isTauriRuntime();
  const [persona, setPersona] = useState<Persona | null>(null);
  const [allPersonas, setAllPersonas] = useState<Persona[]>([]);

  useEffect(() => {
    if (!backendAvailable) return;
    invoke<Persona[]>("get_personas").then(setAllPersonas).catch(() => {});
    invoke<Persona>("get_persona").then(setPersona).catch(() => {});
  }, [backendAvailable]);

  const switchPersona = useCallback(
    async (personaId: string) => {
      if (!backendAvailable) return;
      try {
        const p = await invoke<Persona>("set_persona", { personaId });
        setPersona(p);
      } catch (e) {
        console.error("Failed to set persona:", e);
      }
    },
    [backendAvailable],
  );

  return { persona, allPersonas, switchPersona };
}
