import type { SessionNode } from "./sessionTypes";

// Parse a PLAN_START/PLAN_END block from an AI response
export function extractPlan(markdown: string): SessionNode[] | null {
  const match = markdown.match(/<!--\s*PLAN_START\s*-->([\s\S]*?)<!--\s*PLAN_END\s*-->/);
  if (!match) return null;

  const lines = match[1].trim().split("\n").filter(Boolean);
  const nodes: SessionNode[] = [];

  for (const line of lines) {
    // Skip header/separator/markdown-table rows
    if (/^(编号|#|\||\-{3,})/.test(line.trim())) continue;

    // Try pipe-delimited format: 01 | Title | Desc | Deps
    let parts = line.split("|").map((s) => s.trim()).filter(Boolean);
    // Fallback: numbered list "1. Title — Description"
    if (parts.length < 2) {
      const numMatch = line.match(/^(\d{1,2})[\.\s、)]+\s*(.+)/);
      if (!numMatch) continue;
      const num = numMatch[1].padStart(2, "0");
      const rest = numMatch[2];
      const dashIdx = rest.indexOf("—");
      const title = dashIdx > 0 ? rest.slice(0, dashIdx).trim() : rest.slice(0, 50).trim();
      const desc = dashIdx > 0 ? rest.slice(dashIdx + 1).trim() : "";
      nodes.push({
        id: `s${num}`, index: num, title,
        type: "planning",
        status: num === "01" ? "pending_approval" : "pending",
        description: desc || undefined,
      });
      continue;
    }

    const index = parts[0];
    const title = parts[1] ?? "";
    const description = parts[2] ?? "";
    const rawDeps = parts[3] ?? "-";
    const dependsOn = rawDeps === "-" || rawDeps === ""
      ? undefined
      : rawDeps.split(",").map((d) => `s${d.trim()}`);

    if (!title) continue;
    nodes.push({
      id: `s${index}`, index, title,
      type: "planning",
      status: index === "01" ? "pending_approval" : "pending",
      description: description || undefined,
      dependsOn,
    });
  }

  return nodes.length > 0 ? nodes : null;
}

// Check if an AI message contains a plan
export function hasPlan(markdown: string): boolean {
  return /<!--\s*PLAN_START\s*-->/.test(markdown);
}

// Generate a ConfirmationCard message for the plan
export function planConfirmationPrompt(nodes: SessionNode[]): string {
  if (nodes.length === 0) return "未检测到计划节点。";
  const lines = nodes.map(
    (n) => `${n.index}. **${n.title}** — ${n.description ?? ""}${n.dependsOn?.length ? ` (依赖: ${n.dependsOn.join(", ")})` : ""}`
  );
  return `已解析到 ${nodes.length} 个 Session 节点：\n\n${lines.join("\n")}\n\n确认计划后，画布将生成这些节点。要确认吗？`;
}
