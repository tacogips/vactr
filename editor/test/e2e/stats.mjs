export const THRESHOLDS = Object.freeze({
  inputP95Ms: 50, inputP99Ms: 100,
  animationWorkP50Ms: 4, animationWorkP95Ms: 8, animationWorkP99Ms: 16.7,
  textWorkP95Ms: 8, frameIntervalP95Ms: 20, frameIntervalP99Ms: 50,
  syncAbsP95Ms: 33.4, syncAbsP99Ms: 50, earlyFlashMs: 2,
  ledgerMiB: 96, heapGrowthMiB: 8, lateBeatDriftMs: 1,
});
export const TARGETS = Object.freeze({ textWorkP95Ms: 4, animationWorkP50Ms: 1 });
export const RENDER_QUANTUM_FRAMES = 128;

export function percentile(values, p) {
  if (!Array.isArray(values) || values.length === 0 || !Number.isFinite(p) || p < 0 || p > 100) return null;
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  if (!sorted.length) return null;
  return sorted[Math.max(0, Math.ceil((p / 100) * sorted.length) - 1)];
}

export function phaseSummary(rows) {
  const names = ['input', 'caret', 'shaping', 'syntax', 'upload', 'frame', 'tick'];
  return Object.fromEntries(names.map((name, index) => {
    const values = (rows ?? []).map((row) => Number(row[index + 2])).filter((value) => Number.isFinite(value) && value > 0);
    return [name, { spans: values.length, p50: percentile(values, 50), p95: percentile(values, 95),
      p99: percentile(values, 99), totalMs: values.reduce((sum, value) => sum + value, 0) }];
  }));
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

export function attributeWorkloadOnsets(rows, baselineKeys, clickMs) {
  const baseline = baselineKeys instanceof Set ? baselineKeys : new Set(baselineKeys ?? []);
  const workload = []; let excludedBaseline = 0; let excludedPreClick = 0;
  for (const row of rows ?? []) {
    if (!(Number(row.receivedMs) > clickMs)) { excludedPreClick += 1; continue; }
    if (baseline.has(JSON.stringify(row))) { excludedBaseline += 1; continue; }
    workload.push(row);
  }
  return { workload, count:workload.length, excludedBaseline, excludedPreClick };
}

export function splitSinkOnsets(times, ctxTime) {
  const control = (times ?? []).filter((time) => Number(time) <= ctxTime).length;
  return { control, workload:(times ?? []).filter((time) => Number(time) > ctxTime).length };
}

const rangeKey = (row) => `${row.from}-${row.to}`;
const activeRanges = (key) => new Set(String(key ?? '').split(',').filter(Boolean));
const sameEpoch = (a, b) => (a.epoch ?? null) === (b.epoch ?? null);

function onsetCoverage(onsets) {
  const byEpoch = new Map();
  for (const onset of onsets ?? []) {
    const epoch = onset.epoch ?? null;
    if (!byEpoch.has(epoch)) byEpoch.set(epoch, new Map());
    const atTime = byEpoch.get(epoch);
    const current = atTime.get(onset.time);
    atTime.set(onset.time, Math.max(current ?? -Infinity, onset.end));
  }
  const windows = [];
  for (const [epoch, atTime] of byEpoch) {
    const points = [...atTime].map(([time, end]) => ({ time:Number(time), end:Number(end) }))
      .sort((a, b) => a.time - b.time);
    const deltas = points.slice(1).map((point, index) => point.time - points[index].time)
      .filter((delta) => Number.isFinite(delta) && delta > 0).sort((a, b) => a - b);
    const middle = Math.floor(deltas.length / 2);
    const cadence = deltas.length === 0 ? Infinity : deltas.length % 2
      ? deltas[middle] : (deltas[middle - 1] + deltas[middle]) / 2;
    const maxGap = cadence * 3;
    let start = null;
    let end = null;
    for (let index = 0; index < points.length; index += 1) {
      const point = points[index];
      if (start === null) { start = point.time; end = point.end; continue; }
      if (point.time - points[index - 1].time > maxGap) {
        windows.push({ epoch, start, end });
        start = point.time;
      }
      end = Math.max(end, point.end);
    }
    if (start !== null) windows.push({ epoch, start, end });
  }
  return {
    windows,
    contains(time, epoch) {
      return windows.some((window) => window.epoch === (epoch ?? null) && time >= window.start && time <= window.end);
    },
  };
}

export function syncWindow(onsets, presented) {
  if (!onsets?.length || !presented?.length) return { windowStart:null, windowEnd:null, empty:true };
  const firstOnset = Math.min(...onsets.map((row) => row.time));
  const maxRetainedDuration = Math.max(...onsets.map((row) => row.end - row.time));
  const windowStart = Math.max(presented[0].audibleTime, firstOnset + maxRetainedDuration);
  const windowEnd = Math.min(presented.at(-1).audibleTime, Math.max(...onsets.map((row) => row.end)));
  return { windowStart, windowEnd, empty:windowStart > windowEnd };
}

export function attributeSync(onsets, presented, { earlyToleranceS, sampleRate, nominalMs: nominalOverride } = {}) {
  const toleranceS = Number.isFinite(earlyToleranceS) ? earlyToleranceS
    : Number.isFinite(sampleRate) && sampleRate > 0 ? RENDER_QUANTUM_FRAMES / sampleRate : 0.002;
  const window = syncWindow(onsets ?? [], presented ?? []);
  const coverage = onsetCoverage(onsets ?? []);
  const { windowStart, windowEnd, empty } = window;
  const inWindow = (time) => !empty && time >= windowStart && time <= windowEnd;
  const onsetIndex = new Map();
  for (const onset of onsets ?? []) {
    const epoch = onset.epoch ?? null;
    if (!onsetIndex.has(epoch)) onsetIndex.set(epoch, new Map());
    const byKey = onsetIndex.get(epoch);
    const key = rangeKey(onset);
    if (!byKey.has(key)) byKey.set(key, new Map());
    const byTime = byKey.get(key);
    if (!byTime.has(onset.time)) byTime.set(onset.time, onset);
  }
  const sortedOnsets = new Map();
  for (const [epoch, byKey] of onsetIndex) {
    sortedOnsets.set(epoch, new Map([...byKey].map(([key, byTime]) =>
      [key, [...byTime.values()].sort((a, b) => a.time - b.time)])));
  }
  const candidateFor = (row, key) => {
    const candidates = sortedOnsets.get(row.epoch ?? null)?.get(key) ?? [];
    const limit = row.audibleTime + toleranceS;
    let low = 0;
    let high = candidates.length;
    while (low < high) {
      const middle = (low + high) >>> 1;
      if (candidates[middle].time <= limit) low = middle + 1;
      else high = middle;
    }
    return low > 0 ? candidates[low - 1] : null;
  };
  const frameRanges = new Map((presented ?? []).map((row) => [row, activeRanges(row.activeKey)]));
  const windowOnsets = (onsets ?? []).filter((row) => inWindow(row.time));
  const windowOnsetSet = new Set(windowOnsets);
  const evaluated = (presented ?? []).map((row, index) => ({ row, index }))
    .filter(({ row }) => inWindow(row.audibleTime) && coverage.contains(row.audibleTime, row.epoch));
  let earlyFlashCount = 0, replayedFlashCount = 0, framePairs = 0, withinToleranceCount = 0, maxLeadMs = null;
  const earlyFlashes = [];
  for (const { row, index } of evaluated) {
    for (const key of frameRanges.get(row)) {
      framePairs += 1;
      const candidate = candidateFor(row, key);
      if (!candidate) {
        earlyFlashCount += 1;
        earlyFlashes.push({ frameIndex:index, frameMs:row.frameMs, audibleTime:row.audibleTime, epoch:row.epoch ?? null, range:key });
      } else if (candidate.time > row.audibleTime) {
        const leadMs = (candidate.time - row.audibleTime) * 1000;
        withinToleranceCount += 1;
        maxLeadMs = Math.max(maxLeadMs ?? 0, leadMs);
      } else if (candidate.end <= row.audibleTime) replayedFlashCount += 1;
    }
  }
  const positiveDeltas = [];
  for (let index = 1; index < (presented ?? []).length; index += 1) {
    const delta = Number(presented[index].frameMs) - Number(presented[index - 1].frameMs);
    if (Number.isFinite(delta) && delta > 0) positiveDeltas.push(delta);
  }
  const nominalCandidates = positiveDeltas.sort((a, b) => a - b);
  const middle = Math.floor(nominalCandidates.length / 2);
  const medianNominal = nominalCandidates.length === 0 ? 1000 / 60
    : nominalCandidates.length % 2 === 1 ? nominalCandidates[middle]
      : (nominalCandidates[middle - 1] + nominalCandidates[middle]) / 2;
  const nominal = Number.isFinite(nominalOverride) && nominalOverride > 0 ? nominalOverride : medianNominal;
  const sampled = new Map();
  const sampleKeys = new Set();
  let duplicateSamples = 0;
  let droppedFrameSamples = 0;
  let droppedFrames = 0;
  for (let index = 0; index < (presented ?? []).length - 1; index += 1) {
    const row = presented[index];
    for (const key of frameRanges.get(row)) {
      const onset = candidateFor(row, key);
      if (!onset || !windowOnsetSet.has(onset) || sampled.has(onset) || row.audibleTime < onset.time - toleranceS) continue;
      const sampleKey = `${onset.time}|${onset.epoch ?? ''}|${index}`;
      sampled.set(onset, null);
      if (sampleKeys.has(sampleKey)) {
        duplicateSamples += 1;
        continue;
      }
      sampleKeys.add(sampleKey);
      const next = presented[index + 1];
      const value = next.frameMs - (row.targetMs + (onset.time - row.audibleTime) * 1000);
      const previous = presented[index - 1];
      const droppedBefore = previous ? Math.max(0, Math.round((row.frameMs - previous.frameMs) / nominal) - 1) : 0;
      const droppedAfter = Math.max(0, Math.round((next.frameMs - row.frameMs) / nominal) - 1);
      const sampleDroppedFrames = droppedBefore + droppedAfter;
      sampled.set(onset, {
        time: onset.time,
        epoch: onset.epoch ?? null,
        frameIndex: index,
        value,
        droppedFrames: sampleDroppedFrames,
        onset: { time:onset.time, end:onset.end, from:onset.from, to:onset.to, epoch:onset.epoch ?? null, receivedMs:onset.receivedMs },
        frame: { frameMs:row.frameMs, targetMs:row.targetMs, audibleTime:row.audibleTime, epoch:row.epoch ?? null, activeKey:row.activeKey },
        previousFrameMs: previous?.frameMs ?? null,
        nextFrameMs: next.frameMs,
      });
      if (sampleDroppedFrames >= 1) droppedFrameSamples += 1;
      droppedFrames += sampleDroppedFrames;
    }
  }
  const samples = windowOnsets.map((onset) => sampled.get(onset)).filter(Boolean);
  const sync = samples.map((sample) => sample.value);
  return { sync, earlyFlashCount, earlyFlashes, earlyFlash:{withinToleranceCount,maxLeadMs}, earlyToleranceS:toleranceS, replayedFlashCount, framePairs, windowOnsets:windowOnsets.length,
    excludedFrames:(presented ?? []).length - evaluated.length, coverageWindows:coverage.windows.length, windowStart, windowEnd,
    samples, duplicateSamples, droppedFrameSamples, droppedFrames };
}

export function lateActiveMismatchDetails(onsets, presented, stalls) {
  const window = syncWindow(onsets ?? [], presented ?? []);
  const coverage = onsetCoverage(onsets ?? []);
  if (window.empty) return [];
  const audits = [];
  for (const stall of stalls ?? []) {
    const row = (presented ?? []).find((item) => item.frameMs >= stall);
    if (!row || row.audibleTime < window.windowStart || row.audibleTime > window.windowEnd) continue;
    if (!coverage.contains(row.audibleTime, row.epoch)) continue;
    const overlappingOnsets = (onsets ?? []).filter((onset) => sameEpoch(onset, row) &&
      onset.time <= row.audibleTime && row.audibleTime < onset.end);
    const receiptCutoffMs = Number.isFinite(row.executionMs) ? row.executionMs : row.frameMs;
    const eligibleOnsets = overlappingOnsets.filter((onset) =>
      !Number.isFinite(onset.receivedMs) || onset.receivedMs <= receiptCutoffMs);
    const expected = new Set(eligibleOnsets.map(rangeKey));
    const actual = activeRanges(row.activeKey);
    const expectedRanges = [...expected].sort();
    const actualRanges = [...actual].sort();
    const missingRanges = expectedRanges.filter((key) => !actual.has(key));
    const extraRanges = actualRanges.filter((key) => !expected.has(key));
    audits.push({
      stallFrameMs:stall,
      frameMs:row.frameMs,
      executionMs:receiptCutoffMs,
      audibleTime:row.audibleTime,
      epoch:row.epoch ?? null,
      activeKey:row.activeKey,
      expectedRanges,
      actualRanges,
      missingRanges,
      extraRanges,
      eligibleOnsets:eligibleOnsets.map(({ time, end, from, to, epoch, receivedMs }) => ({ time, end, from, to, epoch:epoch ?? null, receivedMs })),
      overlappingOnsets:overlappingOnsets.map(({ time, end, from, to, epoch, receivedMs }) => ({
        time, end, from, to, epoch:epoch ?? null, receivedMs,
        eligible:!Number.isFinite(receivedMs) || receivedMs <= receiptCutoffMs,
      })),
      mismatch:expected.size !== actual.size || missingRanges.length > 0,
    });
  }
  return audits;
}

export function lateActiveMismatches(onsets, presented, stalls) {
  return lateActiveMismatchDetails(onsets, presented, stalls).filter((audit) => audit.mismatch).length;
}

export function beatResidualMs(row, transport) {
  if (!transport?.running || row?.epoch !== transport.epoch || !Number.isFinite(row?.beatCycle)) return null;
  const bpm = Number(transport.bpm);
  const beatsPerCycle = Number(transport.beats_per_cycle);
  const sampleTime = Number(transport.sample_time);
  const denominator = Number(transport.cycle?.[1]);
  if (![bpm, beatsPerCycle, sampleTime, denominator].every(Number.isFinite) || bpm <= 0 || beatsPerCycle <= 0 || denominator === 0) return null;
  const cycle = Number(transport.cycle[0]) / denominator;
  const expected = cycle + (row.audibleTime - sampleTime) * bpm / 60 / beatsPerCycle;
  return (row.beatCycle - expected) * 60 * beatsPerCycle / bpm * 1000;
}

export function classifyStallSamples(samples, presented, windows, onsets = (samples ?? []).map((sample) => sample.onset), earlyFlashes = []) {
  const prepared = (windows ?? []).map((window) => {
    const firstFrameIndex = (presented ?? []).findIndex((row) => row.frameMs >= window.startMs);
    const first = firstFrameIndex < 0 ? null : presented[firstFrameIndex];
    return { ...window, firstFrameIndex, firstFrameMs:first?.frameMs ?? null };
  });
  const matchingBySample = new Map();
  const classified = (samples ?? []).map((sample) => {
    const onsetPageMs = sample.frame.targetMs + (sample.time - sample.frame.audibleTime) * 1000;
    let a = false;
    let b = false;
    const matchingWindows = [];
    for (const window of prepared) {
      const first = window.firstFrameIndex;
      const conditionA = onsetPageMs >= window.startMs && onsetPageMs <= window.endMs;
      const conditionB = first >= 0 && (sample.frameIndex === first || sample.frameIndex + 1 === first);
      if (conditionA || conditionB) matchingWindows.push({ window, conditionA, conditionB });
      a ||= conditionA;
      b ||= conditionB;
    }
    const classifiedSample = { ...sample, onsetPageMs, stallClass:matchingWindows.length ? 'stall' : 'non-stall', stallConditions:{ a, b } };
    matchingBySample.set(classifiedSample, matchingWindows);
    return classifiedSample;
  });
  const auditedRows = lateActiveMismatchDetails(
    onsets, presented,
    prepared.filter((window) => window.firstFrameMs !== null).map((window) => window.startMs),
  );
  const auditByStart = new Map(auditedRows.map((row) => [row.stallFrameMs, row]));
  let stallEarlyCount = 0;
  const pageProxyEarly = [];
  const windowRows = prepared.map((window) => {
    const audit = auditByStart.get(window.startMs);
    const windowSamples = classified.filter((sample) => matchingBySample.get(sample).some((match) => match.window === window));
    const expectedRanges = new Set(audit?.expectedRanges ?? []);
    const expiredActive = audit?.actualRanges.some((key) => !expectedRanges.has(key) && (onsets ?? []).some((onset) =>
      sameEpoch(onset, audit) && rangeKey(onset) === key && onset.end <= audit.audibleTime)) ?? false;
    const firstShowingIndexes = new Set(windowSamples.map((sample) => sample.frameIndex));
    const relevantIndexes = new Set([window.firstFrameIndex, window.firstFrameIndex - 1, ...firstShowingIndexes]);
    const early = (earlyFlashes ?? []).filter((flash) => relevantIndexes.has(flash.frameIndex)).length;
    stallEarlyCount += early;
    const proxySamples = windowSamples.filter((sample) => sample.value < -THRESHOLDS.earlyFlashMs);
    pageProxyEarly.push(...proxySamples);
    return {
      index:window.index, startMs:window.startMs, endMs:window.endMs,
      firstFrameMs:window.firstFrameMs,
      firstFrameLagMs:window.firstFrameMs === null ? null : window.firstFrameMs - window.endMs,
      samples:windowSamples.length,
      activeSetMatch:audit ? !audit.mismatch : null,
      replayed:audit ? expiredActive : null,
      early,
      pageProxyEarly:proxySamples.length,
      beatResidualMs:window.beatResidualMs ?? null,
      audited:Boolean(audit),
    };
  });
  const stall = classified.filter((sample) => sample.stallClass === 'stall');
  const nonStall = classified.filter((sample) => sample.stallClass === 'non-stall');
  return {
    stall, nonStall, windows:windowRows,
    earlyCount:stallEarlyCount, pageProxyEarly, pageProxyEarlyCount:pageProxyEarly.length,
    counts:{
      a:classified.filter((sample) => sample.stallConditions.a).length,
      b:classified.filter((sample) => sample.stallConditions.b).length,
      both:classified.filter((sample) => sample.stallConditions.a && sample.stallConditions.b).length,
    },
  };
}

export function tickStarvationEvidence(phaseRows, { lookaheadMs = 120, longSpanMs = 50 } = {}) {
  const ticks = (phaseRows ?? []).filter((row) => row[0] === 6)
    .map((row) => Number(row[1])).filter(Number.isFinite).sort((a, b) => a - b);
  const gaps = [];
  for (let index = 1; index < ticks.length; index += 1) {
    const durationMs = ticks[index] - ticks[index - 1];
    if (durationMs > lookaheadMs) gaps.push({ startMs:ticks[index - 1], endMs:ticks[index], durationMs });
  }
  const longSpans = (phaseRows ?? []).flatMap((row) => {
    const startMs = Number(row[1]);
    const durationMs = row.slice(2).reduce((total, value) => total + Number(value || 0), 0);
    return Number.isFinite(startMs) && durationMs > longSpanMs ? [{ phase:row[0], startMs, durationMs }] : [];
  });
  const overlaps = gaps.flatMap((gap) => longSpans
    .filter((span) => span.startMs <= gap.endMs && span.startMs + span.durationMs >= gap.startMs)
    .map((span) => ({ ...gap, span })));
  return {
    tickStartCount:ticks.length,
    tickStartGapsOverLookahead:gaps.length,
    maxTickStartGapMs:gaps.reduce((max, gap) => Math.max(max, gap.durationMs), 0),
    mainThreadSpansOverThreshold:longSpans.length,
    starvationOverlaps:overlaps.length,
    overlaps,
  };
}

export function rankSelfTime(cpuProfile, top = 10) {
  const nodes = new Map((cpuProfile?.nodes ?? []).map((node) => [node.id, node]));
  const totals = new Map();
  let total = 0;
  for (let i = 0; i < (cpuProfile?.samples ?? []).length; i += 1) {
    const delta = Number(cpuProfile.timeDeltas?.[i] ?? 0) / 1000;
    const frame = nodes.get(cpuProfile.samples[i])?.callFrame;
    if (!frame || !Number.isFinite(delta) || delta < 0) continue;
    const key = JSON.stringify([frame.functionName ?? '(anonymous)', frame.url ?? '', frame.lineNumber ?? 0]);
    totals.set(key, (totals.get(key) ?? 0) + delta); total += delta;
  }
  return [...totals].map(([key, selfMs]) => {
    const [functionName, url, line] = JSON.parse(key);
    return { functionName, url, line, selfMs, share:total ? selfMs / total : 0 };
  }).sort((a, b) => b.selfMs - a.selfMs).slice(0, top);
}

export function classifyWithControl(metricKey, path, productValue, controlValue, threshold) {
  const over = (value) => !Number.isFinite(value) || (path === 'min' ? value < threshold : value > threshold);
  if (!over(productValue)) return 'pass';
  if (!Number.isFinite(controlValue)) return 'fail';
  return over(controlValue) ? 'limitation' : 'fail';
}

export function evaluate(summary, thresholds = THRESHOLDS) {
  const failures = [];
  const limitations = [];
  const metrics = summary?.metrics ?? {};
  for (const [key, path] of [['inputLatencyMs','p95'],['inputLatencyMs','p99'],['animationWorkMs','p50'],['animationWorkMs','p95'],['animationWorkMs','p99'],['textWorkMs','p95'],['frameIntervalMs','p95'],['frameIntervalMs','p99']]) if (!Number.isFinite(metrics[key]?.[path])) failures.push(`${key}.${path} unavailable`);
  if (!Number.isFinite(metrics.editKeyCount)) failures.push('editing keystroke count unavailable');
  else if (metrics.editKeyCount < 500 && !(summary?.browser === 'webkit' && summary?.control?.mode === 'headless')) failures.push(`editing keystrokes=${metrics.editKeyCount}, expected at least 500`);
  if (!Number.isFinite(metrics.editPairedKeyCount)) failures.push('paired editing sample count unavailable');
  else if (metrics.editPairedKeyCount <= 0) failures.push('paired editing samples=0, expected at least 1');
  if (Number.isFinite(metrics.editKeyCount) && Number.isFinite(metrics.editUnpairedKeys) && metrics.editKeyCount > 0 && metrics.editUnpairedKeys / metrics.editKeyCount > 0.1) failures.push(`unpaired editing keys=${metrics.editUnpairedKeys}/${metrics.editKeyCount}, expected at most 10%`);
  if (metrics.audioRunning === true && (!Number.isFinite(metrics.onsetCount) || metrics.onsetCount === 0)) failures.push('no playing onset telemetry; active audio/visual workload was not observed');
  if (summary.silentAudioRequired === true && !summary.controlSink) failures.push('silent-sink: control report unavailable');
  if (summary.silentAudioRequired === true && !summary.workloadSink) failures.push('silent-sink: workload report unavailable');
  for (const [label, sink] of [['control',summary.controlSink],['workload',summary.workloadSink]]) {
    if (!sink) continue;
    if (sink.installed !== true || sink.installedBeforeFirstConnect !== true) failures.push('silent-sink: not installed before first destination connection');
    if (sink.directDestinationConnections !== 0) failures.push(`silent-sink: direct destination connections=${sink.directDestinationConnections}`);
    if (sink.post?.peak !== 0) failures.push(`silent-sink: post-sink peak=${sink.post?.peak}`);
    if ((sink.violations ?? []).length) failures.push(`silent-sink: ${sink.violations.join('; ')}`);
    if (label === 'control' && (!(sink.pre?.onsetCount >= 1) || !(sink.pre?.peakDbfs > -60))) failures.push(`control: harness failure onsets=${sink.pre?.onsetCount ?? 'unavailable'} peak=${sink.pre?.peakDbfs ?? 'unavailable'} dBFS`);
  }
  const workloadSink = summary.workloadSink ?? summary.controlSink;
  if (workloadSink && !Number.isFinite(workloadSink.pre?.peakDbfs)) failures.push('pre-sink peak unavailable');
  else if (workloadSink && workloadSink.pre.peakDbfs > -1) failures.push(`pre-sink peak ${workloadSink.pre.peakDbfs} dBFS exceeds -1`);
  if (summary.hushQuiet === false) failures.push('control: hush did not silence control');
  if (summary.workloadOnsetCount !== undefined && summary.workloadOnsetCount <= 0) failures.push('no playing onset telemetry; active audio/visual workload was not observed');
  const limit = (key, p, max) => {
    const v = metrics[key]?.[`p${p}`];
    if (Number.isFinite(v) && v > max) failures.push(`${key}.p${p}=${v} exceeds ${max}`);
  };
  const classifyLimit = (key, p, max) => {
    const v = metrics[key]?.[`p${p}`];
    if (summary?.browser === 'webkit' && summary?.control?.mode === 'headless' && ['inputLatencyMs','frameIntervalMs'].includes(key)) {
      const classification = classifyWithControl(key, `p${p}`, v, summary.control[key]?.[`p${p}`], max);
      if (classification === 'limitation') limitations.push(`headless WebKit ${key}.p${p} product=${v}ms control=${summary.control[key][`p${p}`]}ms threshold=${max}ms`);
      else if (classification === 'fail') failures.push(`${key}.p${p}=${v} exceeds ${max}`);
    } else limit(key, p, max);
  };
  if (summary?.browser === 'webkit' && summary?.control?.mode === 'headless') {
    const countClass = classifyWithControl('editKeyCount','min',metrics.editKeyCount,summary.control.editKeyCount,500);
    if(countClass==='limitation')limitations.push(`headless WebKit editKeyCount product=${metrics.editKeyCount} control=${summary.control.editKeyCount} threshold=500`);
    else if(countClass==='fail'&&metrics.editKeyCount<500)failures.push(`editing keystrokes=${metrics.editKeyCount}, expected at least 500`);
  }
  classifyLimit('inputLatencyMs', 95, thresholds.inputP95Ms);
  classifyLimit('inputLatencyMs', 99, thresholds.inputP99Ms);
  limit('animationWorkMs', 50, thresholds.animationWorkP50Ms);
  limit('animationWorkMs', 95, thresholds.animationWorkP95Ms);
  limit('animationWorkMs', 99, thresholds.animationWorkP99Ms);
  limit('textWorkMs', 95, thresholds.textWorkP95Ms);
  classifyLimit('frameIntervalMs', 95, thresholds.frameIntervalP95Ms);
  classifyLimit('frameIntervalMs', 99, thresholds.frameIntervalP99Ms);
  if (metrics.syncProvenance === 'measured') {
    if(!Number.isFinite(metrics.syncAbsMs?.p95)||!Number.isFinite(metrics.syncAbsMs?.p99)) failures.push('measured sync samples unavailable');
    if(!Number.isFinite(metrics.audioSampleRate)||metrics.audioSampleRate<=0) failures.push('audio sample rate unavailable');
    limit('syncAbsMs', 95, thresholds.syncAbsP95Ms);
    limit('syncAbsMs', 99, thresholds.syncAbsP99Ms);
    if ((metrics.earlyFlashCount ?? 0) > 0) failures.push(`early flashes=${metrics.earlyFlashCount}`);
    if (metrics.stallWindowSync && (metrics.stallWindowSync.earlyCount ?? 0) > 0) failures.push(`stall-window early flashes=${metrics.stallWindowSync.earlyCount}`);
    if (metrics.stallWindowsInjected !== undefined && metrics.stallWindowsInjected !== (metrics.stallWindows ?? []).length) failures.push(`stall windows injected=${metrics.stallWindowsInjected}, recorded=${(metrics.stallWindows ?? []).length}`);
    if ((metrics.stallWindows ?? []).some((window) => window.endMs - window.startMs < 250)) failures.push('stall window shorter than 250 ms');
    if (metrics.stallWindowsInjected > 0 && !(metrics.stallWindows ?? []).some((window) => window.audited)) failures.push('stall windows were not audited');
    const activeSetMismatches = (metrics.stallWindows ?? []).filter((window) => window.audited && window.activeSetMatch === false).length;
    if (activeSetMismatches > 0) failures.push(`stall-window active-set mismatches=${activeSetMismatches}`);
    if ((metrics.stallWindows ?? []).some((window) => window.audited && window.replayed === true)) failures.push('expired highlights replayed after stall');
    if ((metrics.stallWindows ?? []).some((window) => window.audited && Number.isFinite(window.beatResidualMs) && Math.abs(window.beatResidualMs) > thresholds.lateBeatDriftMs)) failures.push('stall-frame beat residual exceeded 1 ms');
  }
  if ((metrics.ledgerMaxBytes ?? 0) > thresholds.ledgerMiB * 1024 * 1024) failures.push('resource ledger exceeded 96 MiB');
  if (Number.isFinite(metrics.heapGrowthBytes) && metrics.heapGrowthBytes > thresholds.heapGrowthMiB * 1024 * 1024) failures.push('heap growth exceeded 8 MiB');
  if (metrics.audioRunning === true && !Number.isFinite(metrics.beatDriftMs)) failures.push('beat drift unavailable');
  if (Number.isFinite(metrics.beatDriftMs) && Math.abs(metrics.beatDriftMs) > thresholds.lateBeatDriftMs) failures.push('beat drift exceeded 1 ms');
  if ((metrics.replayedFlashCount ?? 0) > 0) failures.push('expired highlights replayed after stall');
  if ((metrics.lateActiveMismatchCount ?? 0) > 0) failures.push(`post-stall active-set mismatches=${metrics.lateActiveMismatchCount}`);
  if (metrics.ledgerAfterDisposeBytes !== undefined && metrics.ledgerAfterDisposeBytes !== 0) failures.push(`ledger after dispose=${metrics.ledgerAfterDisposeBytes}`);
  const targets = {
    textWorkP95Met: Number.isFinite(metrics.textWorkMs?.p95) ? metrics.textWorkMs.p95 <= TARGETS.textWorkP95Ms : null,
    animationWorkP50Met: Number.isFinite(metrics.animationWorkMs?.p50) ? metrics.animationWorkMs.p50 <= TARGETS.animationWorkP50Ms : null,
  };
  return { pass: failures.length === 0, failures, limitations, targets };
}

const cell = (value) => value === null || value === undefined ? 'unavailable' : String(value);
export function renderEvidence(summary) {
  const browsers = summary.browsers ?? [];
  const rows = [];
  const wasm = summary.wasm ?? summary.environment?.wasm;
  const wasmLine = wasm
    ? `WASM: ${cell(wasm.profile)}; ${cell(wasm.bytes)} bytes; SHA-256 ${cell(wasm.sha256)}.`
    : 'WASM: not recorded.';
  for (const browser of browsers) {
    const m = browser.metrics ?? {};
    if(browser.control?.mode==='headless')rows.push(`| ${browser.name} | Headless control p95/p99 frame interval (ms) | ${browser.control.frameIntervalMs?.p95} / ${browser.control.frameIntervalMs?.p99} | diagnostic control page |`,`| ${browser.name} | Headless control p95/p99 input latency (ms) | ${browser.control.inputLatencyMs?.p95} / ${browser.control.inputLatencyMs?.p99} | diagnostic control page |`,`| ${browser.name} | Headless control editing keys / frames | ${browser.control.editKeyCount} / ${browser.control.frames} | same 60 s key pacing |`);
    for (const [name, value, threshold] of [
      ['Input latency p95 (ms)', m.inputLatencyMs?.p95, '<= 50'],
      ['Input latency p99 (ms)', m.inputLatencyMs?.p99, '<= 100'],
      ['Input samples (paired / expired)', m.inputSamplesUsed == null ? null : `${m.inputSamplesUsed} / ${m.inputKeysExpired ?? 0}`, 'expired keys are outside the retained frame ring'],
      ['Editing keystrokes / paired', m.editKeyCount == null ? null : `${m.editKeyCount} / ${m.editPairedKeyCount ?? 0}`, '>= 500 keys; paired > 0; unpaired <= 10%'],
      ['Audio control / run start', m.controlOnsetCount == null ? null : `${m.controlOnsetCount} onsets / ${m.controlStartMethod ?? 'unknown'}`, m.onsetAttribution ?? 'attribution unavailable'],
      ['Silent sink post-sink peak', m.sink?.workload?.post?.peak ?? m.sink?.control?.post?.peak, '== 0'],
      ['Direct destination connections', m.sink?.workload?.directDestinationConnections ?? m.sink?.control?.directDestinationConnections, '0'],
      ['Pre-sink peak (dBFS)', m.sink?.workload?.pre?.peakDbfs ?? m.sink?.control?.pre?.peakDbfs, '<= -1'],
      ['Pre-sink RMS (dBFS)', m.sink?.workload?.pre?.rmsDbfs ?? m.sink?.control?.pre?.rmsDbfs, 'reported'],
      ['Pre-sink onsets (count)', m.sinkOnsetsTotal, 'reported'],
      ['Control onsets / peak dBFS', m.controlSink?.pre ? `${m.controlSink.pre.onsetCount} / ${m.controlSink.pre.peakDbfs}` : null, '>= 1 / > -60'],
      ['Animation frame work p50 (ms)', m.animationWorkMs?.p50, '<= 4 (target 1, recorded)'],
      ['Animation frame work p95 (ms)', m.animationWorkMs?.p95, '<= 8'],
      ['Animation frame work p99 (ms)', m.animationWorkMs?.p99, '<= 16.7'],
      ['Text-dirty frame work p95 (ms)', m.textWorkMs?.p95, '<= 8 (target 4, recorded)'],
      ['Frame interval p95 (ms)', m.frameIntervalMs?.p95, '<= 20'],
      ['Frame interval p99 (ms)', m.frameIntervalMs?.p99, '<= 50'],
      ['Audio sample rate / early tolerance (Hz / ms)', m.audioSampleRate == null ? null : `${m.audioSampleRate} / ${m.earlyToleranceMs}`, '128-frame render quantum'],
      ['Audio-domain early flashes / tolerance-band pairs', m.earlyFlashCount == null ? null : `${m.earlyFlashCount} / ${m.earlyFlash?.withinToleranceCount ?? 0}`, '0 early; tolerance-band count informational'],
      ['Page-time proxy early flashes', m.syncPageProxyEarlyCount, 'informational only; not gated'],
      ['A/V model absolute error p95/p99 (non-stall samples, ms)', m.syncAbsMs == null ? null : `${m.syncAbsMs.p95} / ${m.syncAbsMs.p99}`, 'measured only; <= 33.4 / 50'],
      ['A/V model all-sample absolute error p95/p99 (ms)', m.syncAbsMsAll == null ? null : `${m.syncAbsMsAll.p95} / ${m.syncAbsMsAll.p99}`, 'informational; includes stall-window samples'],
      ['A/V stall-window sync samples by condition (a / b / both)', m.stallWindowSync?.counts == null ? null : `${m.stallWindowSync.counts.a} / ${m.stallWindowSync.counts.b} / ${m.stallWindowSync.counts.both}`, 'informational; classified by recorded stall windows'],
      ['A/V stall-window sample count / p50 / p95 / max absolute error (ms)', m.stallWindowSync == null ? null : `${m.stallWindowSync.count} / ${m.stallWindowSync.p50} / ${m.stallWindowSync.p95} / ${m.stallWindowSync.max}`, 'informational; recovery gates apply'],
      ['Stall windows injected / recorded / audited', m.stallWindowsInjected == null ? null : `${m.stallWindowsInjected} / ${(m.stallWindows ?? []).length} / ${(m.stallWindows ?? []).filter((window) => window.audited).length}`, 'equal counts; all windows >= 250 ms; >= 1 audited'],
      ['Stall-window early flashes / page-time proxy', m.stallWindowSync?.earlyCount == null ? null : `${m.stallWindowSync.earlyCount} / ${m.stallWindowSync.pageProxyEarlyCount ?? 0}`, 'audio-domain gate 0; page-time proxy informational'],
      ['F beat residual max (ms)', m.beatResidual?.maxAbsStallFrameMs, '<= 1'],
      ['Frame interval exclusions (stall / non-stall)', m.frameIntervalExcluded == null ? null : `${m.frameIntervalExcluded.stall} / ${m.frameIntervalExcluded.nonStall}`, 'informational; intervals > 200 ms excluded'],
      ['A/V sync dropped-frame samples', m.syncDroppedFrameSamples, 'reported separately; sync gate covers non-stall samples'],
      ['A/V sync duplicates folded', m.syncDuplicateSamples, 'one sample per onset time, epoch and presented frame'],
      ['A/V model absolute error p95/p99 without dropped frames (informational)', m.syncAbsMsNoDrop == null ? null : `${m.syncAbsMsNoDrop.p95} / ${m.syncAbsMsNoDrop.p99}`, 'informational; not gated'],
      ['Early flashes', m.earlyFlashCount, '0; no frame more than one render quantum before onset'],
      ['A/V window start / end (s)', m.syncWindowStart == null ? null : `${m.syncWindowStart} / ${m.syncWindowEnd}`, 'intersection with onset eviction guard'],
      ['A/V window onsets / frame pairs / excluded frames', m.syncWindowOnsets == null ? null : `${m.syncWindowOnsets} / ${m.syncFramePairs} / ${m.syncExcludedFrames}`, 'excluded rows remain in raw JSONL'],
      ['Post-stall active-set mismatches', m.lateActiveMismatchCount, '0'],
      ['A/V model absolute error p95 (non-stall samples, ms)', m.syncAbsMs?.p95, 'measured only; <= 33.4'],
      ['Resource ledger peak (MiB)', m.ledgerMaxBytes == null ? null : (m.ledgerMaxBytes / 1048576).toFixed(2), '<= 96'],
      ['JS heap growth (MiB)', m.heapGrowthBytes == null ? null : (m.heapGrowthBytes / 1048576).toFixed(2), '<= 8'],
      ['Beat drift (ms)', m.beatDriftMs, '<= 1'],
      ['Replayed flashes', m.replayedFlashCount, '0'],
      ['Ledger after dispose (bytes)', m.ledgerAfterDisposeBytes, '0'],
    ]) rows.push(`| ${browser.name} | ${name} | ${cell(value)} | ${threshold} |`);
  }
  const stallRows = browsers.flatMap((browser) => (browser.metrics?.stallWindows ?? []).map((window) =>
    `| ${browser.name} | ${window.index} | ${window.startMs} / ${window.endMs} | ${window.firstFrameMs ?? 'unavailable'} | ${window.samples} | ${window.activeSetMatch ?? 'unavailable'} | ${window.replayed ?? 'unavailable'} | ${window.early} | ${window.pageProxyEarly ?? 0} | ${window.beatResidualMs ?? 'unavailable'} | ${window.audited} |`));
  const sampleNotes = browsers.filter((browser) => browser.sampleDownsampling)
    .map((browser) => `${browser.name}: ${browser.sampleDownsampling.frame}; ${browser.sampleDownsampling.presented}; ${browser.sampleDownsampling.onset}; aggregate metrics use full in-memory samples`).join('; ');
  const beatDrifts = browsers.map((browser) => `${browser.name} ${cell(browser.metrics?.beatDriftMs)} ms`).join('; ');
  const status = summary.pass ? 'PASS' : summary.blocked ? 'BLOCKED' : 'FAIL';
  const ascii = (s) => String(s).replace(/[^\x00-\x7F]/g, '?');
  return ascii(`### Run ${summary.runId}: ${status}\n\n` +
    `Commands: \`${summary.commands?.join('; ') ?? 'not recorded'}\`.\n\n` +
    `${wasmLine}\n\n` +
    '| Browser | Metric | Result | Threshold |\n|---|---|---:|---:|\n' + rows.join('\n') +
    `\n\nBehavior checks: ${browsers.map((b) => `${b.name} ${b.behavior?.passed ?? 0}/${b.behavior?.total ?? 0}`).join('; ')}. ` +
    `Failed checks: ${browsers.flatMap((b) => (b.checks ?? []).filter((c) => c.status !== 'limitation' && !c.pass).map((c) => `${b.name}:${c.id}`)).join(', ') || 'none'}. ` +
    `Measurement failures: ${browsers.flatMap((b) => (b.measurement?.failures ?? []).map((failure) => `${b.name}:${String(failure).split('\n')[0]}`)).join('; ') || 'none'}. ` +
    `Limitations: ${[...new Set(browsers.flatMap((b) => b.limitations ?? []))].join('; ') || 'none recorded'}.\n\n` +
    `### Stall-window sync classification (session 293, design 15.3.8.15)\n\n` +
    `Early flashes use the audio domain: a shown range is early only when its frame audibleTime precedes the matching onset by more than Q = 128 / sampleRate. The page-time proxy is informational. Sample rate and Q are recorded in the metric table.\n\n` +
    '| Browser | Window | Start / end (page ms) | F frame (page ms) | Samples | Active set matches | Replayed | Audio early | Page proxy early | F beat residual (ms) | Audited |\n|---|---:|---:|---:|---:|---|---|---:|---:|---:|---|\n' +
    stallRows.join('\n') + '\n\n' +
    `The transport publisher pairs a running sample's cycle with the host time of that cycle under the one-grid-period guard; the prior grid-quantized pairing could add up to 2.083 ms. Current final beat drift: ${beatDrifts}.\n\n` +
    `Serialized raw samples are downsampled for the committed evidence size cap (${sampleNotes || 'not applicable'}); thresholds, counts and aggregate metrics are computed from full in-memory samples.\n`);
}
