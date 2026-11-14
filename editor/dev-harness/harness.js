// Vactr dev harness: the TASK-008 real-worklet checks (design 12.8.11).
//
// Loads the host-wasm module into the main thread (wasm #1: evaluator and
// scheduler) and an AudioWorklet (wasm #2: DSP), drives them with Vactr
// source and with fault injection on the record path between the halves
// (hold, drop, replace, reorder: the host's `filter` hook), and reads the
// worklet's report counters. Each check is {id, pass, detail}; the page
// POSTs {checks, memoryStable, userAgent, ...} to /report when opened with
// ?auto=1 (run-headless.mjs). A context that never reaches `running` is
// reported as BLOCKED, never as a pass.

import { startHost } from '../worklet/host.js';

// Report indices (src/host/wasm/worklet_half.rs R_*).
const R = {
  NOW: 0, QUANTA: 1, LATE: 2, DROPPED: 3, STOLEN: 4, BYTES_Q: 5, BYTES_MAX: 6,
  BYTES_TOTAL: 7, HELD: 8, DEFERRED_QUANTA: 9, ACTIVE: 10, RETIRED: 11,
  CELL_RETIRED: 12, MEM_INIT: 13, MEM_NOW: 14, INBOX_REFUSED: 15, RING_FULL: 16,
  OUTBOX_DROPPED: 17, FAULT_ARENA: 18, FAULT_GRAPH: 19, FAULT_OVERFLOW: 20,
  FAULT_OTHER: 21, SOUNDING: 22, DROPOUTS: 23, RMS: 24, RMS_MIN: 25,
  SLICE_ACKS: 26, DEFERRED_ACKS: 27, STAGED_MAX: 28, INSTALLED: 29, RES_ID: 30,
  RES_STATE: 31, PROBES: 32, TAGGED_N: 64, TAGGED: 65, LOG_N: 97, LOG: 98,
};
const READ_LOG = 16;
// Record tags (src/host/wire.rs, src/dsp/ring.rs).
const T = { EVENT: 0x01, CELL_INIT: 0x11, CELL_BATCH: 0x12, CELL_RETIRE: 0x13, GRAPH: 0x16, SLICE: 0x18 };
const INSTALL_BYTES_PER_QUANTUM = 65536;
const ARENA = 16 << 20;
const SPARE_CELL = 890; // below the registry range, never used by the runtime here

const auto = new URLSearchParams(location.search).get('auto') === '1';
const checks = [];
const logLines = [];
let host = null;
let reads = [];
let readSeen = 0;

function log(line) {
  logLines.push(line);
  const el = document.getElementById('log');
  if (el) el.textContent += line + '\n';
  if (auto) fetch('/log', { method: 'POST', body: line }).catch(() => {});
}

