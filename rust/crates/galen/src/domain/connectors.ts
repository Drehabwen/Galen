/** Research data-source registry. The therapist workbench is the first
 * production adapter; the others stay visible as the expansion surface.
 */
export type ConnectorKind = "local_export" | "http_api" | "device_file";

export interface FirstPartyConnector {
  id: string;
  label: string;
  kind: ConnectorKind;
  status: "ready" | "adapter_needed";
  sourcePath?: string;
  endpoints?: string[];
  exportFormats: string[];
  normalizedEntities: string[];
  notes: string;
}

export const FIRST_PARTY_CONNECTORS: FirstPartyConnector[] = [
  {
    id: "rehab-workbench",
    label: "康复师工作台",
    kind: "local_export",
    status: "ready",
    sourcePath: "D:/DEV/Rehab",
    exportFormats: ["JSON"],
    normalizedEntities: ["RehabID", "session", "assessment", "measurement"],
    notes: "已接通完整备份结构；在对话中说出对象和时间范围即可发现并确认获取。",
  },
  {
    id: "qingyue-workbench",
    label: "青跃康复工作台",
    kind: "local_export",
    status: "adapter_needed",
    sourcePath: "D:/DEV/QingYueRehabWorkbench",
    exportFormats: ["JSON"],
    normalizedEntities: ["patient", "assessment", "session"],
    notes: "保留为扩展数据源，不作为当前主工作台。",
  },
  {
    id: "rehabgpt",
    label: "RehabGPT 患者端",
    kind: "http_api",
    status: "adapter_needed",
    sourcePath: "D:/DEV/RehabGPT-",
    endpoints: [
      "/api/chatbot/assessment-history/:name",
      "/api/chatbot/assessment-trend/:name/:tool",
      "/api/integration/tracking/:patientId",
      "/api/integration/scale/results/:sessionId",
    ],
    exportFormats: ["JSON"],
    normalizedEntities: ["assessment", "tracking", "scale", "session_summary"],
    notes: "先由用户明确选择患者和时间范围，再通过本地服务读取脱敏结果。",
  },
];

export function getFirstPartyConnector(id: string): FirstPartyConnector | undefined {
  return FIRST_PARTY_CONNECTORS.find((connector) => connector.id === id);
}

export interface ConnectorCasePreview {
  caseId: string;
  displayName: string;
  sessionCount: number;
  assessmentCount: number;
  timepointCount: number;
  measurementCount: number;
}

export interface ConnectorPreview {
  sourceId: string;
  sourceLabel: string;
  exportPath: string;
  exportedAt: number;
  connectionMode: "live_bridge" | "file_fallback";
  patientCount: number;
  sessionCount: number;
  assessmentCount: number;
  timepointCount: number;
  measurementCount: number;
  cases: ConnectorCasePreview[];
  canImport: boolean;
  message: string;
}

export interface ConnectorIntent {
  sourceId: "rehab-workbench";
  caseHint?: string;
  latestAssessments?: number;
}

export function parseConnectorIntent(text: string): ConnectorIntent | null {
  const normalized = text.trim();
  const mentionsSource = /康复师工作台|Rehab\s*工作台|Rehab\s*评估系统/i.test(normalized);
  const requestsData = /获取|读取|导入|同步|接入|连接/.test(normalized) && /数据|评估|记录|RehabID/i.test(normalized);
  if (!mentionsSource || !requestsData) return null;
  const caseHint = normalized.match(/\b[A-Z]{2,}(?:[-_][A-Z0-9]+)+\b/i)?.[0];
  const latestAssessments = /最近(?:三|3)次/.test(normalized)
    ? 3
    : /最近(?:两|二|2)次/.test(normalized)
      ? 2
      : /最近(?:一|1)次/.test(normalized)
        ? 1
        : undefined;
  return { sourceId: "rehab-workbench", caseHint, latestAssessments };
}
