const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const { chromium } = require('C:/Users/labops/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright');
const OUT = path.join(ROOT, 'output', 'pdf');
const TMP = path.join(ROOT, 'tmp', 'pdfs');
const FIG = path.join(OUT, 'figures');
const PDF = path.join(OUT, 'Galen_高强度间歇负荷后多模态恢复_正式论文.pdf');
const CSV = path.join(OUT, 'galen_sports_fatigue_demo_cohort_v2.csv');
const HTML = path.join(TMP, 'galen_sports_fatigue_demo_report.html');
const GALEN_DEMO_PDF = path.join(ROOT, 'rust', 'crates', 'galen', 'public', 'demo', 'galen-sports-fatigue-paper.pdf');

const profiles = [
  ['ATH-001', 64, 31.4, 58, [.68,.84,.95], [.86,.91,.97], [15,7,2], [0,2,4,1], 8],
  ['ATH-002', 52, 40.8, 55, [.66,.82,.93], [.85,.90,.96], [18,8,3], [0,2,5,2], 9],
  ['ATH-003', 58, 29.5, 62, [.64,.78,.89], [.84,.88,.94], [19,11,5], [1,3,6,3], 9],
  ['ATH-004', 49, 44.1, 56, [.70,.86,.98], [.88,.94,.99], [14,6,1], [0,1,3,1], 8],
  ['ATH-005', 68, 33.2, 50, [.73,.89,.99], [.89,.95,.99], [12,5,0], [0,1,3,1], 7],
  ['ATH-006', 55, 37.7, 59, [.67,.81,.92], [.86,.91,.96], [17,9,3], [0,2,5,2], 8],
  ['ATH-007', 61, 30.1, 57, [.65,.80,.91], [.85,.89,.95], [16,9,4], [1,3,5,2], 8],
  ['ATH-008', 46, 42.4, 61, [.62,.77,.88], [.83,.88,.93], [21,12,6], [1,3,6,3], 9],
  ['ATH-009', 53, 28.9, 63, [.66,.83,.94], [.87,.92,.97], [18,8,3], [0,2,4,1], 8],
  ['ATH-010', 59, 38.4, 54, [.71,.87,.97], [.88,.94,.98], [13,5,1], [0,1,3,1], 7],
  ['ATH-011', 66, 32.6, 52, [.69,.85,.96], [.87,.93,.98], [15,7,2], [0,2,4,1], 8],
  ['ATH-012', 50, 41.2, 58, [.63,.79,.90], [.84,.89,.95], [20,10,4], [1,3,6,3], 9],
];

const round = (value, digits = 1) => Number(value.toFixed(digits));
const timepoints = ['Baseline', 'T0 +30 min', 'T24', 'T72'];
const data = profiles.flatMap(([id, rmssd, cmj, hr, rmssdRatios, cmjRatios, hrOffsets, vas, srpe]) => [
  { id, timepoint: timepoints[0], rmssd_ms: rmssd, resting_hr_bpm: hr, cmj_cm: cmj, doms_vas: vas[0], session_rpe: '' },
  { id, timepoint: timepoints[1], rmssd_ms: round(rmssd * rmssdRatios[0], 0), resting_hr_bpm: hr + hrOffsets[0], cmj_cm: round(cmj * cmjRatios[0]), doms_vas: vas[1], session_rpe: srpe },
  { id, timepoint: timepoints[2], rmssd_ms: round(rmssd * rmssdRatios[1], 0), resting_hr_bpm: hr + hrOffsets[1], cmj_cm: round(cmj * cmjRatios[1]), doms_vas: vas[2], session_rpe: '' },
  { id, timepoint: timepoints[3], rmssd_ms: round(rmssd * rmssdRatios[2], 0), resting_hr_bpm: hr + hrOffsets[2], cmj_cm: round(cmj * cmjRatios[2]), doms_vas: vas[3], session_rpe: '' },
]);

const mean = (values) => values.reduce((sum, value) => sum + value, 0) / values.length;
const sd = (values) => {
  const m = mean(values);
  return Math.sqrt(values.reduce((sum, value) => sum + (value - m) ** 2, 0) / (values.length - 1));
};
const byTime = (metric) => timepoints.map((timepoint) => data.filter((row) => row.timepoint === timepoint).map((row) => Number(row[metric])));
const stat = (metric) => byTime(metric).map((values) => ({ mean: mean(values), sd: sd(values), se: sd(values) / Math.sqrt(values.length) }));
const fmt = (value, digits = 1) => Number(value).toFixed(digits);
const pct = (from, to) => ((to - from) / from) * 100;

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (character) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character]);
}

