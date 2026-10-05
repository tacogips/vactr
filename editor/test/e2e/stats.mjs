export const THRESHOLDS = Object.freeze({
  inputP95Ms: 50, inputP99Ms: 100,
  animationWorkP50Ms: 4, animationWorkP95Ms: 8, animationWorkP99Ms: 16.7,
  textWorkP95Ms: 16.7, frameIntervalP95Ms: 20, frameIntervalP99Ms: 50,
  syncAbsP95Ms: 33.4, syncAbsP99Ms: 50, earlyFlashMs: 2,
  ledgerMiB: 96, heapGrowthMiB: 8, lateBeatDriftMs: 1,
});

export function percentile(values, p) {
  if (!Array.isArray(values) || values.length === 0 || !Number.isFinite(p) || p < 0 || p > 100) return null;
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  if (!sorted.length) return null;
  return sorted[Math.max(0, Math.ceil((p / 100) * sorted.length) - 1)];
}

export function pairInputLatency(frames, keys) {
  const orderedFrames = frames.filter((row) => Number.isFinite(Number(row[0])) && Number.isFinite(Number(row[3])))
    .map((row) => row.map(Number)).sort((a, b) => a[0] - b[0]);
  const orderedKeys = keys.filter((row) => Number.isFinite(Number(row[0])) && Number.isFinite(Number(row[1])))
    .map((row) => row.map(Number)).sort((a, b) => a[0] - b[0]);
  const paired = [];
  let nonEditingKeys = 0;
  let expiredKeys = 0;
  let unpairedKeys = 0;
  let frameCursor = 0;
  let baseline = null;
  for (let index = 0; index < orderedKeys.length; index += 1) {
    const [keyTime, keyRevision] = orderedKeys[index];
    while (frameCursor < orderedFrames.length && orderedFrames[frameCursor][0] <= keyTime) {
      baseline = orderedFrames[frameCursor];
      frameCursor += 1;
    }
    if (orderedFrames.length && keyTime < orderedFrames[0][0]) {
      expiredKeys += 1;
      continue;
    }
    const baselineRevision = baseline ? baseline[3] : Number.NEGATIVE_INFINITY;
    const nextKey = orderedKeys[index + 1];
    let targetRevision = null;
    if (keyRevision > baselineRevision) targetRevision = keyRevision;
    else if (nextKey && nextKey[0] - keyTime <= 1000 && nextKey[1] > keyRevision) targetRevision = keyRevision + 1;
    else {
      nonEditingKeys += 1;
      continue;
    }
    const deadline = keyTime + 1000;
    const frame = orderedFrames.find((row) => row[0] >= keyTime && row[0] <= deadline && row[3] >= targetRevision);
    if (!frame) {
      unpairedKeys += 1;
      continue;
    }
    paired.push({ keyTime, keyRevision, frameTime: frame[0], frameRevision: frame[3], latencyMs: frame[0] - keyTime });
  }
  return { paired, pairedKeys: paired.length, nonEditingKeys, expiredKeys, unpairedKeys };
}

export function countChecks(checks) {
  const gated = checks.filter((check) => check.status !== 'limitation');
  return {
    total: gated.length,
    passed: gated.filter((check) => check.pass === true).length,
    failed: gated.filter((check) => check.pass !== true).length,
  };
}

export function evaluate(summary, thresholds = THRESHOLDS) {
  const failures = [];
  const metrics = summary?.metrics ?? {};
  for (const [key, path] of [['inputLatencyMs','p95'],['inputLatencyMs','p99'],['animationWorkMs','p50'],['animationWorkMs','p95'],['animationWorkMs','p99'],['textWorkMs','p95'],['frameIntervalMs','p95'],['frameIntervalMs','p99']]) if (!Number.isFinite(metrics[key]?.[path])) failures.push(`${key}.${path} unavailable`);
  if (!Number.isFinite(metrics.editKeyCount)) failures.push('editing keystroke count unavailable');
  else if (metrics.editKeyCount < 500) failures.push(`editing keystrokes=${metrics.editKeyCount}, expected at least 500`);
  if (!Number.isFinite(metrics.editPairedKeyCount)) failures.push('paired editing sample count unavailable');
  else if (metrics.editPairedKeyCount <= 0) failures.push('paired editing samples=0, expected at least 1');
  if (Number.isFinite(metrics.editKeyCount) && Number.isFinite(metrics.editUnpairedKeys) && metrics.editKeyCount > 0 && metrics.editUnpairedKeys / metrics.editKeyCount > 0.1) failures.push(`unpaired editing keys=${metrics.editUnpairedKeys}/${metrics.editKeyCount}, expected at most 10%`);
  if (metrics.audioRunning === true && (!Number.isFinite(metrics.onsetCount) || metrics.onsetCount === 0)) failures.push('no playing onset telemetry; active audio/visual workload was not observed');
  const limit = (key, p, max) => {
    const v = metrics[key]?.[`p${p}`];
    if (Number.isFinite(v) && v > max) failures.push(`${key}.p${p}=${v} exceeds ${max}`);
  };
  limit('inputLatencyMs', 95, thresholds.inputP95Ms);
  limit('inputLatencyMs', 99, thresholds.inputP99Ms);
  limit('animationWorkMs', 50, thresholds.animationWorkP50Ms);
  limit('animationWorkMs', 95, thresholds.animationWorkP95Ms);
  limit('animationWorkMs', 99, thresholds.animationWorkP99Ms);
  limit('textWorkMs', 95, thresholds.textWorkP95Ms);
  limit('frameIntervalMs', 95, thresholds.frameIntervalP95Ms);
  limit('frameIntervalMs', 99, thresholds.frameIntervalP99Ms);
  if (metrics.syncProvenance === 'measured') {
    if(!Number.isFinite(metrics.syncAbsMs?.p95)||!Number.isFinite(metrics.syncAbsMs?.p99)) failures.push('measured sync samples unavailable');
    limit('syncAbsMs', 95, thresholds.syncAbsP95Ms);
    limit('syncAbsMs', 99, thresholds.syncAbsP99Ms);
    if ((metrics.earlyFlashCount ?? 0) > 0) failures.push(`early flashes=${metrics.earlyFlashCount}`);
  }
  if ((metrics.ledgerMaxBytes ?? 0) > thresholds.ledgerMiB * 1024 * 1024) failures.push('resource ledger exceeded 96 MiB');
  if (Number.isFinite(metrics.heapGrowthBytes) && metrics.heapGrowthBytes > thresholds.heapGrowthMiB * 1024 * 1024) failures.push('heap growth exceeded 8 MiB');
  if (Number.isFinite(metrics.beatDriftMs) && Math.abs(metrics.beatDriftMs) > thresholds.lateBeatDriftMs) failures.push('beat drift exceeded 1 ms');
  if ((metrics.replayedFlashCount ?? 0) > 0) failures.push('expired highlights replayed after stall');
  if ((metrics.lateActiveMismatchCount ?? 0) > 0) failures.push(`post-stall active-set mismatches=${metrics.lateActiveMismatchCount}`);
  if (metrics.ledgerAfterDisposeBytes !== undefined && metrics.ledgerAfterDisposeBytes !== 0) failures.push(`ledger after dispose=${metrics.ledgerAfterDisposeBytes}`);
  return { pass: failures.length === 0, failures };
}

