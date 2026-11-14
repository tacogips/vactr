import { percentile, THRESHOLDS } from './stats.mjs';

export const RENDERERS = Object.freeze(['canvas', 'dom']);
export const BROWSERS = Object.freeze(['chromium', 'webkit']);
export const DEFAULT_LINES = Object.freeze([1000, 20000]);
export const DEFAULT_RUNS = 3;
export const MAX_RUN_JSONL_BYTES = 870_000;
export const COMPARISON_BEGIN = '<!-- RENDERER-COMPARISON:BEGIN -->';
export const COMPARISON_END = '<!-- RENDERER-COMPARISON:END -->';
export const REPORT_SKELETON = `# Canvas vs DOM Renderer Comparison

Status: Pending measured data. Design: [DOM renderer design](design-dom-renderer.md).

${COMPARISON_BEGIN}
${COMPARISON_END}

## Analysis
Pending measured data.

## Limitations
Pending measured data.

## Observations for the canvas tuning proposal
Pending measured data.
`;

export const METRIC_ROWS = Object.freeze([
  ['inputP50', 'Input latency p50', 'ms'], ['inputP95', 'Input latency p95', 'ms'], ['inputP99', 'Input latency p99', 'ms'],
  ['frameP95', 'Frame interval p95', 'ms'], ['frameP99', 'Frame interval p99', 'ms'], ['textP95', 'Text work p95', 'ms'],
  ['animP50', 'Animation work p50', 'ms'], ['animP95', 'Animation work p95', 'ms'], ['syncP95', 'Sync p95', 'ms'], ['syncP99', 'Sync p99', 'ms'],
  ['stallSyncCount', 'Stall sync samples', 'count'], ['stallSyncP50', 'Stall sync p50', 'ms'], ['stallSyncP95', 'Stall sync p95', 'ms'], ['stallSyncMax', 'Stall sync max', 'ms'],
  ['lateActiveMismatchCount', 'Late active mismatches', 'count'], ['replayedFlashCount', 'Replayed flashes', 'count'], ['heapGrowthBytes', 'JS heap growth', 'bytes'],
  ['domNodesPeak', 'DOM nodes peak', 'count'], ['domNodesFinal', 'DOM nodes final', 'count'], ['ledgerMaxBytes', 'GPU ledger max', 'bytes'],
  ['mountToFirstFrameMs', 'Mount to first frame', 'ms'], ['loadToViewportMs', 'Load to viewport', 'ms'],
].map(([key, label, unit]) => Object.freeze({ key, label, unit, lowerIsBetter: true })));

export function sliceWorkload(workload, lines) {
  if (!Number.isInteger(lines) || lines < 8 || lines > workload.lines) throw new RangeError(`invalid workload line count: ${lines}`);
  const rows = workload.text.split('\n').slice(0, lines);
  const text = rows.join('\n');
  return { ...workload, text, lines, bytes: new TextEncoder().encode(text).byteLength, sourceLines: workload.lines, longLines: rows.filter((row) => row.length > 400).length };
}

export function runOrder(runs, renderers) {
  return Array.from({ length: runs }, (_, index) => {
    const ordered = (index % 2 === 0 ? renderers : [...renderers].reverse());
    return ordered.map((renderer, position) => ({ run: index + 1, renderer, position }));
  }).flat();
}

export function runKey({ lines, browser, renderer, run }) { return `${lines}-${browser}-${renderer}-r${run}`; }
const numberOrNull = (value) => typeof value === 'number' && Number.isFinite(value) ? value : null;
const p = (values, quantile) => percentile(values.filter(Number.isFinite), quantile);
export function inputP50(samples) { return p(samples.filter((row) => row.phase === 'input-pair').map((row) => row.latencyMs), 50); }

