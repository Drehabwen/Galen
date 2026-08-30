import { FormEvent, useState } from "react";
import type { FileEntry } from "../types";

interface ResearchWorkbenchProps {
  wsRoot: string | null;
  files: FileEntry[];
  currentFile: { path: string; content: string } | null;
  backendAvailable: boolean;
  reportAvailable?: boolean;
  onOpenReport?: () => void;
  onAgentPrompt: (prompt: string) => void;
  onReadFile: (path: string) => void;
}

const STAGES = ["数据审查", "假设构建", "模型比较", "误差解释", "结果交付"];
const METRICS = [
  ["非线性时间基线", "1.71", "teal"],
  ["仅肌电信号", "2.92", "violet"],
  ["时间 + 全部传感器", "1.79", "blue"],
] as const;
const TRACE_PATHS = [
  ["M12 92 L28 72 L44 80 L60 60 L76 69 L92 49 L108 63 L124 52 L140 74 L156 43 L172 58 L188 70 L204 54 L220 66 L236 45 L252 62 L268 50 L284 77 L300 58 L316 68 L332 54 L348 73 L364 60 L380 79 L396 58 L412 73 L428 48 L444 84 L460 66 L476 35", "violet"],
  ["M12 110 L28 96 L44 103 L60 91 L76 102 L92 88 L108 99 L124 93 L140 107 L156 86 L172 96 L188 109 L204 92 L220 101 L236 87 L252 106 L268 94 L284 112 L300 96 L316 103 L332 92 L348 108 L364 96 L380 111 L396 98 L412 105 L428 95 L444 112 L460 102 L476 91", "slate"],
  ["M12 124 L28 109 L44 117 L60 105 L76 114 L92 103 L108 112 L124 108 L140 120 L156 99 L172 110 L188 121 L204 106 L220 114 L236 102 L252 118 L268 109 L284 125 L300 111 L316 117 L332 106 L348 121 L364 111 L380 124 L396 113 L412 119 L428 110 L444 126 L460 116 L476 106", "teal"],
  ["M12 132 L28 119 L44 126 L60 114 L76 122 L92 111 L108 121 L124 116 L140 129 L156 107 L172 118 L188 130 L204 114 L220 123 L236 110 L252 127 L268 117 L284 134 L300 120 L316 126 L332 115 L348 130 L364 120 L380 133 L396 122 L412 128 L428 118 L444 136 L460 126 L476 116", "blue"],
] as const;

