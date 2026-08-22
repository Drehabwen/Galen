export interface ArtifactRecord {
  id: string;
  path: string;
  kind: string;
  mimeType: string;
  size: number;
  contentHash: string;
  taskId?: string | null;
  nodeId?: string | null;
  createdAt: string;
  source: string;
}