const cell = (value) => value === null || value === undefined ? 'unavailable' : String(value);
export function renderEvidence(summary) {
  const browsers = summary.browsers ?? [];
  const rows = [];
  for (const browser of browsers) {
    const m = browser.metrics ?? {};
    for (const [name, value, threshold] of [
      ['Input latency p95 (ms)', m.inputLatencyMs?.p95, '<= 50'],
      ['Input latency p99 (ms)', m.inputLatencyMs?.p99, '<= 100'],
      ['Input samples (paired / expired)', m.inputSamplesUsed == null ? null : `${m.inputSamplesUsed} / ${m.inputKeysExpired ?? 0}`, 'expired keys are outside the retained frame ring'],
      ['Editing keystrokes / paired', m.editKeyCount == null ? null : `${m.editKeyCount} / ${m.editPairedKeyCount ?? 0}`, '>= 500 keys; paired > 0; unpaired <= 10%'],
      ['Audio control / run start', m.controlOnsetCount == null ? null : `${m.controlOnsetCount} onsets / ${m.controlStartMethod ?? 'unknown'}`, m.onsetAttribution ?? 'attribution unavailable'],
      ['Animation frame work p50 (ms)', m.animationWorkMs?.p50, '<= 4'],
      ['Animation frame work p95 (ms)', m.animationWorkMs?.p95, '<= 8'],
      ['Animation frame work p99 (ms)', m.animationWorkMs?.p99, '<= 16.7'],
      ['Text-dirty frame work p95 (ms)', m.textWorkMs?.p95, '<= 16.7'],
      ['Frame interval p95 (ms)', m.frameIntervalMs?.p95, '<= 20'],
      ['Frame interval p99 (ms)', m.frameIntervalMs?.p99, '<= 50'],
      ['A/V model absolute error p99 (ms)', m.syncAbsMs?.p99, 'measured only; <= 50'],
      ['Early flashes', m.earlyFlashCount, '0; none earlier than 2 ms'],
      ['Post-stall active-set mismatches', m.lateActiveMismatchCount, '0'],
      ['A/V model absolute error p95 (ms)', m.syncAbsMs?.p95, 'measured only; <= 33.4'],
      ['Resource ledger peak (MiB)', m.ledgerMaxBytes == null ? null : (m.ledgerMaxBytes / 1048576).toFixed(2), '<= 96'],
      ['JS heap growth (MiB)', m.heapGrowthBytes == null ? null : (m.heapGrowthBytes / 1048576).toFixed(2), '<= 8'],
      ['Beat drift (ms)', m.beatDriftMs, '<= 1'],
      ['Replayed flashes', m.replayedFlashCount, '0'],
      ['Ledger after dispose (bytes)', m.ledgerAfterDisposeBytes, '0'],
    ]) rows.push(`| ${browser.name} | ${name} | ${cell(value)} | ${threshold} |`);
  }
  const status = summary.pass ? 'PASS' : summary.blocked ? 'BLOCKED' : 'FAIL';
  const ascii = (s) => String(s).replace(/[^\x00-\x7F]/g, '?');
  return ascii(`### Run ${summary.runId}: ${status}\n\n` +
    `Commands: \`${summary.commands?.join('; ') ?? 'not recorded'}\`.\n\n` +
    '| Browser | Metric | Result | Threshold |\n|---|---|---:|---:|\n' + rows.join('\n') +
    `\n\nBehavior checks: ${browsers.map((b) => `${b.name} ${b.behavior?.passed ?? 0}/${b.behavior?.total ?? 0}`).join('; ')}. ` +
    `Failed checks: ${browsers.flatMap((b) => (b.checks ?? []).filter((c) => c.status !== 'limitation' && !c.pass).map((c) => `${b.name}:${c.id}`)).join(', ') || 'none'}. ` +
    `Measurement failures: ${browsers.flatMap((b) => (b.measurement?.failures ?? []).map((failure) => `${b.name}:${String(failure).split('\n')[0]}`)).join('; ') || 'none'}. ` +
    `Limitations: ${[...new Set(browsers.flatMap((b) => b.limitations ?? []))].join('; ') || 'none recorded'}.\n`);
}