export function extractRunMetrics(measured, extras = {}) {
  const metrics = measured.metrics ?? {};
  const domNodes = metrics.domNodes ?? {};
  const stall = metrics.stallWindowSync ?? {};
  return {
    inputP50: inputP50(measured.samples ?? []), inputP95: numberOrNull(metrics.inputLatencyMs?.p95), inputP99: numberOrNull(metrics.inputLatencyMs?.p99),
    frameP95: numberOrNull(metrics.frameIntervalMs?.p95), frameP99: numberOrNull(metrics.frameIntervalMs?.p99), textP95: numberOrNull(metrics.textWorkMs?.p95),
    animP50: numberOrNull(metrics.animationWorkMs?.p50), animP95: numberOrNull(metrics.animationWorkMs?.p95),
    syncP95: numberOrNull(metrics.syncAbsMs?.p95), syncP99: numberOrNull(metrics.syncAbsMs?.p99), stallSyncCount: numberOrNull(stall.count),
    stallSyncP50: numberOrNull(stall.p50), stallSyncP95: numberOrNull(stall.p95), stallSyncMax: numberOrNull(stall.max),
    lateActiveMismatchCount: numberOrNull(metrics.lateActiveMismatchCount), replayedFlashCount: numberOrNull(metrics.replayedFlashCount),
    heapGrowthBytes: numberOrNull(metrics.heapGrowthBytes), domNodesPeak: numberOrNull(domNodes.peak), domNodesFinal: numberOrNull(domNodes.final),
    ledgerMaxBytes: numberOrNull(metrics.ledgerMaxBytes), mountToFirstFrameMs: numberOrNull(extras.mountToFirstFrameMs), loadToViewportMs: numberOrNull(extras.loadToViewportMs),
    gatePass: Boolean(measured.pass), failures: Array.isArray(measured.failures) ? measured.failures : [],
  };
}

export function aggregateCell(records, key) {
  const values = records.map((record) => numberOrNull(record.metrics?.[key])).filter((value) => value !== null);
  const sorted = [...values].sort((a, b) => a - b);
  return { values, median: p(values, 50), min: sorted[0] ?? null, max: sorted.at(-1) ?? null, missing: records.length - values.length };
}

export function winner(canvasAgg, domAgg) {
  if (canvasAgg.median === null || domAgg.median === null) return 'unavailable';
  const variance = Math.max((canvasAgg.max ?? canvasAgg.median) - (canvasAgg.min ?? canvasAgg.median), (domAgg.max ?? domAgg.median) - (domAgg.min ?? domAgg.median));
  if (Math.abs(canvasAgg.median - domAgg.median) <= variance) return 'no clear difference';
  return canvasAgg.median < domAgg.median ? 'canvas' : 'dom';
}

export function downsampleSamples(samples, stride = 32) {
  const phases = new Set(['frame', 'presented', 'onset', 'sync-sample', 'stall-audit']);
  const seen = new Map(); const rows = [];
  for (const row of samples) {
    if (phases.has(row.phase)) { const index = seen.get(row.phase) ?? 0; seen.set(row.phase, index + 1); if (index % stride === 0) rows.push(row); }
    else if (row.phase === 'phase') { if ((row.row?.slice(2).reduce((sum, value) => sum + (Number(value) || 0), 0) ?? 0) >= 50) rows.push(row); }
    else rows.push(row);
  }
  return { rows, stride };
}

export function contended(load1, cpus) { return Number(load1) > Number(cpus); }
export function checkLock(ownerText) { const owner = String(ownerText ?? '').trim(); return { ok: owner.length > 0, owner: owner || null }; }

