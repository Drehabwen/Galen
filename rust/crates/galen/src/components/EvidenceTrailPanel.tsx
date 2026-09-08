import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArtifactMarkdown } from "./ArtifactMarkdown";
import { EvidenceCoverageCard } from "./EvidenceCoverageCard";
import type { VerifiableSource } from "./SourceInspector";
import type { ArtifactRecord } from "../domain/artifact";
import { useLiteratureCoverage } from "../hooks/useLiteratureCoverage";

type Evidence = {
  id: string;
  node_id: string;
  node_title: string;
  source: string;
  claim: string;
  detail?: string;
  confidence: "high" | "medium" | "low" | string;
  created_at: string;
};

type ReviewFlow = {
  taskId?: string | null;
  identified: number;
  duplicatesRemoved?: number | null;
  screened?: number | null;
  excluded?: number | null;
  fullTextAssessed?: number | null;
  included?: number | null;
  updatedAt?: string | null;
};

type ReviewFlowDraft = Record<"duplicatesRemoved" | "screened" | "excluded" | "fullTextAssessed" | "included", string>;

const emptyDraft = (): ReviewFlowDraft => ({ duplicatesRemoved: "", screened: "", excluded: "", fullTextAssessed: "", included: "" });

function draftFromFlow(flow: ReviewFlow): ReviewFlowDraft {
  return {
    duplicatesRemoved: flow.duplicatesRemoved?.toString() ?? "",
    screened: flow.screened?.toString() ?? "",
    excluded: flow.excluded?.toString() ?? "",
    fullTextAssessed: flow.fullTextAssessed?.toString() ?? "",
    included: flow.included?.toString() ?? "",
  };
}

function optionalCount(value: string): number | undefined {
  const count = Number(value);
  return Number.isInteger(count) && count >= 0 ? count : undefined;
}

interface EvidenceTrailPanelProps {
  backendAvailable: boolean;
  workspaceRoot: string | null;
  workspaceSelected: boolean;
  refreshKey: string | number;
  onOpenSource?: (source: VerifiableSource) => void;
  onOpenArtifact?: (artifact: ArtifactRecord) => void;
  onSearchChineseEvidence?: () => void;
}

const confidenceLabel: Record<string, string> = {
  high: "高置信度",
  medium: "中等置信度",
  low: "待验证",
};

function citedPmidCount(items: Evidence[]): number {
  return new Set(
    items.flatMap((item) => `${item.claim}\n${item.detail ?? ""}`.match(/\bPMID\s*[:：]?\s*(\d{5,9})\b/gi) ?? [])
      .map((match) => match.replace(/\D/g, "")),
  ).size;
}

