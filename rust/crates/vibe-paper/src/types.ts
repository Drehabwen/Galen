export interface ModelConfig {
  name: string;
  model_id: string;
}

export interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
}

export interface Paper {
  pmid: string;
  title: string;
  authors: string[];
  journal: string | null;
  year: string | null;
  doi: string | null;
  abstract_text: string | null;
}

export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
}

export type LeftTab = "workspace" | "papers" | "notes";
