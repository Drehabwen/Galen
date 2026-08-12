// Session node type — mirrors PRD v0.2 §8.4
export interface SessionNode {
  id: string;
  index: string;
  title: string;
  description?: string;
  type: string;
  status: "pending" | "pending_approval" | "approved" | "assigned" | "running" | "blocked" | "completed" | "returned";
  owner?: string;
  inputs?: string[];
  outputs?: string[];
  dependsOn?: string[];
  tags?: string[];
  riskLevel?: "low" | "medium" | "high";
  approvalRequired?: boolean;
  subSessions?: SessionNode[];
  /** Structured outcome attached when the session flows back to the main thread. */
  result?: string;
  /** Key evidence points extracted from the session summary (loop output). */
  evidence?: string[];
}

// Mock data for M1 static UI
export const MOCK_NODES: SessionNode[] = [
  {
    id: "s01",
    index: "01",
    title: "课题定义",
    type: "planning",
    status: "completed",
    owner: "张医生",
    outputs: ["研究方案"],
    riskLevel: "low",
  },
  {
    id: "s02",
    index: "02",
    title: "文献证据检索",
    type: "research",
    status: "completed",
    owner: "李研究生",
    inputs: ["研究方案"],
    outputs: ["证据摘要"],
    dependsOn: ["s01"],
    riskLevel: "low",
  },
  {
    id: "s03",
    index: "03",
    title: "队列构建与纳排",
    type: "data",
    status: "pending_approval",
    owner: "张医生",
    inputs: ["研究方案", "证据摘要"],
    outputs: ["清洁数据集"],
    dependsOn: ["s01", "s02"],
    riskLevel: "medium",
    approvalRequired: true,
    subSessions: [
      {
        id: "s03a",
        index: "03a",
        title: "数据质控",
        type: "data",
        status: "pending",
        owner: "张医生",
        inputs: ["原始数据"],
        outputs: ["质控报告"],
        riskLevel: "medium",
      },
      {
        id: "s03b",
        index: "03b",
        title: "变量校验",
        type: "data",
        status: "pending",
        owner: "李研究生",
        inputs: ["质控报告", "变量字典"],
        outputs: ["校验通过数据"],
        riskLevel: "low",
      },
    ],
  },
  {
    id: "s04",
    index: "04",
    title: "数据清洗与编码",
    type: "data",
    status: "pending",
    owner: "李研究生",
    inputs: ["清洁数据集"],
    outputs: ["分析数据集"],
    dependsOn: ["s03"],
    riskLevel: "medium",
  },
  {
    id: "s05",
    index: "05",
    title: "统计分析",
    type: "analysis",
    status: "pending",
    owner: "王统计师",
    inputs: ["分析数据集"],
    outputs: ["统计结果", "图表"],
    dependsOn: ["s04"],
    riskLevel: "high",
    approvalRequired: true,
    subSessions: [
      {
        id: "s05a",
        index: "05a",
        title: "描述统计与基线表",
        type: "analysis",
        status: "pending",
        outputs: ["Table 1"],
        riskLevel: "low",
      },
      {
        id: "s05b",
        index: "05b",
        title: "Cox 回归与 KM 曲线",
        type: "analysis",
        status: "pending",
        outputs: ["HR 结果", "KM 图"],
        riskLevel: "medium",
      },
    ],
  },
  {
    id: "s06",
    index: "06",
    title: "图表与论文撰写",
    type: "writing",
    status: "pending",
    owner: "张医生",
    inputs: ["统计结果", "图表", "证据摘要"],
    outputs: ["论文草稿"],
    dependsOn: ["s05"],
    riskLevel: "medium",
  },
];
