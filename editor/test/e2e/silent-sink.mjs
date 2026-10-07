export function dbfs(peak) { return peak === 0 ? Number.NEGATIVE_INFINITY : 20 * Math.log10(peak); }

export function blockRms(samples, sampleRate, blockMs = 10) {
  const size = Math.max(1, Math.round(sampleRate * blockMs / 1000));
  const values = [];
  for (let start = 0; start < samples.length; start += size) {
    let sum = 0;
    const end = Math.min(samples.length, start + size);
    for (let i = start; i < end; i += 1) sum += samples[i] * samples[i];
    values.push(Math.sqrt(sum / (end - start)));
  }
  return values;
}

export function detectOnsets(blockRmsDb, blockMs, { onDb = -40, quietDb = -50, quietMs = 50 } = {}) {
  const onsets = [];
  let quietBlocks = Math.ceil(quietMs / blockMs);
  for (let i = 0; i < blockRmsDb.length; i += 1) {
    if (blockRmsDb[i] <= quietDb) quietBlocks += 1;
    else if (blockRmsDb[i] >= onDb && quietBlocks * blockMs >= quietMs) {
      onsets.push(i);
      quietBlocks = 0;
    } else quietBlocks = 0;
  }
  return onsets;
}

export function silentSinkInit() {
  const w = window;
  if (w.__vactrSink) return;
  const state = { contexts: new Map(), wrappedDestinationConnections: 0, directDestinationConnections: 0,
    mediaElementsForcedMuted: 0, violations: [], suspendedOnViolation: false, installedBeforeFirstConnect: true };
  const originalConnect = AudioNode.prototype.connect;
  const originalDisconnect = AudioNode.prototype.disconnect;
  const originalPlay = HTMLMediaElement.prototype.play;
  const db = (value) => value === 0 ? -Infinity : 20 * Math.log10(value);
  const violate = (ctx, message) => {
    if (!state.violations.includes(message)) state.violations.push(message);
    state.suspendedOnViolation = true;
    void ctx.suspend().catch(() => {});
  };
  const sinkFor = (ctx) => {
    let sink = state.contexts.get(ctx);
    if (sink) return sink;
    const pre = ctx.createAnalyser(); pre.fftSize = 32768;
    const gain = new GainNode(ctx, { gain:0 });
    const post = ctx.createAnalyser(); post.fftSize = 32768;
    originalConnect.call(pre, gain); originalConnect.call(gain, post); originalConnect.call(post, ctx.destination);
    sink = { pre, gain, post, lastTime: ctx.currentTime, prePeak: 0, preSquares: 0, preSamples: 0, postPeak: 0, onsetTimes: [], quietBlocks: 5 };
    state.contexts.set(ctx, sink);
    const timer = setInterval(() => {
      if (ctx.state === 'closed') { clearInterval(timer); return; }
      const preData = new Float32Array(pre.fftSize); const postData = new Float32Array(post.fftSize);
      pre.getFloatTimeDomainData(preData); post.getFloatTimeDomainData(postData);
      const elapsed = Math.max(0, Math.round((ctx.currentTime - sink.lastTime) * ctx.sampleRate));
      const count = Math.min(elapsed, preData.length); const start = preData.length - count;
      for (let i = start; i < preData.length; i += 1) {
        const value = preData[i]; const abs = Math.abs(value);
        sink.prePeak = Math.max(sink.prePeak, abs);
        sink.preSquares += value * value; sink.preSamples += 1;
      }
      sink.lastTime = ctx.currentTime;
      if (count) {
        const blockSize = Math.max(1, Math.round(ctx.sampleRate * 0.01));
        for (let offset = start; offset + blockSize <= preData.length && offset < start + count; offset += blockSize) {
          let squares = 0; let peak = 0;
          for (let i = offset; i < offset + blockSize; i += 1) { const value = preData[i]; squares += value * value; peak = Math.max(peak, Math.abs(value)); }
          const rmsDb = db(Math.sqrt(squares / blockSize));
          if (rmsDb <= -50) sink.quietBlocks += 1;
          else if (rmsDb >= -40 && sink.quietBlocks >= 5 && peak > 0) {
            if (sink.onsetTimes.length < 4096) sink.onsetTimes.push(ctx.currentTime - (preData.length - (offset + blockSize / 2)) / ctx.sampleRate);
            sink.quietBlocks = 0;
          } else sink.quietBlocks = 0;
        }
      }
      for (const sample of postData) sink.postPeak = Math.max(sink.postPeak, Math.abs(sample));
      if (sink.postPeak > 0) violate(ctx, `post-sink peak=${sink.postPeak}`);
    }, 100);
    return sink;
  };
  AudioNode.prototype.connect = function(destination, ...args) {
    if (destination instanceof AudioDestinationNode && !(this.context instanceof OfflineAudioContext)) {
      state.wrappedDestinationConnections += 1;
      const sink = sinkFor(this.context);
      return originalConnect.call(this, sink.pre, ...args);
    }
    return originalConnect.call(this, destination, ...args);
  };
  AudioNode.prototype.disconnect = function(destination, ...args) {
    if (arguments.length === 0) return originalDisconnect.call(this);
    const sink = destination instanceof AudioDestinationNode ? state.contexts.get(this.context) : null;
    return originalDisconnect.call(this, sink ? sink.pre : destination, ...args);
  };
  HTMLMediaElement.prototype.play = function(...args) {
    if (!this.muted) { this.muted = true; state.mediaElementsForcedMuted += 1; }
    return originalPlay.apply(this, args);
  };
  w.__vactrSink = {
    now() { const ctx = state.contexts.keys().next().value; return ctx ? { ctxTime:ctx.currentTime, pageMs:performance.now(), sampleRate:ctx.sampleRate } : null; },
    report() {
      const sinks = [...state.contexts].map(([ctx, sink]) => ({ ctx, sink }));
      const prePeak = Math.max(0, ...sinks.map(({ sink }) => sink.prePeak));
      const preSquares = sinks.reduce((sum, { sink }) => sum + sink.preSquares, 0);
      const preSamples = sinks.reduce((sum, { sink }) => sum + sink.preSamples, 0);
      const postPeak = Math.max(0, ...sinks.map(({ sink }) => sink.postPeak));
      const onsets = sinks.flatMap(({ sink }) => sink.onsetTimes).sort((a,b) => a-b);
      return { installed:true, installedBeforeFirstConnect:state.installedBeforeFirstConnect, contexts:sinks.length,
        wrappedDestinationConnections:state.wrappedDestinationConnections, directDestinationConnections:state.directDestinationConnections,
        mediaElementsForcedMuted:state.mediaElementsForcedMuted, violations:[...new Set(state.violations)], suspendedOnViolation:state.suspendedOnViolation,
        pre:{ peak:prePeak, peakDbfs:db(prePeak), rms:preSamples ? Math.sqrt(preSquares/preSamples) : 0,
          rmsDbfs:db(preSamples ? Math.sqrt(preSquares/preSamples) : 0), onsetCount:onsets.length, onsetTimes:onsets.slice(0,4096) }, post:{peak:postPeak} };
    }
  };
}

export async function installSilentSink(context) { await context.addInitScript(silentSinkInit); }
export async function readSinkReport(page) { return page.evaluate(() => window.__vactrSink?.report() ?? null); }