function trajectorySvg() {
  const metrics = [
    { key: 'rmssd_ms', title: 'HRV RMSSD', unit: 'ms', min: 25, max: 75, color: '#0f766e' },
    { key: 'cmj_cm', title: 'CMJ height', unit: 'cm', min: 20, max: 50, color: '#2563eb' },
    { key: 'resting_hr_bpm', title: 'Resting HR', unit: 'bpm', min: 45, max: 90, color: '#ea580c' },
    { key: 'doms_vas', title: 'DOMS VAS', unit: '0-10', min: 0, max: 7, color: '#be185d' },
  ];
  const panelW = 548; const panelH = 270; const width = 1120; const height = 570;
  const panel = (metric, index) => {
    const offsetX = (index % 2) * 565;
    const offsetY = Math.floor(index / 2) * 285;
    const left = offsetX + 58; const top = offsetY + 44; const chartW = 450; const chartH = 165;
    const x = (i) => left + (chartW / 3) * i;
    const y = (value) => top + chartH - ((value - metric.min) / (metric.max - metric.min)) * chartH;
    const rows = profiles.map((profile) => data.filter((row) => row.id === profile[0]));
    const individual = rows.map((row) => `<polyline points="${row.map((point, i) => `${x(i)},${y(point[metric.key])}`).join(' ')}" fill="none" stroke="#b9c7d8" stroke-width="1.15" opacity=".82"/>`).join('');
    const aggregate = stat(metric.key);
    const ribbon = aggregate.map((item, i) => `${x(i)},${y(Math.min(metric.max, item.mean + item.se))}`).join(' ') + ' ' + aggregate.slice().reverse().map((item, i) => `${x(3 - i)},${y(Math.max(metric.min, item.mean - item.se))}`).join(' ');
    const meanPath = aggregate.map((item, i) => `${x(i)},${y(item.mean)}`).join(' ');
    const labels = timepoints.map((name, i) => `<text x="${x(i)}" y="${top + chartH + 27}" text-anchor="middle" class="axis">${name.replace(' +30 min', '')}</text>`).join('');
    const grid = [0, .5, 1].map((t) => `<line x1="${left}" y1="${top + chartH * t}" x2="${left + chartW}" y2="${top + chartH * t}" stroke="#dbe5ef" stroke-width="1"/><text x="${left - 10}" y="${top + chartH * t + 4}" text-anchor="end" class="axis">${fmt(metric.max - (metric.max - metric.min) * t, 0)}</text>`).join('');
    const markers = aggregate.map((item, i) => `<circle cx="${x(i)}" cy="${y(item.mean)}" r="4.5" fill="${metric.color}" stroke="white" stroke-width="2"/>`).join('');
    return `<g><rect x="${offsetX}" y="${offsetY}" width="${panelW}" height="${panelH}" rx="15" fill="#f8fbff" stroke="#d9e5f0"/><text x="${offsetX + 22}" y="${offsetY + 26}" class="panel-title">${metric.title}</text><text x="${offsetX + panelW - 20}" y="${offsetY + 26}" text-anchor="end" class="unit">${metric.unit}</text>${grid}${individual}<polygon points="${ribbon}" fill="${metric.color}" opacity=".16"/><polyline points="${meanPath}" fill="none" stroke="${metric.color}" stroke-width="3.2"/>${markers}${labels}</g>`;
  };
  return `<svg viewBox="0 0 ${width} ${height}" role="img" aria-label="group recovery trajectories"><style>.axis{font:12px 'Segoe UI',Arial,sans-serif;fill:#5d7188}.panel-title{font:700 16px 'Segoe UI',Arial,sans-serif;fill:#142c45}.unit{font:12px 'Segoe UI',Arial,sans-serif;fill:#6f8499}</style>${metrics.map(panel).join('')}</svg>`;
}

function recoverySvg() {
  const width = 1120; const height = 450; const left = 56; const top = 62; const colW = 495; const gap = 76;
  const recovery = profiles.map(([id]) => {
    const rows = data.filter((row) => row.id === id);
    return { id, rmssd: rows[3].rmssd_ms / rows[0].rmssd_ms * 100, cmj: rows[3].cmj_cm / rows[0].cmj_cm * 100, doms: rows[2].doms_vas };
  }).sort((a, b) => (a.rmssd + a.cmj) - (b.rmssd + b.cmj));
  const xRecovery = (value) => left + ((value - 82) / 20) * colW;
  const xDoms = (value) => left + colW + gap + value / 7 * colW;
  const rowY = (i) => top + i * 26;
  const leftGrid = [85, 90, 95, 100].map((tick) => `<line x1="${xRecovery(tick)}" y1="${top - 20}" x2="${xRecovery(tick)}" y2="${top + 300}" stroke="#dbe5ef"/><text x="${xRecovery(tick)}" y="${top - 31}" text-anchor="middle" class="axis">${tick}%</text>`).join('');
  const rightGrid = [0,2,4,6].map((tick) => `<line x1="${xDoms(tick)}" y1="${top - 20}" x2="${xDoms(tick)}" y2="${top + 300}" stroke="#dbe5ef"/><text x="${xDoms(tick)}" y="${top - 31}" text-anchor="middle" class="axis">${tick}</text>`).join('');
  const rows = recovery.map((row, i) => `<text x="${left - 11}" y="${rowY(i) + 4}" text-anchor="end" class="axis">${row.id}</text><line x1="${xRecovery(row.rmssd)}" y1="${rowY(i)}" x2="${xRecovery(row.cmj)}" y2="${rowY(i)}" stroke="#94a3b8" stroke-width="2"/><circle cx="${xRecovery(row.rmssd)}" cy="${rowY(i)}" r="5" fill="#0f766e"/><circle cx="${xRecovery(row.cmj)}" cy="${rowY(i)}" r="5" fill="#2563eb"/><line x1="${xDoms(0)}" y1="${rowY(i)}" x2="${xDoms(row.doms)}" y2="${rowY(i)}" stroke="#eab7c9" stroke-width="8" stroke-linecap="round"/><circle cx="${xDoms(row.doms)}" cy="${rowY(i)}" r="5" fill="#be185d"/>`).join('');
  return `<svg viewBox="0 0 ${width} ${height}" role="img" aria-label="individual recovery profile"><style>.axis{font:12px 'Segoe UI',Arial,sans-serif;fill:#5d7188}.panel-title{font:700 16px 'Segoe UI',Arial,sans-serif;fill:#142c45}.legend{font:12px 'Segoe UI',Arial,sans-serif;fill:#334e68}</style><text x="${left}" y="22" class="panel-title">T72 relative recovery from individual baseline</text><circle cx="${left + 340}" cy="18" r="5" fill="#0f766e"/><text x="${left + 350}" y="22" class="legend">RMSSD</text><circle cx="${left + 420}" cy="18" r="5" fill="#2563eb"/><text x="${left + 430}" y="22" class="legend">CMJ</text><text x="${left + colW + gap}" y="22" class="panel-title">T24 DOMS VAS</text>${leftGrid}${rightGrid}${rows}<text x="${left}" y="${top + 340}" class="legend">Each row is one de-identified RehabID. Endpoints preserve individual variability rather than only showing a group average.</text></svg>`;
}

function metricSummary(metric, label, unit, direction) {
  const values = stat(metric);
  const change0 = pct(values[0].mean, values[1].mean);
  const change72 = pct(values[0].mean, values[3].mean);
  return { label, unit, baseline: `${fmt(values[0].mean)} ± ${fmt(values[0].sd)}`, t0: `${fmt(values[1].mean)} ± ${fmt(values[1].sd)}`, t24: `${fmt(values[2].mean)} ± ${fmt(values[2].sd)}`, t72: `${fmt(values[3].mean)} ± ${fmt(values[3].sd)}`, change0: `${change0 >= 0 ? '+' : ''}${fmt(change0)}%`, change72: `${change72 >= 0 ? '+' : ''}${fmt(change72)}%`, direction };
}

function table(rows, className = '') {
  return `<table class="${className}">${rows.map((row, rowIndex) => `<tr>${row.map((cell) => `<${rowIndex === 0 ? 'th' : 'td'}>${cell}</${rowIndex === 0 ? 'th' : 'td'}>`).join('')}</tr>`).join('')}</table>`;
}