export function ResearchWorkbench({ wsRoot, backendAvailable, reportAvailable = false, onOpenReport, onAgentPrompt }: ResearchWorkbenchProps) {
  const [decision, setDecision] = useState("保留并分层分析");
  const [command, setCommand] = useState("");
  const chooseDecision = (next: string) => {
    setDecision(next);
    onAgentPrompt(`针对受试者 27，采用“${next}”并更新分析方案。`);
  };
  const submitCommand = (event: FormEvent) => {
    event.preventDefault();
    const next = command.trim();
    if (!next) return;
    onAgentPrompt(next);
    setCommand("");
  };

  return (
    <div className="insight-workbench">
      <main className="insight-canvas">
        <header className="insight-hero">
          <div>
            <span className="insight-eyebrow">当前研究 · {wsRoot?.split(/[/\\]/).pop() ?? "未选择工作区"}</span>
            <h1>运动疲劳中的时间捷径与跨受试者泛化</h1>
            <p>从多模态生理信号中区分真实疲劳表征与实验流程偏差</p>
          </div>
          <span className={`insight-model-state ${backendAvailable ? "online" : "offline"}`}><i />{backendAvailable ? "模型在线" : "模型离线"}</span>
        </header>

        <ol className="research-lifecycle" aria-label="研究生命周期">
          {STAGES.map((stage, index) => <li key={stage} className={index < 3 ? "done" : index === 3 ? "active" : ""}><span>{index < 3 ? "✓" : index + 1}</span><b>{stage}</b></li>)}
        </ol>

        <section className="metric-strip" aria-label="模型比较">
          {METRICS.map(([label, value, tone]) => <article className={`metric-card ${tone}`} key={label}><div className="metric-label"><i />{label}</div><div><small>MAE</small><strong>{value}</strong></div></article>)}
        </section>

        <div className="insight-analysis-grid">
          <section className="error-chart-panel">
            <div className="section-heading"><div><span>MODEL DIAGNOSTICS</span><h2>跨受试者误差分布</h2></div><button type="button" onClick={() => onAgentPrompt("解释跨受试者误差分布，并检查受试者 27。")}>解释图表 ↗</button></div>
            <svg className="error-chart" viewBox="0 0 500 176" role="img" aria-label="30 名受试者的模型误差折线图，受试者 27 被突出显示">
              {[36, 76, 116, 156].map((y) => <line key={y} x1="12" y1={y} x2="488" y2={y} className="gridline" />)}
              <line x1="444" y1="18" x2="444" y2="156" className="subject-guide" />
              {TRACE_PATHS.map(([d, tone]) => <path key={tone} d={d} className={`trace ${tone}`} />)}
              <circle cx="444" cy="84" r="6" className="subject-point" /><text x="421" y="14" className="subject-label">Subject 27</text>
              <text x="12" y="171">1</text><text x="238" y="171">15</text><text x="468" y="171">30</text>
            </svg>
            <div className="chart-legend"><span className="teal">非线性时间基线</span><span className="violet">仅肌电信号</span><span className="blue">时间 + 全部传感器</span></div>
            <div className="activity-strip"><strong>最近活动</strong><span>✓ 已完成 LOSO 验证</span><span>✓ 已生成受试者误差图</span></div>
          </section>

          <section className="finding-panel">
            <div className="finding-copy"><span className="insight-eyebrow">EVIDENCE SYNTHESIS</span><h2>关键发现</h2><p>时间变量几乎复现了完整多模态模型的性能，提示模型可能学习了实验流程，而非稳定的疲劳生理表征。</p><div className="evidence-chips"><span>✓ LOSO 验证</span><span>置换检验 p &lt; 0.01</span></div></div>
            <article className="alignment-card"><h2>受试者 27 · 证据对齐</h2><table><thead><tr><th>来源</th><th>事件</th><th>时间戳</th><th>解释</th></tr></thead><tbody><tr><td>实验记录</td><td>Borg 达到 15</td><td>122.00 s</td><td>主观疲劳上升</td></tr><tr><td>EMG / IMU</td><td>特征转折</td><td>80.91 s</td><td>生理变化提前</td></tr><tr><td>视频标注</td><td>动作代偿出现</td><td>83.40 s</td><td>与传感器一致</td></tr></tbody></table></article>
            <div className="research-decision"><h3>研究者裁决</h3><div>{["标记流程偏差", "保留并分层分析", "加入敏感性分析"].map((option) => <button key={option} type="button" aria-label={option} aria-pressed={decision === option} className={decision === option ? "selected" : ""} onClick={() => chooseDecision(option)}>{decision === option && "✓ "}{option}</button>)}</div></div>
          </section>
        </div>

        <form className="research-command" onSubmit={submitCommand}><span aria-hidden="true">✦</span><input value={command} onChange={(event) => setCommand(event.target.value)} placeholder="询问数据、运行分析或生成研究产物…" /><button type="submit" aria-label="发送研究指令">→</button></form>
      </main>

      <aside className="evidence-rail" aria-label="证据链与研究产物">
        <section><div className="rail-heading"><span>TRACEABILITY</span><h2>证据链</h2></div><ul className="evidence-list"><li><i>▤</i><span>原始数据<small>版本化输入</small></span><b>已锁定</b></li><li><i>⌘</i><span>分析脚本<small>commit</small></span><b>8f3c2a1</b></li><li><i>▥</i><span>统计结果<small>一致性检查</small></span><b>已复核</b></li><li><i>⌁</i><span>文献依据<small>相关证据</small></span><b>12 条</b></li></ul></section>
        <section><div className="rail-heading"><span>DELIVERABLES</span><h2>研究产物</h2></div><ul className="output-list">{["结果摘要", "方法记录", "图表包"].map((item) => <li key={item}><span>▧ {item}</span><b>已完成</b></li>)}<li><span>▧ 可复现报告</span><b>{reportAvailable ? "已生成" : "待生成"}</b></li></ul></section>
        <button className="open-report" type="button" disabled={!reportAvailable} aria-label={reportAvailable ? "打开可复现报告" : "尚未生成 PDF 报告"} onClick={onOpenReport}>{reportAvailable ? "打开可复现报告" : "尚未生成 PDF 报告"} <span>{reportAvailable ? "↗" : "—"}</span></button>
      </aside>
    </div>
  );
}