function record(id, pass, detail) {
  checks.push({ id, pass: !!pass, detail });
  log(`${pass ? 'PASS' : 'FAIL'} ${id}: ${detail}`);
  const li = document.createElement('li');
  li.className = pass ? 'pass' : 'fail';
  li.textContent = `${pass ? 'PASS' : 'FAIL'} ${id}: ${detail}`;
  document.getElementById('checks').appendChild(li);
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const tagOf = (buf) => new Uint8Array(buf)[0];
const u32 = (buf, at) => new DataView(buf).getUint32(at, true);
const f32 = (buf, at) => new DataView(buf).getFloat32(at, true);
const near = (a, b) => Math.abs(a - b) < 1e-4;
const rep = () => host.report;

// Waits (wall clock) until `pred()` holds; false on timeout.
async function until(pred, seconds) {
  const end = performance.now() + seconds * 1000;
  while (performance.now() < end) {
    collectReads();
    if (pred()) return true;
    await sleep(5);
  }
  collectReads();
  return pred();
}

// Waits `seconds` of audio time (the worklet's clock).
async function audio(seconds) {
  const start = host.now;
  await until(() => host.now >= start + seconds, seconds * 4 + 2);
}

// A fresh report after now.
async function fresh() {
  const q = rep() ? rep()[R.QUANTA] : 0;
  await until(() => rep() && rep()[R.QUANTA] >= q + 8, 2);
  return rep();
}

// Merges the worklet's probed-read ring into `reads`.
function collectReads() {
  const r = rep();
  if (!r) return;
  const n = r[R.LOG_N];
  for (let i = Math.max(readSeen, n - READ_LOG); i < n; i += 1) {
    const b = R.LOG + 4 * (i % READ_LOG);
    reads.push({ t: r[b], cell: r[b + 1], value: r[b + 2], live: r[b + 3] === 1 });
  }
  readSeen = n;
}

function probe(k) {
  const r = rep();
  const b = R.PROBES + 8 * k;
  return {
    cell: r[b], kind: r[b + 1], epoch: r[b + 2], value: r[b + 3],
    reads: r[b + 4], uninit: r[b + 5], last: r[b + 6], lastEpoch: r[b + 7],
  };
}

function tagged() {
  const r = rep();
  const out = [];
  for (let i = 0; i < r[R.TAGGED_N]; i += 1) {
    const b = R.TAGGED + 4 * i;
    out.push({ slot: r[b], pitch: r[b + 1], seq: r[b + 2], released: r[b + 3] === 1 });
  }
  return out;
}

function sentSince(t0) {
  const out = [];
  for (let i = 0; i < 64; i += 1) {
    const e = host.sent(i);
    if (!e) break;
    if (e.time >= t0) out.push(e);
  }
  return out;
}

function consoleHas(mark, text) {
  return host.console.slice(mark).some((c) => c.line.includes(text));
}

function evalOk(src) {
  const mark = host.console.length;
  const n = host.eval(src);
  if (n > 0) log(`eval ${JSON.stringify(src)} -> ${n}: ${host.console.slice(mark).map((c) => c.line).join(' | ')}`);
  return n;
}

function sine(seconds, hz, rate = 48000) {
  const n = Math.round(seconds * rate);
  const f = new Float32Array(n);
  for (let i = 0; i < n; i += 1) f[i] = 0.3 * Math.sin((2 * Math.PI * hz * i) / rate);
  return f;
}

// A batch record with one more (cell, epoch, value) entry appended.
function appendEntry(buf, cell, epoch, value) {
  const src = new Uint8Array(buf);
  const out = new Uint8Array(src.length + 12);
  out.set(src);
  const v = new DataView(out.buffer);
  v.setUint32(5, v.getUint32(5, true) + 1, true);
  v.setUint32(src.length, cell, true);
  v.setUint32(src.length + 4, epoch, true);
  v.setFloat32(src.length + 8, value, true);
  return out.buffer;
}

// A browser `SampleBegin` record (src/dsp/ring.rs encode_sample_begin).
function sampleBegin(resource, gen, frames, channels, rate) {
  const b = new ArrayBuffer(18);
  const v = new DataView(b);
  v.setUint8(0, 0x1a);
  v.setUint32(1, resource, true);
  v.setUint32(5, gen, true);
  v.setUint32(9, frames, true);
  v.setUint8(13, channels);
  v.setUint32(14, rate, true);
  return b;
}

async function criterion8() {
  const gain = host.callStr('ctl_id', 'gain');
  host.call('main_probe_ctl', gain);

  // cell-first-before-ack: hold every CellInit, so no ack arrives.
  let init = null;
  host.filter = (b) => {
    if (tagOf(b) === T.CELL_INIT) {
      if (!init) init = b.slice(0);
      return 'hold';
    }
    return 'pass';
  };
  const t0 = host.now;
  evalOk('var g 0.5');
  evalOk('s [:analog :analog :analog :analog :analog :analog :analog :analog] > gain g > d1');
  await until(() => init !== null, 2);
  const cellG = init ? u32(init, 1) : -1;
  const epochG = init ? u32(init, 5) : -1;
  host.workletCall('worklet_probe_cell', 0, cellG);
  await audio(1.0);
  let r = await fresh();
  const heldSent = sentSince(t0);
  const consts = heldSent.filter((e) => e.kind === 1 && near(e.value, 0.5)).length;
  const cellsEarly = heldSent.filter((e) => e.kind === 2).length;
  const p0 = probe(0);
  const sounded = r[R.SOUNDING] > 0;
  host.filter = null;
  host.release();
  await until(() => sentSince(t0).some((e) => e.kind === 2) && probe(0).reads > 0, 3);
  const p1 = probe(0);
  record(
    'cell-first-before-ack',
    init && consts >= 2 && cellsEarly === 0 && p0.reads === 0 && p0.uninit === 0 && sounded
      && p1.reads > 0 && p1.uninit === 0 && near(p1.last, 0.5),
    `cell ${cellG} epoch ${epochG}; while CellInit held: ${consts} Const(0.5) events, ${cellsEarly} Cell events, `
      + `mirror reads ${p0.reads} (uninitialized ${p0.uninit}), sounding quanta ${r[R.SOUNDING]}; `
      + `after the ack: reads ${p1.reads}, uninitialized ${p1.uninit}, last read ${p1.last}`,
  );

  // cell-batch-next-voice
  reads = [];
  await audio(0.6);
  evalOk('upd g 0.8');
  await until(() => near(probe(0).value, 0.8), 2);
  const tb = rep()[R.NOW];
  await audio(1.0);
  collectReads();
  const before = reads.filter((x) => x.cell === cellG && x.t < tb);
  const after = reads.filter((x) => x.cell === cellG && x.t >= tb);
  record(
    'cell-batch-next-voice',
    before.length > 0 && before.every((x) => near(x.value, 0.5)) && after.length > 0
      && after.every((x) => near(x.value, 0.8)),
    `batch applied by ${tb.toFixed(3)} s; voice-start reads before: ${before.map((x) => x.value.toFixed(2))}, `
      + `after: ${after.map((x) => x.value.toFixed(2))}`,
  );

  // cell-delayed-hop: hold the batch across several voice starts.
  host.filter = (b) => (tagOf(b) === T.CELL_BATCH ? 'hold' : 'pass');
  const th = rep()[R.NOW];
  evalOk('upd g 0.3');
  await audio(1.0);
  collectReads();
  const hop = reads.filter((x) => x.cell === cellG && x.t >= th);
  const heldValue = probe(0).value;
  host.filter = null;
  host.release();
  await until(() => near(probe(0).value, 0.3), 2);
  const ta = rep()[R.NOW];
  await audio(0.8);
  collectReads();
  const landed = reads.filter((x) => x.cell === cellG && x.t >= ta);
  record(
    'cell-delayed-hop',
    hop.length >= 2 && hop.every((x) => near(x.value, 0.8)) && near(heldValue, 0.8)
      && landed.length > 0 && landed.every((x) => near(x.value, 0.3)),
    `while the batch was held: mirror ${heldValue}, voice-start reads ${hop.map((x) => x.value.toFixed(2))}; `
      + `after it landed (${ta.toFixed(3)} s): ${landed.map((x) => x.value.toFixed(2))}`,
  );

  // cell-init-replay: replay the captured CellInit (0.5) after newer batches.
  const tr = rep()[R.NOW];
  host.postRaw(init.slice(0));
  await audio(0.8);
  collectReads();
  const pr = probe(0);
  const afterReplay = reads.filter((x) => x.cell === cellG && x.t >= tr);
  record(
    'cell-init-replay',
    near(pr.value, 0.3) && pr.epoch === epochG && afterReplay.length > 0
      && afterReplay.every((x) => near(x.value, 0.3)),
    `replayed CellInit(cell ${cellG}, epoch ${epochG}, 0.5): mirror ${pr.value} epoch ${pr.epoch}, `
      + `reads after ${afterReplay.map((x) => x.value.toFixed(2))}`,
  );

  // cell-reuse: Live(1) -> Retiring -> Vacant -> Live(2) in the real worklet.
  host.workletCall('worklet_probe_cell', 1, SPARE_CELL);
  await fresh();
  host.call('harness_cell_init', SPARE_CELL, 1, 0.1);
  const live1 = await until(() => probe(1).kind === 1 && probe(1).epoch === 1 && near(probe(1).value, 0.1), 2);
  const retiredBefore = rep()[R.CELL_RETIRED];
  host.call('harness_cell_retire', SPARE_CELL, 1);
  const vacant = await until(() => probe(1).kind === 0 && rep()[R.CELL_RETIRED] > retiredBefore, 2);
  host.call('harness_cell_init', SPARE_CELL, 2, 0.2);
  const live2 = await until(() => probe(1).kind === 1 && probe(1).epoch === 2 && near(probe(1).value, 0.2), 2);
  record(
    'cell-reuse',
    live1 && vacant && live2,
    `cell ${SPARE_CELL}: Live(epoch 1)=${live1}, retired to Vacant with a CellRetired ack=${vacant}, `
      + `re-initialized Live(epoch 2, 0.2)=${live2}`,
  );

  // cell-stale-epoch: an entry with the retired epoch rides in a real batch.
  let injected = false;
  host.filter = (b) => {
    if (!injected && tagOf(b) === T.CELL_BATCH) {
      injected = true;
      return appendEntry(b, SPARE_CELL, 1, 9.9);
    }
    return 'pass';
  };
  evalOk('upd g 0.35');
  const applied = await until(() => near(probe(0).value, 0.35), 2);
  host.filter = null;
  await fresh();
  const ps = probe(1);
  record(
    'cell-stale-epoch',
    injected && applied && ps.kind === 1 && ps.epoch === 2 && near(ps.value, 0.2),
    `batch carrying (cell ${SPARE_CELL}, epoch 1, 9.9) applied its live entry (g -> 0.35: ${applied}); `
      + `cell ${SPARE_CELL} stays Live epoch ${ps.epoch} value ${ps.value}`,
  );

  // cell-stall-burst: nothing reaches the worklet while values keep changing.
  const seqs = new Set();
  let heldBatches = 0;
  host.filter = (b) => {
    if (tagOf(b) === T.CELL_BATCH) {
      seqs.add(u32(b, 1));
      heldBatches += 1;
    }
    return 'hold';
  };
  let maxFlight = 0;
  let maxPending = 0;
  for (let k = 1; k <= 6; k += 1) {
    evalOk(`upd g 0.4${k}`);
    for (let i = 0; i < 5; i += 1) {
      maxFlight = Math.max(maxFlight, host.call('main_stat', 1));
      maxPending = Math.max(maxPending, host.call('main_stat', 2));
      await sleep(20);
    }
  }
  host.filter = null;
  host.release();
  const converged = await until(() => near(probe(0).value, 0.46), 3);
  record(
    'cell-stall-burst',
    maxFlight <= 1 && maxPending <= 1 && seqs.size <= 1 && converged,
    `6 updates while stalled: batches in flight <= ${maxFlight}, pending <= ${maxPending}, `
      + `${heldBatches} held batch records with ${seqs.size} distinct seq; converged to 0.46 on resume: ${converged}`,
  );

  // cell-reconnect: cell records are lost, then the port is re-established.
  host.filter = (b) => ([T.CELL_INIT, T.CELL_BATCH, T.CELL_RETIRE].includes(tagOf(b)) ? 'drop' : 'pass');
  evalOk('upd g 0.9');
  await audio(0.5);
  await fresh();
  const stale = probe(0).value;
  host.filter = null;
  const tc = host.now;
  host.call('main_resync');
  const synced = await until(() => near(probe(0).value, 0.9), 2);
  await audio(1.0);
  collectReads();
  const lead = host.call('main_stat', 0);
  const late = reads.filter((x) => x.cell === cellG && x.t >= tc + lead + 0.05);
  const sent = sentSince(tc + lead + 0.05);
  const sentOk = sent.every((e) => (e.kind === 1 && near(e.value, 0.9)) || (e.kind === 2 && e.value === cellG));
  record(
    'cell-reconnect',
    near(stale, 0.46) && synced && late.length > 0 && late.every((x) => near(x.value, 0.9)) && sentOk,
    `mirror stale at ${stale} while disconnected; after main_resync the snapshot landed (${synced}); `
      + `voice-start reads after the commit lead: ${late.map((x) => x.value.toFixed(2))}; `
      + `${sent.length} events sent after it, all Const(0.9) or the cell: ${sentOk}`,
  );
  evalOk('stop :d1');
  await audio(0.3);

  // release-after-genbump: two tagged live voices, a slot generation bump,
  // then a release of one tag.
  const analog = host.callStr('inst_id', 'analog');
  host.call('harness_live_note', 200, 0, 60, 1, analog, 220, -1, -1, 0);
  host.call('harness_live_note', 200, 0, 64, 2, analog, 330, -1, -1, 0);
  const both = await until(() => {
    const v = tagged();
    return v.some((x) => x.seq === 1 && !x.released) && v.some((x) => x.seq === 2 && !x.released);
  }, 2);
  host.call('harness_slot_control', 200, 7, host.now, 0);
  await audio(0.1);
  host.call('harness_voice_release', 200, 0, 60, 1);
  await audio(0.15);
  await fresh();
  const v = tagged();
  const one = v.find((x) => x.seq === 1);
  const two = v.find((x) => x.seq === 2);
  record(
    'release-after-genbump',
    both && (!one || one.released) && two && !two.released,
    `both tagged voices sounding=${both}; after SlotControl(gen 7) and VoiceRelease(seq 1): `
      + `seq 1 ${one ? (one.released ? 'released' : 'OPEN') : 'ended'}, seq 2 ${two ? (two.released ? 'RELEASED' : 'open') : 'MISSING'}`,
  );
  host.call('harness_voice_release', 200, 0, 64, 2);

  // release-tombstone: the release arrives before its note-on.
  const dropped = rep()[R.DROPPED];
  host.call('harness_voice_release', 200, 0, 67, 3);
  host.call('harness_live_note', 200, 0, 67, 3, analog, 392, -1, -1, 0);
  await audio(0.3);
  await fresh();
  const three = tagged().find((x) => x.seq === 3);
  record(
    'release-tombstone',
    !three && rep()[R.DROPPED] >= dropped + 1,
    `release before start: tagged voice seq 3 ${three ? 'STARTED' : 'never started'}, `
      + `dropped ${dropped} -> ${rep()[R.DROPPED]}`,
  );
}

async function criterion11() {
  const analog = host.callStr('inst_id', 'analog');
  const gaps = () => (host.js ? host.js[3] : 0);

  // A sustained open voice under the load and the graph swap.
  host.call('harness_live_note', 201, 0, 48, 10, analog, 110, -1, -1, 0);
  await audio(0.4);
  host.workletCall('worklet_reset_watch');
  await fresh();

  // load-during-playback
  host.putSample('bd:0', sine(2.0, 220), 48000, 1);
  const g0 = gaps();
  evalOk('s :bd > d3');
  const loaded = await until(() => {
    const id = host.callStr('sample_id', 'bd:0');
    return id >= 0 && Number(BigInt(id) >> 32n) === 1;
  }, 6);
  await audio(0.5);
  let r = await fresh();
  record(
    'load-during-playback',
    loaded && r[R.DROPOUTS] === 0 && r[R.RMS_MIN] > 0 && r[R.SOUNDING] > 0 && gaps() === g0,
    `2 s sample installed while a tone sounds: ${loaded}; slices sent ${host.call('main_stat', 3)}; `
      + `silent quanta after sounding ${r[R.DROPOUTS]}, min block RMS ${r[R.RMS_MIN].toExponential(2)}, `
      + `frame gaps ${gaps() - g0}`,
  );
  evalOk('stop :d3');

  // graph-replace-during-playback
  evalOk('inst hvtone freq: float = 220:\n\tsaw freq\n\t\t> * 0.2');
  const tone = host.callStr('inst_id', 'hvtone');
  host.call('harness_live_note', 202, 0, 50, 11, tone, 110, -1, -1, 0);
  await audio(0.4);
  host.workletCall('worklet_reset_watch');
  await fresh();
  const installed = host.call('main_stat', 6);
  const g1 = gaps();
  evalOk('inst hvtone freq: float = 220:\n\ttri freq\n\t\t> * 0.2');
  const swapped = await until(() => host.call('main_stat', 6) > installed, 3);
  await audio(0.5);
  r = await fresh();
  const toneVoice = tagged().find((x) => x.seq === 11);
  record(
    'graph-replace-during-playback',
    swapped && r[R.DROPOUTS] === 0 && r[R.RMS_MIN] > 0 && toneVoice && gaps() === g1,
    `redefined inst hvtone installed (Installed acks ${installed} -> ${host.call('main_stat', 6)}); `
      + `the sounding voice kept playing: ${!!toneVoice}; silent quanta ${r[R.DROPOUTS]}, `
      + `min block RMS ${r[R.RMS_MIN].toExponential(2)}, frame gaps ${gaps() - g1}`,
  );
  host.call('harness_voice_release', 202, 0, 50, 11);

  // arena-exhausted: main-side admission, then the worklet's own check.
  let mark = host.console.length;
  host.putSample('sd:0', new Float32Array(Math.ceil((ARENA * 1.1) / 4)), 48000, 1);
  evalOk('s :sd > d4');
  const admitted = await until(() => consoleHas(mark, 'arena-exhausted'), 3);
  const fa = rep()[R.FAULT_ARENA];
  host.postRaw(sampleBegin(245, 1, ARENA, 1, 48000));
  const faulted = await until(() => rep()[R.FAULT_ARENA] > fa, 2);
  record(
    'arena-exhausted',
    admitted && faulted,
    `oversized sample refused before sending with an arena-exhausted diagnostic: ${admitted}; `
      + `an oversized SampleBegin on the worklet is an ArenaExhausted fault: ${faulted}`,
  );
  evalOk('stop :d4');

  // graph-too-large
  // Ten groups of 15 terms (about 320 nodes) keep the lowering recursion
  // shallow; a single 130-term chain can exhaust the browser main-thread
  // stack in a debug build (recorded for BE-FINAL).
  mark = host.console.length;
  const groups = Array.from({ length: 10 }, (_, g) =>
    `{+ {saw freq} ${Array.from({ length: 15 }, (_, k) => g * 15 + k + 1).join(' ')}}`);
  host.eval(`inst big freq:\n\t+ ${groups.join(' ')}`);
  record(
    'graph-too-large',
    consoleHas(mark, 'graph-too-large'),
    host.console.slice(mark).map((c) => c.line).join(' | ').slice(0, 300),
  );

  // deferred-queue-overflow: 12 slices bypassing the sender window.
  mark = host.console.length;
  const fo = rep()[R.FAULT_OVERFLOW];
  host.call('harness_slice_burst', 241, 12);
  const overflow = await until(() => rep()[R.FAULT_OVERFLOW] > fo && consoleHas(mark, 'install-queue-overflow'), 3);
  record(
    'deferred-queue-overflow',
    overflow,
    `faults ${fo} -> ${rep()[R.FAULT_OVERFLOW]}; main diagnostic: `
      + `${host.console.slice(mark).map((c) => c.line).filter((l) => l.includes('overflow')).slice(0, 1)}`,
  );

  // unload-while-playing: a voice whose event references the bd sample
  // (`bank`), which the engine counts as a user of the resource (16.1). The
  // voice is an `analog` one: the prelude `sampler` template does not install
  // on the worklet in this tree (BadEdge, recorded for BE-FINAL).
  const rbd = Number(BigInt(host.callStr('sample_id', 'bd:0')) & 0xffffffffn);
  const release = host.callStr('ctl_id', 'release');
  host.workletCall('worklet_probe_resource', rbd);
  host.call('harness_live_note', 203, 0, 60, 20, analog, 440, rbd, release, 1.5);
  const playing = await until(() => tagged().some((x) => x.seq === 20), 2);
  host.call('harness_unload', rbd);
  await audio(0.3);
  r = await fresh();
  const heldState = r[R.RES_STATE];
  const heldRetired = host.call('main_retired', rbd);
  const stillPlaying = tagged().some((x) => x.seq === 20);
  host.call('harness_voice_release', 203, 0, 60, 20);
  // Worklet times: the last report that still shows the voice, and the
  // frame time posted with the `Retired` ack.
  let lastActive = -1;
  let retiredAt = -1;
  const retired = await until(() => {
    if (tagged().some((x) => x.seq === 20)) lastActive = Math.max(lastActive, rep()[R.NOW]);
    if (host.call('main_retired', rbd) === 1) {
      retiredAt = host.now;
      return true;
    }
    return false;
  }, 8);
  r = await fresh();
  const voiceGoneFirst = retired && lastActive >= 0 && lastActive < retiredAt
    && !tagged().some((x) => x.seq === 20);
  record(
    'unload-while-playing',
    playing && heldState === 3 && heldRetired === 0 && stillPlaying && retired && voiceGoneFirst
      && r[R.RES_STATE] === 0,
    `resource ${rbd}: while its voice plays after the unload: state ${heldState} (3 = retiring), `
      + `Retired acked ${heldRetired}; voice last seen at ${lastActive.toFixed(3)} s, Retired at ${retiredAt.toFixed(3)} s `
      + `(voice gone first ${voiceGoneFirst}), `
      + `state ${r[R.RES_STATE]}`,
  );

  // install-burst-credit: four installs, with one slice forced to arrive
  // in the same quantum as a graph install.
  const keys = ['hh:0', 'cp:0', 'crash:0', 'piano:0'];
  keys.forEach((k, i) => host.putSample(k, sine(2.73, 200 + 50 * i), 48000, 1));
  host.workletCall('worklet_reset_watch');
  await fresh();
  let heldSlice = null;
  let heldGraph = null;
  host.filter = (b) => {
    const t = tagOf(b);
    if (t === T.SLICE && !heldSlice) {
      heldSlice = b;
      return 'hold';
    }
    if (t === T.GRAPH && heldSlice && !heldGraph) {
      heldGraph = b;
      return 'hold';
    }
    return 'pass';
  };
  evalOk('s [:hh :cp :crash :piano] > d5');
  await until(() => heldSlice !== null, 2);
  evalOk('inst burstg freq:\n\tsaw freq');
  await until(() => heldGraph !== null, 2);
  host.filter = null;
  host.release((held) => [...held.filter((b) => b === heldGraph), ...held.filter((b) => b !== heldGraph)]);
  const all = await until(
    () => keys.every((k) => {
      const id = host.callStr('sample_id', k);
      return id >= 0 && Number(BigInt(id) >> 32n) === 1;
    }),
    20,
  );
  r = await fresh();
  record(
    'install-burst-credit',
    all && r[R.BYTES_MAX] <= INSTALL_BYTES_PER_QUANTUM && r[R.DEFERRED_ACKS] >= 1
      && r[R.STAGED_MAX] <= INSTALL_BYTES_PER_QUANTUM + 4096,
    `4 x ${(2.73 * 48000 * 4 / 1024).toFixed(0)} KB installed: ${all}; slices acked ${r[R.SLICE_ACKS]}; `
      + `max install bytes copied by one process() ${r[R.BYTES_MAX]} (limit ${INSTALL_BYTES_PER_QUANTUM}); `
      + `max install payload staged per quantum ${r[R.STAGED_MAX]}; quanta with a deferred record ${r[R.DEFERRED_QUANTA]}; `
      + `SliceOk acks withheld to a later quantum ${r[R.DEFERRED_ACKS]}`,
  );
  evalOk('stop :d5');
  host.call('harness_voice_release', 201, 0, 48, 10);
}

async function run() {
  const t0 = performance.now();
  try {
    host = await startHost({
      wasmUrl: '/vactr.wasm',
      processorUrl: '/editor/worklet/processor.js',
      arenaBytes: ARENA,
      voices: 64,
      onConsole: (line) => log(`console: ${line}`),
      onLog: log,
    });
  } catch (e) {
    return finish({ blocked: true, reason: `the host did not start: ${e}` });
  }
  const ctx = host.ctx;
  if (ctx.state !== 'running') {
    ctx.resume().catch(() => {});
  }
  const running = await until(() => ctx.state === 'running', 10);
  if (!running) {
    return finish({ blocked: true, reason: `the AudioContext stayed '${ctx.state}'` });
  }
  const ready = await Promise.race([host.ready.then(() => true), sleep(20000).then(() => false)]);
  if (!ready) {
    return finish({ failed: `the worklet never became ready (${host.errors.join('; ')})` });
  }
  await until(() => rep() !== null, 5);
  log(`ready: sample rate ${ctx.sampleRate}, worklet memory ${host.workletMemory} bytes, `
    + `base latency ${ctx.baseLatency}`);
  const memInit = rep()[R.MEM_INIT];
  const startupFaults = host.console.filter((c) => c.line.includes('refused resource')).map((c) => c.line);
  if (startupFaults.length > 0) log(`note: prelude template installs refused by the worklet: ${startupFaults.length}`);
  try {
    await criterion8();
    await criterion11();
  } catch (e) {
    record('harness-error', false, String(e && e.stack ? e.stack : e));
  }
  await audio(0.3);
  const r = await fresh();
  const memoryStable = r[R.MEM_INIT] === memInit && r[R.MEM_NOW] === memInit;
  record(
    'memory-stable',
    memoryStable,
    `worklet memory at the end of init ${memInit} bytes, at the end of the run ${r[R.MEM_NOW]} bytes`,
  );
  return finish({
    memoryStable,
    notes: { startupFaults },
    seconds: (performance.now() - t0) / 1000,
    sampleRate: ctx.sampleRate,
    report: Array.from(r),
    js: host.js,
    workletErrors: host.errors,
    console: host.console.slice(-200),
  });
}

function finish(extra) {
  const body = {
    ...extra,
    checks,
    userAgent: navigator.userAgent,
    log: logLines.slice(-400),
  };
  if (auto) {
    fetch('/report', { method: 'POST', body: JSON.stringify(body) }).catch(() => {});
  }
  log(`done: ${checks.filter((c) => c.pass).length}/${checks.length} checks passed`);
  return body;
}

document.getElementById('start').addEventListener('click', () => run());
if (auto) run();