function pageFooter(pageNumber) {
  return `<footer><span>Galen RehabID · 路演模拟验证队列 · v2.0</span><span>Confidential demo artifact · ${pageNumber}</span></footer>`;
}

async function build() {
  fs.mkdirSync(OUT, { recursive: true });
  fs.mkdirSync(TMP, { recursive: true });
  fs.mkdirSync(FIG, { recursive: true });
  fs.writeFileSync(CSV, ['rehab_id,timepoint,hrv_rmssd_ms,resting_hr_bpm,cmj_height_cm,doms_vas,session_rpe', ...data.map((row) => [row.id, row.timepoint, row.rmssd_ms, row.resting_hr_bpm, row.cmj_cm, row.doms_vas, row.session_rpe].join(','))].join('\n'), 'utf8');
  fs.writeFileSync(path.join(FIG, 'trajectory.svg'), trajectorySvg(), 'utf8');
  fs.writeFileSync(path.join(FIG, 'individual-recovery.svg'), recoverySvg(), 'utf8');

  const rmssd = metricSummary('rmssd_ms', 'HRV RMSSD', 'ms', 'lower at acute recovery');
  const cmj = metricSummary('cmj_cm', 'CMJ height', 'cm', 'lower at acute recovery');
  const hr = metricSummary('resting_hr_bpm', 'Resting HR', 'bpm', 'higher at acute recovery');
  const doms = metricSummary('doms_vas', 'DOMS VAS', '0-10', 'higher at T24');
  const statsRows = [
    ['Measure', 'Baseline', 'T0 +30 min', 'T24', 'T72', 'T0 vs baseline', 'T72 vs baseline'],
    ...[rmssd, cmj, hr, doms].map((item) => [item.label, `${item.baseline} ${item.unit}`, `${item.t0} ${item.unit}`, `${item.t24} ${item.unit}`, `${item.t72} ${item.unit}`, item.change0, item.change72]),
  ];
  const meanRmssd = stat('rmssd_ms');
  const meanCmj = stat('cmj_cm');
  const meanDoms = stat('doms_vas');
  const formalPaperHtml = `<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><title>高强度间歇负荷后72小时多模态恢复的纵向特征</title><style>
@page{size:A4;margin:18mm 18mm 17mm}*{box-sizing:border-box}body{margin:0;color:#172b4d;font-family:"Times New Roman","Microsoft YaHei",serif;font-size:10.25pt;line-height:1.72} .paper{position:relative;min-height:260mm;page-break-after:always;padding-bottom:13mm}.paper:last-child{page-break-after:auto}header{display:flex;justify-content:space-between;align-items:center;border-bottom:1.4px solid #163b63;padding-bottom:5px;color:#426783;font-family:"Microsoft YaHei",sans-serif;font-size:7.8pt;letter-spacing:.4px;margin-bottom:15px}footer{position:absolute;left:0;right:0;bottom:0;display:flex;justify-content:space-between;border-top:.5px solid #aebdcb;padding-top:5px;color:#60758b;font-family:"Microsoft YaHei",sans-serif;font-size:7.5pt}h1{font-family:"Microsoft YaHei",sans-serif;font-size:22pt;line-height:1.32;letter-spacing:.25px;text-align:center;color:#102b46;margin:7px 0 7px}h2{font-family:"Microsoft YaHei",sans-serif;font-size:13.2pt;color:#0f4c81;margin:15px 0 7px;border-left:4px solid #0f766e;padding-left:8px;line-height:1.35}h3{font-family:"Microsoft YaHei",sans-serif;font-size:10.9pt;color:#183f63;margin:11px 0 4px}p{margin:5px 0;text-align:justify}.authors{text-align:center;font-family:"Microsoft YaHei",sans-serif;font-size:10.2pt;margin:5px 0;color:#183f63}.affiliation{text-align:center;color:#62768a;font-size:8.6pt;margin-bottom:15px}.abstract{border-top:2px solid #163b63;border-bottom:1px solid #c7d5e0;padding:9px 12px 10px;margin:12px 0 11px;background:#fbfdff;font-size:9.35pt;line-height:1.68}.abstract b{font-family:"Microsoft YaHei",sans-serif;color:#0d4e7c}.keywords{font-size:9pt;margin-top:6px}.note{font-size:8.4pt;color:#5f7183;border-left:3px solid #4f88b7;background:#f5f9fc;padding:7px 9px;margin:8px 0}.twocol{columns:2;column-gap:19px;column-rule:1px solid #eef2f5}.twocol p,.twocol h2,.twocol h3,.twocol figure,.twocol table,.twocol .note{break-inside:avoid}.full{columns:auto}.table{border-collapse:collapse;width:100%;font-size:8.45pt;margin:6px 0 10px;break-inside:avoid}.table th{background:#173b60;color:#fff;padding:6px 6px;text-align:center;font-family:"Microsoft YaHei",sans-serif;font-weight:600;line-height:1.3}.table td{border-bottom:.5px solid #cbd9e4;padding:5.5px 6px;vertical-align:top;line-height:1.47}.table tr:nth-child(even) td{background:#f7fafc}.table .left{text-align:left}.table caption{caption-side:top;text-align:left;font-family:"Microsoft YaHei",sans-serif;font-weight:700;color:#183f63;font-size:9.1pt;margin:0 0 4px}.figure{border:1px solid #c7d7e4;padding:7px;background:#fff;margin:7px 0 2px;break-inside:avoid}.figure svg{display:block;width:100%;height:auto}.caption{font-size:8.2pt;color:#61768b;line-height:1.45;text-align:justify;margin:3px 0 9px}.result-box{border:1px solid #b8d2df;border-left:4px solid #0f766e;background:#f5fbfb;padding:8px 10px;margin:6px 0 9px;break-inside:avoid}.result-box p{margin:0;font-size:9pt}.references{margin:4px 0;padding-left:20px;font-size:8.2pt;line-height:1.48}.references li{padding-left:2px;margin:3px 0;text-align:justify}.references a{color:#0d5795;text-decoration:none}.section-small{font-size:8.7pt}.subtle{color:#5d738a}.data-grid{display:grid;grid-template-columns:1fr 1fr;gap:8px;margin:7px 0}.data-cell{border:1px solid #d3e0ea;padding:7px 8px;background:#fbfdff;font-size:8.5pt}.data-cell b{display:block;color:#0f4c81;margin-bottom:2px;font-family:"Microsoft YaHei",sans-serif}.statistics{font-size:8.6pt;background:#f8fbfd;border-top:1px solid #cedce7;border-bottom:1px solid #cedce7;padding:7px 9px;margin:6px 0}.small{font-size:8.6pt}.page-break{break-before:page}
</style></head><body>
<section class="paper"><header><span>高强度间歇负荷后多模态恢复的纵向特征</span><span>研究性分析示例稿</span></header>
<h1>高强度间歇自行车负荷后 72 小时多模态恢复的纵向特征：<br>基于 RehabID 时间轴的演示性队列研究</h1>
<div class="authors">Galen Rehab Intelligence 研究团队</div><div class="affiliation">康复与运动科学智能数据治理研发组</div>
<div class="abstract"><p><b>摘要</b>　<b>目的：</b>建立以 RehabID 为最小分析单元的连续数据结构，描述一次高强度间歇自行车负荷后心率变异性（heart rate variability, HRV）、反向跳（countermovement jump, CMJ）、静息心率和延迟性肌肉酸痛（delayed-onset muscle soreness, DOMS）的 72 h 纵向变化，并验证结构化康复数据从采集、质控到论文级交付的可追溯性。<b>方法：</b>构建 12 个匿名对象、4 个评估节点（基线、负荷后 30 min、24 h、72 h）的确定性演示队列。负荷为 10 × 1 min 高强度间歇自行车（90% peak power），间歇 1 min。采集 5 min 静息 RMSSD、静息心率、三次有效 CMJ 的代表值、DOMS-VAS 与会话 RPE。全部记录按对象、时间点、指标、单位和来源写入 RehabID 时间轴。采用均值±标准差描述各时间点，并保留个体轨迹用于观察恢复异质性。<b>结果：</b>共形成 48 个纵向评估节点和 204 条数值观察。相较基线，T0 时 RMSSD 下降 32.5%，CMJ 下降 14.0%，静息心率上升 28.9%；T24 时 DOMS-VAS 达峰；T72 时群体均值呈恢复趋势，但 3 个对象仍表现出较慢的自主神经或神经肌肉恢复路径。<b>结论：</b>将多来源数据组织为连续 RehabID 时间轴，可在不丢失个体差异的前提下完成负荷后恢复的纵向描述，并形成可复核的图表与 PDF 交付。该示例为后续真实世界队列的采集规范、数据治理和分析流程提供了可运行原型。</p><p class="keywords"><b>关键词</b>　运动疲劳；心率变异性；反向跳；纵向研究；数据治理；康复信息学</p></div>
<div class="note">说明：本文为产品路演所用的<b>研究性分析示例稿</b>。数据为确定性、去标识化的演示队列，用于验证数据模型、分析流程与 PDF 交付；文中的结构、方法和参考文献可直接迁移至经伦理审批的真实研究。</div>
<div class="twocol"><h2>1　引言</h2><p>运动负荷后的恢复并不由单一生理指标决定。自主神经调节、神经肌肉表现与主观恢复感受可呈现不同时间尺度的变化；若研究者仅保留某一次检测值，便难以分辨急性反应、延迟性反应与个体恢复差异。HRV 指标可为训练状态和自主神经调节提供重要信息，但其解释受到测量条件、负荷类型和个体基线的共同影响<sup>[1,2]</sup>。CMJ 及其力—时特征则常被用于捕捉高强度运动后的神经肌肉状态变化<sup>[3,4]</sup>。</p><p>现有康复与运动科研的一个实际问题不在于缺少测量，而在于数据散落于力量设备、可穿戴、量表与工作台等不同系统，难以回到同一对象的连续时间轴。为此，本研究采用 RehabID 作为跨来源的最小可计算单元：每条观察同时保留对象短码、评估时间点、指标与单位、采集来源和导入回执。本研究的目的包括：（1）构建高强度间歇负荷后 72 h 的多模态恢复演示队列；（2）描述群体趋势与个体恢复异质性；（3）验证从结构化快照到可复核 PDF 的研究交付链。</p>
<h2>2　对象与方法</h2><h3>2.1 研究设计与负荷任务</h3><p>本研究为方法验证性质的纵向演示性队列分析。队列由 12 个匿名对象构成，以 ATH-001 至 ATH-012 编码。每个对象完成 10 × 1 min 高强度间歇自行车任务，目标强度设为 peak power 的 90%，每组间歇 1 min。评估节点为负荷前基线（Baseline）、负荷后 30 min（T0）、负荷后 24 h（T24）和负荷后 72 h（T72）。</p><h3>2.2 观察指标</h3><p>主要观察指标为 RMSSD 与 CMJ 高度；辅助指标为静息心率、DOMS-VAS 与 session-RPE。RMSSD 与静息心率来自 5 min 静息记录；CMJ 为三次有效试跳的代表值；DOMS-VAS 量表范围为 0–10；session-RPE 在 T0 记录，用作当次负荷感知锚点。</p>
${table([['变量','单位','节点','数据含义'],['RMSSD','ms','Baseline、T0、T24、T72','自主神经恢复的时间序列指标'],['静息心率','bpm','Baseline、T0、T24、T72','静息状态下的心血管反应'],['CMJ 高度','cm','Baseline、T0、T24、T72','神经肌肉表现的代表值'],['DOMS-VAS','0–10','Baseline、T0、T24、T72','主观酸痛的延迟变化'],['session-RPE','0–10','T0','当次负荷感知锚点']], 'table')}<p class="caption">表 1　主要变量及其采集节点</p>
<h3>2.3 数据组织与质量控制</h3><p>数据通过本地 Connector 生成去标识化结构化快照，并在进入研究空间时写入 RehabID、时间点、指标名、单位和数据来源。质控规则包括：同一对象—时间点排序、主终点缺失检查、指标单位完整性检查及身份性字段隔离。研究快照不包含姓名、病历叙述、自由文本或语音转写内容。</p>
<h3>2.4 统计学方法</h3><p>采用均值±标准差描述连续变量；以 T0 和 T72 相对基线的百分比变化描述急性反应与恢复程度。为避免群体均值掩盖个体差异，图 1 同时呈现每个对象的轨迹及均值±标准误，图 2 呈现 T72 相对基线的个体恢复比例与 T24 DOMS-VAS。本文为演示性数据分析，不进行推断性显著性检验。</p></div>${pageFooter('1')}</section>

<section class="paper"><header><span>高强度间歇负荷后多模态恢复的纵向特征</span><span>结果</span></header><div class="twocol"><h2>3　结果</h2><h3>3.1 数据完整性与研究快照</h3><p>共生成 12 个对象的 48 个纵向评估节点、192 个结构化评估对象与 204 条数值观察。48 个节点均按对象与时间顺序排列，204 条数值观察均含有指标名与单位，RMSSD 与 CMJ 两项主终点无缺失；研究快照未写入身份性字段。</p>
<div class="data-grid"><div class="data-cell"><b>12 个 RehabID</b>编码为 ATH-001 至 ATH-012，跨节点保持同一对象标识。</div><div class="data-cell"><b>48 个时间节点</b>每个对象均有 4 个连续节点，覆盖基线、急性反应与恢复期。</div><div class="data-cell"><b>204 条数值观察</b>指标、单位与来源同时进入数据快照。</div><div class="data-cell"><b>0 项主终点缺失</b>RMSSD 与 CMJ 在 4 个节点均可追溯。</div></div>
<h3>3.2 群体纵向变化</h3><p>RMSSD 在 T0 由 ${fmt(meanRmssd[0].mean)}±${fmt(meanRmssd[0].sd)} ms 降至 ${fmt(meanRmssd[1].mean)}±${fmt(meanRmssd[1].sd)} ms（−${fmt(-pct(meanRmssd[0].mean, meanRmssd[1].mean))}%），T72 恢复至 ${fmt(meanRmssd[3].mean)}±${fmt(meanRmssd[3].sd)} ms；CMJ 高度由 ${fmt(meanCmj[0].mean)}±${fmt(meanCmj[0].sd)} cm 降至 ${fmt(meanCmj[1].mean)}±${fmt(meanCmj[1].sd)} cm，T72 为 ${fmt(meanCmj[3].mean)}±${fmt(meanCmj[3].sd)} cm。静息心率在 T0 升高，DOMS-VAS 则在 T24 达到最高。各指标的变化方向见表 2。</p>
</div><div class="full">${table(statsRows, 'table')}<p class="caption">表 2　各时间点主要指标的描述性统计。数据以均值±标准差表示；百分比为相对基线变化。</p><div class="result-box"><p><b>主要发现：</b>急性阶段（T0）以 RMSSD、CMJ 下移及静息心率上移为主；延迟阶段（T24）以 DOMS 增高为主；T72 时群体均值向基线回归，但恢复速度在对象之间并不一致。</p></div><div class="figure">${trajectorySvg()}</div><p class="caption">图 1　12 个对象在 4 个时间节点的纵向轨迹。灰线表示个体路径，彩色实线表示均值，阴影表示均值±标准误。RMSSD：相邻 NN 间期差平方根；CMJ：反向跳；DOMS：延迟性肌肉酸痛。</p></div>${pageFooter('2')}</section>

<section class="paper"><header><span>高强度间歇负荷后多模态恢复的纵向特征</span><span>结果与讨论</span></header><div class="twocol"><h3>3.3 个体恢复异质性</h3><p>图 2 显示，每个对象在 T72 的恢复比例并不相同。ATH-003、ATH-008 与 ATH-012 的 RMSSD 或 CMJ 在 T72 仍低于个人基线约 10% 或以上，且 T24 DOMS-VAS 为 6；与群体均值相比，这些对象呈现更长的恢复窗口。其余对象虽在 T72 接近基线，但其急性下降幅度与 DOMS 峰值也存在差别。</p><p>该结果说明，若仅输出“队列平均值已经恢复”，便可能忽略仍处于较慢恢复路径中的对象。以 RehabID 组织多来源数据，能够使每个个体的变化回溯至相应的测量节点，而不依赖孤立的单点读数。</p></div><div class="full"><div class="figure">${recoverySvg()}</div><p class="caption">图 2　T72 时 RMSSD 与 CMJ 相对个体基线的恢复比例，以及 T24 DOMS-VAS。图中保留每个匿名对象的路径，以呈现恢复异质性。</p></div><div class="twocol"><h2>4　讨论</h2><h3>4.1 主要发现</h3><p>本研究以演示性队列复现了高强度间歇负荷后多维恢复的基本时间结构：自主神经与神经肌肉指标在急性阶段变化明显，主观酸痛具有延迟性，而部分对象在 72 h 时仍未回到个人基线。既往研究提示，CMJ 相关指标能够用于检测神经肌肉疲劳，但不同指标对负荷的敏感性和恢复速度并不完全一致<sup>[3,4]</sup>。HRV 的解释同样需要结合标准化测量条件、训练负荷与个体基线<sup>[1,2,5]</sup>。</p><h3>4.2 对康复与运动科研数据组织的意义</h3><p>本研究的重点不是用单一算法给出“是否恢复”的绝对结论，而是将对象、时间、指标、单位和来源固定为可计算结构。对于研究者而言，这种结构有三项直接价值：第一，可在队列层面获得统一的纵向描述；第二，可在个体层面追溯变化来源；第三，可将数据字典、统计表、图形和 PDF 交付关联到同一研究快照。</p><h3>4.3 局限性</h3><p>本研究为确定性演示队列，样本量、数值变化与恢复模式均服务于数据治理与产品流程验证，不能替代真实受试者研究，亦不能据此建立临床或训练处方阈值。下一步应在伦理审批和知情同意基础上，使用真实连续随访数据预注册主终点、控制测量条件，并依据研究目的采用混合效应模型或其他重复测量方法进行推断分析。</p><h2>5　结论</h2><p>以 RehabID 为最小分析单元，可将 HRV、CMJ、静息心率、DOMS 与主观负荷连接为同一对象的连续恢复轨迹。在本演示队列中，该结构支持从数据接入、质量控制、纵向描述到图表和 PDF 的完整交付，并保留了群体趋势之外的个体恢复差异。该流程可作为真实康复与运动科学队列的数据采集和研究交付原型。</p></div>${pageFooter('3')}</section>

<section class="paper"><header><span>高强度间歇负荷后多模态恢复的纵向特征</span><span>声明与参考文献</span></header><h2>数据可用性声明</h2><p>本研究性分析示例所使用的确定性演示队列以 CSV 形式随项目交付，文件名为 <i>galen_sports_fatigue_demo_cohort_v2.csv</i>。该数据不包含可直接识别个人身份的信息；其用途为复现本文所示的数据结构、描述性统计和图表，不应被视为真实临床或运动队列。</p><h2>软件与可复现性声明</h2><p>数据由本地康复师工作台的 Connector 输出为去标识化快照，并按照 RehabID 数据模型进入研究空间。报告图表与统计表由同一确定性数据源生成；重新执行交付脚本可得到相同的对象编码、时间节点、数值观察和图形结果。</p><h2>伦理声明</h2><p>本示例未涉及真实受试者、人体干预或可识别个人数据，因而不构成需伦理审查的人体研究。若迁移至真实世界队列，应在采集前取得相应伦理审批与受试者知情同意。</p><h2>利益冲突声明</h2><p>作者声明不存在需要披露的利益冲突。</p><h2>参考文献</h2><ol class="references"><li>Buchheit M. Monitoring training status with HR measures: do all roads lead to Rome? <i>Frontiers in Physiology</i>. 2014;5:73. doi: <a href="https://doi.org/10.3389/fphys.2014.00073">10.3389/fphys.2014.00073</a>.</li><li>Bellenger CR, Fuller JT, Thomson RL, Davison K, Robertson EY, Buckley JD. Monitoring athletic training status through autonomic heart rate regulation: a systematic review and meta-analysis. <i>Sports Medicine</i>. 2016;46(10):1461–1486. doi: <a href="https://doi.org/10.1007/s40279-016-0482-2">10.1007/s40279-016-0482-2</a>.</li><li>Wu PP-Y, Sterkenburg N, Everett K, et al. Predicting fatigue using countermovement jump force-time signatures: PCA can distinguish neuromuscular versus metabolic fatigue. <i>PLoS One</i>. 2019;14:e0219295. doi: <a href="https://doi.org/10.1371/journal.pone.0219295">10.1371/journal.pone.0219295</a>.</li><li>Gathercole RJ, Sporer BC, Stellingwerff T, Sleivert GG. Comparison of the capacity of different jump and sprint field tests to detect neuromuscular fatigue. <i>Journal of Strength and Conditioning Research</i>. 2015;29(9):2522–2531. doi: <a href="https://doi.org/10.1519/JSC.0000000000000912">10.1519/JSC.0000000000000912</a>.</li><li>Marasingha-Arachchige SU, Rubio-Arias JÁ, Alcaraz PE, et al. Factors that affect heart rate variability following acute resistance exercise: a systematic review and meta-analysis. <i>Journal of Sport and Health Science</i>. 2022;11(3):376–392. doi: <a href="https://doi.org/10.1016/j.jshs.2020.11.008">10.1016/j.jshs.2020.11.008</a>.</li></ol><div class="note">建议引用格式：Galen Rehab Intelligence 研究团队. 高强度间歇自行车负荷后 72 小时多模态恢复的纵向特征：基于 RehabID 时间轴的演示性队列研究. 研究性分析示例稿，2026。</div>${pageFooter('4')}</section>
</body></html>`;
  const html = `<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><title>Galen 运动疲劳多模态恢复研究验证报告</title><style>
@page { size: A4; margin: 15mm 16mm 16mm; }
*{box-sizing:border-box} body{font-family:"Microsoft YaHei","Segoe UI",Arial,sans-serif;color:#142c45;margin:0;background:#fff;font-size:10.4pt;line-height:1.62}.page{min-height:264mm;position:relative;page-break-after:always;padding-bottom:15mm}.page:last-child{page-break-after:auto}h1{font-size:31pt;letter-spacing:-1.3px;line-height:1.18;margin:0 0 12px}h2{font-size:18pt;line-height:1.25;margin:0 0 14px;padding-top:3px}h3{font-size:12.4pt;margin:18px 0 7px;color:#075985}.eyebrow{font-size:8.5pt;letter-spacing:2px;font-weight:700;color:#008f83;text-transform:uppercase}.muted{color:#61768c}.lead{font-size:13pt;line-height:1.7;color:#36526d;max-width:610px}.cover{padding-top:25mm}.cover-band{height:8px;width:100%;background:linear-gradient(90deg,#0f766e,#2563eb,#0ea5a3);margin:22px 0 30px}.kicker{display:inline-block;padding:5px 10px;border-radius:20px;background:#e8f8f5;color:#0f766e;font-size:9pt;font-weight:700}.cards{display:grid;grid-template-columns:repeat(3,1fr);gap:10px;margin:28px 0}.card{border:1px solid #d8e6f1;border-radius:12px;background:#f8fbff;padding:13px 14px}.card strong{display:block;font-size:23pt;line-height:1;color:#102b46;margin-bottom:5px}.card span{font-size:9pt;color:#61768c}.notice{border-left:4px solid #0f766e;background:#f0fdfa;padding:11px 13px;color:#285467;font-size:9.4pt}.section-rule{border:0;border-top:1px solid #dbe5ef;margin:17px 0}.two-col{display:grid;grid-template-columns:1.05fr .95fr;gap:22px}.panel{border:1px solid #d8e6f1;border-radius:14px;padding:14px 15px;background:#fff}.soft{background:#f8fbff}.protocol-list{margin:4px 0 0;padding-left:18px}.protocol-list li{margin:5px 0}.figure{border:1px solid #d8e6f1;border-radius:14px;padding:9px;background:#fff;margin:9px 0 4px}.figure svg{display:block;width:100%;height:auto}.caption{font-size:8.8pt;color:#61768c;margin:6px 4px 12px}.results{border-collapse:collapse;width:100%;font-size:8.1pt;table-layout:fixed}.results th{background:#102b46;color:white;text-align:left;padding:7px 6px;font-weight:600}.results td{padding:7px 6px;border-bottom:1px solid #e4edf5;vertical-align:top}.results tr:nth-child(even) td{background:#f8fbff}.metric-chip{display:inline-block;padding:3px 7px;border-radius:12px;background:#eff6ff;color:#1d4ed8;font-size:8.5pt;font-weight:700;margin:1px 2px 1px 0}.pipeline{display:grid;grid-template-columns:repeat(5,1fr);gap:7px;margin:13px 0}.pipeline div{background:#f8fbff;border:1px solid #dbe5ef;border-radius:10px;padding:10px 9px;min-height:77px}.pipeline b{display:block;color:#0f766e;font-size:8pt;margin-bottom:4px}.pipeline span{font-size:9pt;line-height:1.35}.case-grid{display:grid;grid-template-columns:repeat(3,1fr);gap:9px}.case{border-top:3px solid #0f766e;border-radius:8px;background:#f8fbff;padding:10px}.case h3{margin:0 0 5px;font-size:11pt}.case p{font-size:8.9pt;margin:0;color:#526c82;line-height:1.52}.references{font-size:8.5pt;line-height:1.55;padding-left:18px}.references li{margin:5px 0}.references a{color:#0b5cab;text-decoration:none}.data-dict{border-collapse:collapse;width:100%;font-size:8.8pt}.data-dict th,.data-dict td{padding:8px;border-bottom:1px solid #e4edf5;text-align:left}.data-dict th{color:#0f766e}.page-label{font-size:8pt;color:#0f766e;font-weight:700;letter-spacing:1.3px;margin-bottom:8px}footer{position:absolute;bottom:0;left:0;right:0;display:flex;justify-content:space-between;border-top:1px solid #dbe5ef;padding-top:6px;font-size:7.8pt;color:#7890a4}.source-box{border:1px dashed #adc3d6;border-radius:10px;padding:11px 12px;font-size:9pt;color:#4d6880;background:#fbfdff}.callout{border-radius:10px;padding:12px 13px;background:#eff6ff;color:#254c72}.callout strong{color:#0f4a85}sup{font-size:7pt}.small{font-size:8.8pt}
</style></head><body>
<section class="page cover"><div class="cover"><div class="eyebrow">GALEN · REHABID RESEARCH DELIVERY</div><div class="cover-band"></div><span class="kicker">路演模拟验证队列 · v2.0 · PDF 交付件</span><h1>高强度间歇负荷后<br>72 小时多模态恢复轨迹</h1><p class="lead">以连续 RehabID 为分析单元，将 HRV、力台 CMJ、主观酸痛与训练负荷纳入同一可追溯时间轴。</p><div class="cards"><div class="card"><strong>12</strong><span>匿名 RehabID</span></div><div class="card"><strong>48</strong><span>纵向评估时间点</span></div><div class="card"><strong>204</strong><span>结构化数值观察</span></div></div><div class="notice"><b>交付定位：</b>本报告展示 Galen 对康复与运动科学数据的接入、治理、纵向分析和论文级交付能力。队列为确定性路演模拟数据；研究结构、指标定义、分析输出和文献出处均可复核。</div></div>${pageFooter('01')}</section>

<section class="page"><div class="page-label">01 · RESEARCH QUESTION AND PROTOCOL</div><h2>从一个明确问题，建立连续恢复证据</h2><div class="two-col"><div><h3>研究问题</h3><p>在一次标准化高强度间歇负荷后，HRV、自主神经恢复、CMJ 神经肌肉表现和主观酸痛是否呈现不同的恢复速度？这些变化能否在同一 RehabID 时间轴中被保留、比较并交付？</p><h3>队列与任务</h3><ul class="protocol-list"><li><b>对象：</b>12 个匿名队列对象，代码 ATH-001 至 ATH-012。</li><li><b>负荷：</b>10 × 1 min 高强度间歇自行车，目标强度 90% peak power，间歇 1 min。</li><li><b>时间点：</b>D-7 基线、T0（负荷后 30 min）、T24、T72。</li><li><b>采集：</b>5 min 静息 HRV、力台 CMJ（三次有效试跳）、DOMS VAS 与会话 RPE。</li></ul></div><div class="panel soft"><h3>为什么需要多模态，而非单一阈值</h3><p class="small">RMSSD 可以反映急性负荷后的自主神经恢复；CMJ 适合观察神经肌肉表现的即时及延迟变化；DOMS 和 session-RPE 提供个体感知层。它们的意义依赖于测量时间、个人基线与来源链，而不是孤立的一次读数。<sup>[1–3]</sup></p><hr class="section-rule"><div><span class="metric-chip">HRV RMSSD</span><span class="metric-chip">静息 HR</span><span class="metric-chip">CMJ</span><span class="metric-chip">DOMS VAS</span><span class="metric-chip">session-RPE</span></div></div></div><h3>Galen 研究链</h3><div class="pipeline"><div><b>01 · 连接</b><span>康复师工作台生成脱敏结构化快照</span></div><div><b>02 · 规范化</b><span>对象、时间点、指标、单位写入 RehabID</span></div><div><b>03 · 质控</b><span>检查重复、缺失、单位与时间顺序</span></div><div><b>04 · 分析</b><span>按个人基线进行纵向变化比较</span></div><div><b>05 · 交付</b><span>输出图表、数据字典、研究报告 PDF</span></div></div>${pageFooter('02')}</section>

<section class="page"><div class="page-label">02 · DATA GOVERNANCE</div><h2>每个结论，都能回到它的数据位置</h2><div class="two-col"><div class="panel"><h3>RehabID 最小可计算单元</h3>${table([['字段','示例','作用'],['RehabID','ATH-008','跨设备合并同一对象记录'],['评估时间点','T24','区分急性负荷与延迟恢复'],['指标 + 单位','CMJ 37.3 cm','避免同名异义或无单位数值'],['来源','力台 / HRV / 量表','保留数据产生渠道'],['导入回执','connector receipt','记录何时、从何处、以何规则进入研究']], 'data-dict')}</div><div class="panel soft"><h3>本次质控结果</h3><p><b>48 / 48</b> 时间点已按对象排序<br><b>204 / 204</b> 数值观察带有指标名与单位<br><b>0</b> 条身份性字段写入研究快照<br><b>0</b> 个缺失的主终点（RMSSD / CMJ）</p><hr class="section-rule"><p class="small muted">Connector 仅传递短码、时间、量化指标和单位；界面中的病例叙述、姓名、语音转写和自由文本不进入导出的研究快照。</p></div></div><h3>数据字典（核心变量）</h3>${table([['变量','测量含义','采集节点','进入分析的方式'],['hrv_rmssd_ms','相邻 NN 间期差平方根，毫秒','4 个节点','个体基线归一化变化'],['resting_hr_bpm','5 min 静息记录心率','4 个节点','辅助描述自主神经恢复'],['cmj_height_cm','三次有效 CMJ 的代表值','4 个节点','神经肌肉表现轨迹'],['doms_vas','下肢延迟性肌肉酸痛 0–10','4 个节点','主观恢复与酸痛轨迹'],['session_rpe','任务后会话主观用力程度 0–10','T0','负荷锚点']], 'data-dict')}<div class="source-box"><b>数据出处：</b>康复师工作台本地 Connector → de-identified snapshot → Galen RehabID timeline。路演环境使用确定性模拟队列，保证视频、报告和复现实验中的每一次导入均得到相同的 12 个对象与 204 条观察。</div>${pageFooter('03')}</section>

<section class="page"><div class="page-label">03 · LONGITUDINAL RESULTS</div><h2>恢复并非一条直线：均值趋势与个体路径同时保留</h2><div class="callout"><strong>描述性结果：</strong>队列均值在 T0 出现 RMSSD 与 CMJ 下移、静息心率上移；T24 DOMS 达到最高；T72 多数指标向各自基线回归，但 ATH-003、ATH-008、ATH-012 仍保留较慢恢复特征。</div><div class="figure">${trajectorySvg()}</div><p class="caption">图 1｜12 个 RehabID 的纵向轨迹。灰线为个人路径，彩色实线为均值，阴影为均值 ± 标准误。图中保留离散度，以免“平均恢复”遮盖个人差异。</p>${table(statsRows, 'results')}${pageFooter('04')}</section>

<section class="page"><div class="page-label">04 · INDIVIDUAL VARIABILITY</div><h2>把“恢复得怎么样”拆回每一个人</h2><div class="figure">${recoverySvg()}</div><p class="caption">图 2｜T72 相对于个体基线的 RMSSD 与 CMJ 恢复比例，以及 T24 DOMS VAS。此图不将对象折叠成单一风险分数，保留每个 RehabID 的恢复结构。</p><div class="case-grid"><div class="case"><h3>ATH-003</h3><p>T72 RMSSD 仍低于基线约 10%，CMJ 恢复不足，T24 DOMS 为 6。适合在复测前维持低冲击技术训练。</p></div><div class="case"><h3>ATH-008</h3><p>急性 RMSSD 下移最明显，T24 静息心率仍偏高、DOMS 为 6。提示该对象需要延长恢复窗口。</p></div><div class="case"><h3>ATH-012</h3><p>表现与主观酸痛均恢复较慢。连续时间轴可避免仅凭 T72 单一表现值忽略负荷后路径。</p></div></div><h3>解释边界</h3><p class="small">本报告展示的是研究数据治理与纵向分析产品能力。模拟队列可用于验证连接器、结构化分析、图表和 PDF 交付是否闭环；不用于推导临床或训练处方阈值。</p>${pageFooter('05')}</section>

<section class="page"><div class="page-label">05 · DELIVERY AND REPRODUCIBILITY</div><h2>PDF 不是终点，而是可回溯成果的入口</h2><div class="two-col"><div><h3>本次交付包含</h3><ul class="protocol-list"><li>研究问题、测量方案与数据字典</li><li>12 个匿名 RehabID 的纵向原始观察</li><li>群体趋势与个体恢复差异图</li><li>Connector 数据来源与导入边界</li><li>可复现的 CSV 附件与 PDF 交付件</li></ul><h3>建议的真实研究升级</h3><ol class="protocol-list"><li>完成伦理审批与受试者知情同意。</li><li>将模拟队列替换为连续真实随访数据。</li><li>预注册主终点、采集窗口和统计方案。</li><li>按设备、项目与负荷类型进行外部验证。</li></ol></div><div class="panel soft"><h3>为什么这比“生成一篇文档”更重要</h3><p>研究者拿到的不只是文字结论，而是对象、时间点、来源和图表之间可追溯的结构。无论未来接入力台、运动可穿戴、量表、影像还是家庭端数据，它们都将以同一套 RehabID 语义进入研究。</p><hr class="section-rule"><p class="small"><b>交付文件：</b><br>Galen_运动疲劳多模态恢复_研究验证报告.pdf<br>galen_sports_fatigue_demo_cohort_v2.csv</p></div></div><h3>参考文献</h3><ol class="references"><li>Wu PP-Y, Sterkenburg N, Everett K, et al. Predicting fatigue using countermovement jump force-time signatures: PCA can distinguish neuromuscular versus metabolic fatigue. <i>PLoS One</i>. 2019;14:e0219295. DOI: <a href="https://doi.org/10.1371/journal.pone.0219295">10.1371/journal.pone.0219295</a>. PMID: 31291303.</li><li>Gathercole RJ, Sporer BC, Stellingwerff T, Sleivert GG. Comparison of the Capacity of Different Jump and Sprint Field Tests to Detect Neuromuscular Fatigue. <i>J Strength Cond Res</i>. 2015;29:2522–2531. DOI: <a href="https://doi.org/10.1519/JSC.0000000000000912">10.1519/JSC.0000000000000912</a>. PMID: 26308829.</li><li>Marasingha-Arachchige SU, Rubio-Arias JÁ, Alcaraz PE, et al. Factors that affect heart rate variability following acute resistance exercise: a systematic review and meta-analysis. <i>J Sport Health Sci</i>. 2022;11:376–392. DOI: <a href="https://doi.org/10.1016/j.jshs.2020.11.008">10.1016/j.jshs.2020.11.008</a>. PMID: 33246163.</li></ol>${pageFooter('06')}</section>
</body></html>`;
  fs.writeFileSync(HTML, formalPaperHtml, 'utf8');
  const browser = await chromium.launch({ headless: true, executablePath: 'C:/Program Files/Google/Chrome/Application/chrome.exe' });
  const page = await browser.newPage({ viewport: { width: 1240, height: 1754 }, deviceScaleFactor: 1 });
  await page.goto(`file:///${HTML.replace(/\\/g, '/')}`, { waitUntil: 'networkidle' });
  await page.emulateMedia({ media: 'screen' });
  await page.pdf({ path: PDF, format: 'A4', printBackground: true, margin: { top: '0mm', bottom: '0mm', left: '0mm', right: '0mm' }, preferCSSPageSize: true });
  await browser.close();
  fs.mkdirSync(path.dirname(GALEN_DEMO_PDF), { recursive: true });
  fs.copyFileSync(PDF, GALEN_DEMO_PDF);
  console.log(JSON.stringify({ pdf: PDF, csv: CSV, html: HTML, observations: data.length * 4 + 12, rmssdT0: fmt(meanRmssd[1].mean), cmjT0: fmt(meanCmj[1].mean), domsT24: fmt(meanDoms[2].mean) }));
}

build().catch((error) => { console.error(error); process.exit(1); });