const format = (value, unit = '') => value === null || value === undefined ? 'unavailable' : `${value}${unit ? ` ${unit}` : ''}`;
function threshold(key) {
  const map = { inputP95: 'inputP95Ms', inputP99: 'inputP99Ms', frameP95: 'frameIntervalP95Ms', frameP99: 'frameIntervalP99Ms', textP95: 'textWorkP95Ms', animP50: 'animationWorkP50Ms', syncP95: 'syncAbsP95Ms', syncP99: 'syncAbsP99Ms' };
  const value = THRESHOLDS[map[key]];
  return Number.isFinite(value) ? `${value} ms` : '-';
}
export function renderComparison(comparison) {
  const lines = ['## Generated comparison', ''];
  const cells = comparison.cells?.length ? comparison.cells : DEFAULT_LINES.flatMap((lineCount) => BROWSERS.map((browser) => ({ lines: lineCount, browser, metrics: {}, gate: {}, runs: [] })));
  if (!(comparison.cells ?? []).length) lines.push('No complete raw runs are available; the expected matrix is shown with unavailable values.', '');
  for (const cell of cells) {
    lines.push(`### ${cell.lines} lines · ${cell.browser}`, '', '| Metric | Canvas median | DOM median | Winner | Threshold |', '|---|---:|---:|---|---:|');
    for (const row of METRIC_ROWS) {
      const metric = cell.metrics?.[row.key];
      lines.push(`| ${row.label} | ${format(metric?.canvas?.median, row.unit)} | ${format(metric?.dom?.median, row.unit)} | ${metric?.winner ?? 'unavailable'} | ${threshold(row.key)} |`);
    }
    lines.push('', '#### Per-run values', '', '| Renderer | Run | Values | Gate | Failures |', '|---|---:|---|---|---|');
    for (const record of cell.runs ?? []) lines.push(`| ${record.renderer} | ${record.run} | ${METRIC_ROWS.map((row) => `${row.key}=${format(record.metrics?.[row.key])}`).join('; ')} | ${record.gate?.pass ? 'pass' : 'fail'} | ${(record.gate?.failures ?? []).join(', ') || '-'} |`);
    lines.push('', '#### Gate results', '', `Canvas: ${cell.gate?.canvas?.passed ?? 0}/${cell.gate?.canvas?.runs ?? 0}; DOM: ${cell.gate?.dom?.passed ?? 0}/${cell.gate?.dom?.runs ?? 0}.`, '');
  }
  lines.push('## Method and environment', '', 'Measurements reuse the shared workload, silent sink, browser launch settings and unchanged gates. Text work measures rAF JavaScript time; it excludes browser style, layout and paint. Canvas GPU execution is also excluded.', '', '```json', JSON.stringify(comparison.environment ?? {}, null, 2), '```', '', '## Contended runs and attempts', '', `Contended runs: ${(comparison.contendedRuns ?? []).map((run) => run.key).join(', ') || 'none recorded'}.`, '', '```json', JSON.stringify(comparison.attempts ?? [], null, 2), '```', '');
  return lines.join('\n');
}

export function replaceMarked(source, section) {
  const start = source.indexOf(COMPARISON_BEGIN); const end = source.indexOf(COMPARISON_END);
  if (start < 0 || end < start) throw new Error('comparison report markers missing or reversed');
  return `${source.slice(0, start + COMPARISON_BEGIN.length)}\n${section}\n${source.slice(end)}`;
}

export function buildComparison(runRecords, environment, attempts) {
  const keys = new Map();
  for (const record of runRecords) { const key = `${record.lines}:${record.browser}`; if (!keys.has(key)) keys.set(key, []); keys.get(key).push(record); }
  const cells = [...keys.entries()].map(([key, runs]) => {
    const [lines, browser] = key.split(':'); const metrics = {};
    for (const row of METRIC_ROWS) { const canvas = aggregateCell(runs.filter((item) => item.renderer === 'canvas'), row.key); const dom = aggregateCell(runs.filter((item) => item.renderer === 'dom'), row.key); metrics[row.key] = { canvas, dom, winner: winner(canvas, dom) }; }
    const gate = Object.fromEntries(RENDERERS.map((renderer) => { const matching = runs.filter((item) => item.renderer === renderer); return [renderer, { passed: matching.filter((item) => item.gate?.pass).length, runs: matching.length }]; }));
    return { lines: Number(lines), browser, metrics, gate, runs: runs.sort((a, b) => a.run - b.run || a.position - b.position) };
  }).sort((a, b) => a.lines - b.lines || a.browser.localeCompare(b.browser));
  return { runId: environment?.runId ?? null, environment, cells, attempts, contendedRuns: runRecords.filter((record) => record.contended).map(({ key, loadavgStart, loadavgEnd }) => ({ key, loadavgStart, loadavgEnd })) };
}