function evidenceProvider(item: Evidence): string {
  const text = `${item.claim}\n${item.detail ?? ""}`;
  if (/\bPMID\s*[:：]?\s*\d{5,9}\b/i.test(text)) return "PubMed";
  if (/\bDOI\s*[:：]?\s*10\.\d{4,9}\//i.test(text)) return "DOI";
  return item.source || "任务证据";
}

export function EvidenceTrailPanel({
  backendAvailable,
  workspaceRoot,
  workspaceSelected,
  refreshKey,
  onOpenSource,
  onOpenArtifact,
  onSearchChineseEvidence,
}: EvidenceTrailPanelProps) {
  const [evidence, setEvidence] = useState<Evidence[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [exportStyle, setExportStyle] = useState("ris");
  const [reviewFlow, setReviewFlow] = useState<ReviewFlow | null>(null);
  const [reviewDraft, setReviewDraft] = useState<ReviewFlowDraft>(emptyDraft);
  const [savingFlow, setSavingFlow] = useState(false);
  const literatureCoverage = useLiteratureCoverage(backendAvailable, workspaceRoot, undefined, refreshKey);

  const refresh = useCallback(async () => {
    if (!backendAvailable || !workspaceSelected) {
      setEvidence([]);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const [items, flow] = await Promise.all([
        invoke<Evidence[]>("get_evidence"),
        invoke<ReviewFlow>("get_review_flow"),
      ]);
      setEvidence(items);
      setReviewFlow(flow);
      setReviewDraft(draftFromFlow(flow));
    } catch (cause) {
      setError(`读取证据账本失败：${String(cause)}`);
    } finally {
      setLoading(false);
    }
  }, [backendAvailable, workspaceSelected]);

  useEffect(() => { void refresh(); }, [refresh, refreshKey]);

  const verifiedPmidCount = useMemo(() => citedPmidCount(evidence), [evidence]);
  const highConfidenceCount = evidence.filter((item) => item.confidence === "high").length;
  const evidenceNetwork = useMemo(() => {
    const providers = new Map<string, number>();
    const nodes = new Map<string, number>();
    evidence.forEach((item) => {
      const provider = evidenceProvider(item);
      providers.set(provider, (providers.get(provider) ?? 0) + 1);
      nodes.set(item.node_title, (nodes.get(item.node_title) ?? 0) + 1);
    });
    return { providers: [...providers], nodes: [...nodes] };
  }, [evidence]);

  const exportCitations = async () => {
    setExporting(true);
    setError(null);
    try {
      const artifact = await invoke<ArtifactRecord>("export_evidence_citations", { style: exportStyle });
      onOpenArtifact?.(artifact);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setExporting(false);
    }
  };

  const saveScreening = async () => {
    setSavingFlow(true);
    setError(null);
    try {
      const flow = await invoke<ReviewFlow>("save_review_flow", {
        flow: {
          duplicatesRemoved: optionalCount(reviewDraft.duplicatesRemoved),
          screened: optionalCount(reviewDraft.screened),
          excluded: optionalCount(reviewDraft.excluded),
          fullTextAssessed: optionalCount(reviewDraft.fullTextAssessed),
          included: optionalCount(reviewDraft.included),
        },
      });
      setReviewFlow(flow);
      setReviewDraft(draftFromFlow(flow));
    } catch (cause) {
      setError(String(cause));
    } finally {
      setSavingFlow(false);
    }
  };

  if (!workspaceSelected) {
    return (
      <section className="evidence-trail evidence-trail-empty">
        <span className="plan-canvas-kicker">EVIDENCE PROVENANCE</span>
        <h3>选择工作区后查看证据脉络</h3>
        <p>检索和讨论可以先开始；证据账本、引用导出与可追溯成果会保存在对应研究任务中。</p>
      </section>
    );
  }

  return (
    <section className="evidence-trail" aria-label="证据脉络与引用导出">
      <header className="evidence-trail__header">
        <div>
          <span className="plan-canvas-kicker">EVIDENCE PROVENANCE</span>
          <h2>证据脉络</h2>
          <p>每条结论回到任务节点、证据账本和原始文献链接。</p>
        </div>
        <div className="evidence-trail__header-actions">
          <button type="button" className="btn btn-ghost btn-sm" onClick={onSearchChineseEvidence} disabled={!workspaceSelected}>检索中文证据</button>
          <button type="button" className="btn btn-ghost btn-sm" onClick={() => void refresh()} disabled={loading}>
            {loading ? "更新中…" : "更新证据"}
          </button>
        </div>
      </header>

      <div className="evidence-trail__stats">
        <span><strong>{evidence.length}</strong> 条回流结论</span>
        <span><strong>{verifiedPmidCount}</strong> 个 PMID 来源</span>
        <span><strong>{highConfidenceCount}</strong> 条高置信度</span>
      </div>

      <EvidenceCoverageCard {...literatureCoverage} />

      <div className="evidence-trail__export">
        <div>
          <strong>导出已核验引用</strong>
          <small>仅导出证据账本中明确标注 PMID，且由 PubMed 回查到的题录。</small>
        </div>
        <select value={exportStyle} onChange={(event) => setExportStyle(event.target.value)} aria-label="引用格式">
          <option value="ris">RIS（EndNote / Zotero）</option>
          <option value="bibtex">BibTeX（LaTex）</option>
          <option value="vancouver">Vancouver 文本</option>
        </select>
        <button type="button" className="btn btn-primary btn-sm" onClick={() => void exportCitations()} disabled={exporting || verifiedPmidCount === 0}>
          {exporting ? "正在回查…" : "导出引用"}
        </button>
      </div>

      {error && <div className="evidence-trail__error">{error}</div>}
      {evidence.length > 0 && (
        <section className="evidence-network" aria-label="证据来源关系图">
          <header>
            <span className="plan-canvas-kicker">EVIDENCE RELATION MAP</span>
            <h3>来源如何进入研究结论</h3>
          </header>
          <div className="evidence-network__graph">
            <div className="evidence-network__column">
              <span>来源</span>
              {evidenceNetwork.providers.map(([label, count]) => <div key={label} className="evidence-network__node provider"><strong>{label}</strong><small>{count} 条证据</small></div>)}
            </div>
            <div className="evidence-network__links" aria-hidden="true"><i /><i /><i /></div>
            <div className="evidence-network__column">
              <span>研究节点</span>
              {evidenceNetwork.nodes.map(([label, count]) => <div key={label} className="evidence-network__node"><strong>{label}</strong><small>{count} 条回流结论</small></div>)}
            </div>
            <div className="evidence-network__links terminal" aria-hidden="true"><i /><i /></div>
            <div className="evidence-network__outcome"><span>当前可追溯结论</span><strong>{evidence.length}</strong><small>条</small></div>
          </div>
        </section>
      )}
      {reviewFlow && (
        <section className="prisma-flow" aria-label="PRISMA 文献筛选流程">
          <header>
            <div>
              <span className="plan-canvas-kicker">SYSTEMATIC REVIEW FLOW</span>
              <h3>PRISMA 筛选流程</h3>
              <p>“检出”由本任务检索账本自动汇总；其余阶段由研究者完成筛选后录入。</p>
            </div>
            <button type="button" className="btn btn-primary btn-sm" onClick={() => void saveScreening()} disabled={savingFlow || !reviewFlow.taskId}>
              {savingFlow ? "正在保存…" : "保存筛选状态"}
            </button>
          </header>
          <div className="prisma-flow__diagram">
            <PrismaStage label="数据库检出" value={reviewFlow.identified} note="自动汇总" tone="active" />
            <PrismaArrow />
            <PrismaStage label="去重后初筛" value={reviewDraft.screened} note="录入初筛记录数" editable onChange={(value) => setReviewDraft((draft) => ({ ...draft, screened: value }))} />
            <PrismaArrow />
            <PrismaStage label="全文评估" value={reviewDraft.fullTextAssessed} note="录入全文评估数" editable onChange={(value) => setReviewDraft((draft) => ({ ...draft, fullTextAssessed: value }))} />
            <PrismaArrow />
            <PrismaStage label="最终纳入" value={reviewDraft.included} note="录入研究数" editable onChange={(value) => setReviewDraft((draft) => ({ ...draft, included: value }))} tone="ready" />
          </div>
          <div className="prisma-flow__side-fields">
            <label>去重移除<input type="number" min="0" value={reviewDraft.duplicatesRemoved} onChange={(event) => setReviewDraft((draft) => ({ ...draft, duplicatesRemoved: event.target.value }))} placeholder="待录入" /></label>
            <label>初筛排除<input type="number" min="0" value={reviewDraft.excluded} onChange={(event) => setReviewDraft((draft) => ({ ...draft, excluded: event.target.value }))} placeholder="待录入" /></label>
            <span>数据校验会阻止阶段数量超出上一步。</span>
          </div>
        </section>
      )}
      {!loading && evidence.length === 0 && !error && (
        <div className="evidence-trail__empty">尚无已回流证据。完成检索、分析或写作节点后，结论会在这里形成可追溯记录。</div>
      )}

      <ol className="evidence-trail__list">
        {evidence.map((item, index) => (
          <li className="evidence-trail__item" key={item.id}>
            <div className="evidence-trail__rail" aria-hidden="true"><span>{String(index + 1).padStart(2, "0")}</span></div>
            <article>
              <div className="evidence-trail__item-header">
                <span className="evidence-trail__node">{item.node_title}</span>
                <span className={`evidence-confidence evidence-confidence-${item.confidence}`}>{confidenceLabel[item.confidence] ?? item.confidence}</span>
              </div>
              <div className="evidence-trail__claim"><ArtifactMarkdown onOpenSource={onOpenSource}>{item.claim}</ArtifactMarkdown></div>
              {item.detail && <div className="evidence-trail__detail"><ArtifactMarkdown onOpenSource={onOpenSource}>{item.detail}</ArtifactMarkdown></div>}
              <footer><span>{item.source}</span><span>{item.created_at}</span><span>节点 ID · {item.node_id}</span></footer>
            </article>
          </li>
        ))}
      </ol>
    </section>
  );
}

function PrismaArrow() { return <span className="prisma-flow__arrow" aria-hidden="true">→</span>; }

function PrismaStage({ label, value, note, editable, onChange, tone = "" }: { label: string; value: number | string; note: string; editable?: boolean; onChange?: (value: string) => void; tone?: string }) {
  return (
    <div className={`prisma-flow__stage ${tone}`}>
      <span>{label}</span>
      {editable ? <input type="number" min="0" value={value} onChange={(event) => onChange?.(event.target.value)} placeholder="—" aria-label={label} /> : <strong>{value}</strong>}
      <small>{note}</small>
    </div>
  );
}
