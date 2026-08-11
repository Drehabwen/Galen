/**
 * Plan parser tests — run with: npx tsx scripts/test_plan_parser.ts
 */
import { extractPlan, hasPlan } from "../rust/crates/galen/src/domain/planParser";

let passed = 0;
let failed = 0;

function assert(cond: boolean, label: string) {
  if (cond) { passed++; }
  else { console.error(`  ❌ FAIL: ${label}`); failed++; }
}

function assertNode(n: any, index: string, title: string, desc?: string) {
  assert(n.index === index, `node[${index}].index`);
  assert(n.title === title, `node[${index}].title === "${title}"`);
  if (desc !== undefined) assert(n.description === desc, `node[${index}].desc === "${desc}"`);
}

// ── Test 1: Standard pipe format ──
console.log("Test 1: Standard pipe format");
const input1 = `<!-- PLAN_START -->
01 | 课题定义 | 明确研究问题 | -
02 | 文献检索 | PubMed 搜索 | 01
03 | 数据分析 | 统计建模 | 01,02
<!-- PLAN_END -->`;
const nodes1 = extractPlan(input1);
assert(nodes1 !== null, "parses pipe format");
if (nodes1) {
  assert(nodes1.length === 3, "3 nodes");
  assertNode(nodes1[0], "01", "课题定义", "明确研究问题");
  assertNode(nodes1[1], "02", "文献检索", "PubMed 搜索");
  assert(nodes1[1].dependsOn?.join(",") === "s01", "dependsOn: s01");
  assertNode(nodes1[2], "03", "数据分析", "统计建模");
  assert(nodes1[2].dependsOn?.join(",") === "s01,s02", "dependsOn: s01,s02");
}

// ── Test 2: Numbered list fallback ──
console.log("Test 2: Numbered list fallback");
const input2 = `<!-- PLAN_START -->
1. 课题定义 — 明确研究问题和假设
2. 文献检索 — PubMed 搜索 meta 分析
3. 队列构建 — 从数据源提取队列
<!-- PLAN_END -->`;
const nodes2 = extractPlan(input2);
assert(nodes2 !== null, "parses numbered list");
if (nodes2) {
  assert(nodes2.length === 3, "3 nodes");
  assertNode(nodes2[0], "01", "课题定义", "明确研究问题和假设");
  assertNode(nodes2[1], "02", "文献检索", "PubMed 搜索 meta 分析");
}

// ── Test 3: No PLAN markers → null ──
console.log("Test 3: No PLAN markers");
assert(extractPlan("just some text") === null, "returns null for plain text");
assert(extractPlan("") === null, "returns null for empty string");

// ── Test 4: Empty plan block ──
console.log("Test 4: Empty plan block");
assert(extractPlan("<!-- PLAN_START -->\n<!-- PLAN_END -->") === null, "empty block → null");

// ── Test 5: Mixed format (header rows, markdown noise) ──
console.log("Test 5: Mixed format with headers");
const input5 = `<!-- PLAN_START -->
编号 | 标题 | 描述 | 依赖
---|---|---|---
01 | 课题设计 | 研究方案 | -
02 | 数据采集 | 录入和质控 | 01
<!-- PLAN_END -->`;
const nodes5 = extractPlan(input5);
assert(nodes5 !== null, "parses markdown table");
if (nodes5) {
  assert(nodes5.length === 2, "2 nodes (skipped header row)");
  assertNode(nodes5[0], "01", "课题设计", "研究方案");
}

// ── Test 6: hasPlan detection ──
console.log("Test 6: hasPlan detection");
assert(hasPlan(input1), "detects PLAN_START");
assert(!hasPlan("no plan here"), "no false positive");

// ── Test 7: Markdown inside plan (real AI output) ──
console.log("Test 7: Realistic AI output");
const input7 = `好的，我为你制定了以下研究计划：

<!-- PLAN_START -->
01 | 课题定义 | 明确 PICO、假设、研究类型 | -
02 | 文献检索 | PubMed + 指南检索 | 01
03 | 数据准备 | 提取、纳排、清洗 | 01,02
04 | 统计分析 | 描述统计 + Cox 回归 | 03
05 | 论文撰写 | 方法学/结果/讨论 | 04
<!-- PLAN_END -->

确认计划？还是需要调整？`;
const nodes7 = extractPlan(input7);
assert(nodes7 !== null, "parses realistic AI output");
if (nodes7) {
  assert(nodes7.length === 5, "5 nodes");
  assert(nodes7[0].status === "pending_approval", "first node needs approval");
  assert(nodes7[4].status === "pending", "last node is pending");
}

// ── Results ──
console.log(`\n${passed} passed, ${failed} failed`);
if (failed > 0) process.exit(1);
