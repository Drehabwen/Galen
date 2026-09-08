import type { SessionNode } from "./sessionTypes";

export type ResearchTaskStatus =
  | "draft"
  | "ready"
  | "running"
  | "verifying"
  | "deliverable"
  | "blocked";

/** Canonical task snapshot returned by the Rust host. */
export interface ResearchTask {
  schemaVersion: number;
  revision: number;
  taskId: string;
  title: string;
  goal: string;
  status: ResearchTaskStatus;
  createdAt: string;
  updatedAt: string;
  nodes: SessionNode[];
  evidenceIds: string[];
  artifactIds: string[];
}

/** Derived execution view owned by the Rust PI kernel. */
export interface PiSnapshot {
  task: ResearchTask;
  readyNodeIds: string[];
  activeNodeIds: string[];
  blockedNodeIds: string[];
  awaitingDecisionNodeIds: string[];
  lastEventSequence: number;
}
