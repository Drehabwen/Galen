import { useState } from "react";
import type { AgentBenchmarkReport, RehabCaseBundle, RehabCaseSummary, RehabGoldenEvalReport } from "../domain/rehabContext";

interface RehabContextPanelProps {
  workspaceSelected: boolean;
  cases: RehabCaseSummary[];
  activeCase: RehabCaseBundle | null;
  loading: boolean;
  error: string | null;
  evalReport: RehabGoldenEvalReport | null;
  agentBenchmark: AgentBenchmarkReport | null;
  onOpenCase: (caseId: string) => void;
  onImportCase: (sourcePath: string, caseId: string) => void;
  onResolveReview: (decisionId: string, optionId: string) => void;
  onRunGoldenJourneys: (sourcePath: string) => void;
}

const contextLabel: Record<string, string> = {
  natural_standing: "自然站立",
  in_brace: "支具内",
  immediate_out_of_brace: "刚脱支具",
  out_of_brace_timed: "定时脱支具",
  surface_assessment: "体表评估",
  unknown: "待确认",
};

export function RehabContextPanel(props: RehabContextPanelProps) {
  const [sourcePath, setSourcePath] = useState("evals/case-datasets/ais-textbook-pilot-v1/cases.json");
  const [caseId, setCaseId] = useState("AIS-C025");
  const bundle = props.activeCase;

  if (!props.workspaceSelected) {
    return <div className="rehab-empty"><h2>病例纵向证据</h2><p>先选择工作区，再导入去标识化病例。</p></div>;
  }

  return (
    <main className="rehab-context">
      <header className="rehab-header">
        <div>
          <span className="rehab-kicker">REHABILITATION CONTEXT</span>
          <h1>{bundle ? bundle.case_record.case_id : "建立第一条病例证据链"}</h1>
          <p>{bundle ? `Revision ${bundle.revision} · 主机侧权威状态` : "把来源、随访和裁决连接成可复算的科研记录。"}</p>
        </div>
        <div className="rehab-import">
          <input aria-label="病例集相对路径" value={sourcePath} onChange={(event) => setSourcePath(event.target.value)} />
          <input aria-label="病例 ID" value={caseId} onChange={(event) => setCaseId(event.target.value)} />
          <button className="btn btn-primary" disabled={props.loading} onClick={() => props.onImportCase(sourcePath, caseId)}>导入病例</button>
          <button className="btn btn-ghost rehab-eval-button" disabled={props.loading} onClick={() => props.onRunGoldenJourneys(sourcePath)}>运行黄金旅程</button>
        </div>
      </header>

      {props.error && <div className="rehab-error">{props.error}</div>}
      {props.evalReport && (
        <section className={`rehab-eval ${props.evalReport.passed ? "passed" : "failed"}`}>
          <div className="rehab-eval-heading">
            <div>
              <span className="rehab-kicker">EVALUATION LAB · {props.evalReport.suite_id}</span>
              <h2>{props.evalReport.passed ? "5 条黄金旅程全部通过" : "检测到负优化"}</h2>
            </div>
            <strong>{props.evalReport.journeys.filter((item) => item.passed).length}/{props.evalReport.journeys.length}</strong>
          </div>
          <div className="rehab-eval-metrics">
            {props.evalReport.metrics.map((metric) => (
              <div key={metric.id} className={metric.passed ? "passed" : "failed"}>
                <span>{metric.label}</span>
                <strong>{Math.round(metric.value * 100)}%</strong>
                <small>门槛 {Math.round(metric.threshold * 100)}%</small>
              </div>
            ))}
          </div>
          <div className="rehab-eval-journeys">
            {props.evalReport.journeys.map((journey) => (
              <article key={journey.journey_id}>
                <b>{journey.passed ? "✓" : "×"}</b>
                <div><strong>{journey.journey_id} · {journey.title}</strong><small>{journey.persona}</small></div>
                <code>{journey.duration_ms} ms</code>
              </article>
            ))}
          </div>
          <p className="rehab-eval-next">{props.evalReport.recommendations[0]}</p>
        </section>
      )}
      {props.agentBenchmark && props.agentBenchmark.runs.length > 0 && (
        <section className="rehab-eval passed agent-benchmark">
          <div className="rehab-eval-heading">
            <div><span className="rehab-kicker">AGENT FOUNDATION · {props.agentBenchmark.case_id} · K=5</span><h2>响应速度与可靠性交叉验证</h2></div>
          </div>
          <div className="rehab-eval-metrics">
            {props.agentBenchmark.runs.map((run) => (
              <div key={run.profile} className={run.pass_rate === 1 ? "passed" : "failed"}>
                <span>{run.profile} · {run.model}</span>
                <strong>{run.mean_total_ms} ms</strong>
                <small>TTFR {run.mean_ttfr_ms} ms · P95 {run.p95_total_ms} ms · {Math.round(run.pass_rate * 100)}%</small>
              </div>
            ))}
          </div>
        </section>
      )}
      {props.cases.length > 1 && (
        <div className="rehab-case-tabs">
          {props.cases.map((item) => <button key={item.case_id} className={bundle?.case_record.case_id === item.case_id ? "active" : ""} onClick={() => props.onOpenCase(item.case_id)}>{item.case_id}</button>)}
        </div>
      )}

      {!bundle ? <div className="rehab-empty"><p>尚无病例。可先导入工作区内的 AIS-C025 金标准病例。</p></div> : (
        <>
          <section className="rehab-strip" aria-label="病例状态">
            <div><span>队列状态</span><strong className={`rehab-status ${bundle.cohort_row.status}`}>{bundle.cohort_row.status === "included" ? "可纳入" : "待复核"}</strong></div>
            <div><span>来源覆盖</span><strong>{Math.round(bundle.cohort_row.source_coverage * 100)}%</strong></div>
            <div><span>开放裁决</span><strong>{bundle.cohort_row.open_review_count}</strong></div>
            <div><span>核验观察</span><strong>{bundle.observations.filter((item) => item.verification_status === "verified").length}/{bundle.observations.length}</strong></div>
          </section>

          <section className="rehab-section">
            <div className="rehab-section-title"><h2>纵向事件脉络</h2><span>采集状态不可混用</span></div>
            <div className="rehab-timeline">
              {bundle.events.map((event) => (
                <article key={event.event_id} className="rehab-event">
                  <i />
                  <time>{event.occurred_at}</time>
                  <strong>{event.event_type === "baseline" ? "基线" : event.event_type === "follow_up" ? "随访" : "干预"}</strong>
                  <span>{contextLabel[event.collection_context] ?? event.collection_context}</span>
                </article>
              ))}
            </div>
          </section>

          <div className="rehab-grid">
            <section className="rehab-section">
              <div className="rehab-section-title"><h2>观察值与来源</h2><span>仅已核验值进入硬门</span></div>
              <div className="rehab-observations">
                {bundle.observations.map((item) => (
                  <div className="rehab-observation" key={item.observation_id}>
                    <div><strong>{item.region} · {item.metric}</strong><small>{item.event_id} / {contextLabel[item.collection_context]}</small></div>
                    <b>{item.value ?? "—"}<small>{item.unit}</small></b>
                    <span className={`rehab-verification ${item.verification_status}`}>{item.verification_status === "verified" ? "已核验" : "有争议"}</span>
                    <code>p.{item.source_locator.pdf_page ?? "?"} · {item.source_locator.channel}</code>
                  </div>
                ))}
              </div>
            </section>

            <aside className="rehab-side">
              {bundle.review_decisions.filter((item) => item.status === "open").map((decision) => (
                <section className="rehab-review" key={decision.decision_id}>
                  <span className="rehab-review-flag">需要人类裁决</span>
                  <h2>{decision.question}</h2>
                  <p>系统保留所有来源，不替你选择。</p>
                  {decision.options.map((option) => <button key={option.option_id} disabled={props.loading} onClick={() => props.onResolveReview(decision.decision_id, option.option_id)}><strong>{option.value}</strong> {option.channel}</button>)}
                </section>
              ))}
              <section className="rehab-output">
                <span>可复算队列行</span>
                {Object.entries(bundle.cohort_row.derived_values).map(([key, value]) => <div key={key}><code>{key}</code><strong>{value}</strong></div>)}
              </section>
            </aside>
          </div>
        </>
      )}
    </main>
  );
}
