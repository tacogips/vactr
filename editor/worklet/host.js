// Vactrol main-thread host: the main half (wasm #1, evaluator + scheduler).
// Design: design-docs/specs/design-implementation.md 12.8.10, 16, 16.1.
//
// Plain JS module, no build step. `startHost` fetches the module bytes,
// instantiates wasm #1, creates the AudioContext and the AudioWorkletNode,
// and posts a COPY of the bytes to the worklet (wasm #2). On every worklet
// message it hands the worklet's records to `inbox`, calls `tick` with the
// posted frame time (at most every `tickEvery` seconds of audio time), and
// posts each outbox record to the worklet as a transferred ArrayBuffer.
// Console records (tag 0x70) go to `onConsole` instead. JS never parses a
// record beyond the frame length and that routing tag. Samples are decoded
// here with `decodeAudioData` (off the audio thread) and handed over with
// `sample_put`.
//
// A test hook, `host.filter(buf) -> 'pass' | 'hold' | 'drop' | ArrayBuffer`,
// sits on the main -> worklet path so the dev harness can hold, drop,
// replace or reorder records; `host.release()` posts the held ones in order.
//
// Editor options (TASK-010, design 15.1.3; additive, the defaults keep the
// behavior above): `init: 'main' | 'session'` (default 'main') picks
// `main_init` + `tick`/`inbox`/`sample_put`, or `session_init` +
// `session_tick`/`session_inbox`/`session_sample_put`. Records tagged
// 0x71-0x73 (session envelope, render, package driver) go to
// `onRecord(tag, payload)` after the outbox is cleared and are NEVER posted
// to the worklet.

const TAG_CONSOLE = 0x70;
const TAG_EDITOR_FIRST = 0x71;
const TAG_EDITOR_LAST = 0x73;
const EXPORTS = {
  main: { init: 'main_init', tick: 'tick', inbox: 'inbox', samplePut: 'sample_put' },
  session: {
    init: 'session_init',
    tick: 'session_tick',
    inbox: 'session_inbox',
    samplePut: 'session_sample_put',
  },
};
const enc = new TextEncoder();
const dec = new TextDecoder();

export class VactrolHost {
  constructor(ctx, node, x, opts) {
    this.ctx = ctx;
    this.node = node;
    this.x = x;
    this.opts = opts;
    this.now = 0;
    this.lastTick = -1;
    this.tickEvery = opts.tickEvery ?? 0.005;
    this.fn = opts.init === 'session' ? EXPORTS.session : EXPORTS.main;
    this.report = null;
    this.js = null;
    this.console = [];
    this.errors = [];
    this.filter = null;
    this.held = [];
    this.posted = 0;
    this.dropped = 0;
    this.ready = new Promise((resolve) => {
      this.onReady = resolve;
    });
    node.port.onmessage = (e) => this.onWorklet(e.data);
  }

  onWorklet(m) {
    if (m.type === 'ready') {
      this.workletMemory = m.memory;
      this.onReady(m);
      return;
    }
    if (m.type === 'error') {
      this.errors.push(m.message);
      this.log('worklet error: ' + m.message);
      return;
    }
    if (m.o) {
      this.withBytes(new Uint8Array(m.o), (p, n) => this.x[this.fn.inbox](p, n));
    }
    if (m.r) {
      this.report = m.r;
    }
    if (m.js) {
      this.js = m.js;
    }
    if (typeof m.t === 'number') {
      this.now = m.t;
      if (this.lastTick < 0 || this.now - this.lastTick >= this.tickEvery) {
        this.lastTick = this.now;
        this.x[this.fn.tick](this.now);
      }
    }
    this.flush();
  }

  log(line) {
    if (this.opts.onLog) this.opts.onLog(line);
  }

  // Copies `bytes` into wasm #1 memory for the duration of `f(ptr, len)`.
  withBytes(bytes, f) {
    const n = bytes.byteLength;
    const p = this.x.alloc(n);
    new Uint8Array(this.x.memory.buffer, p, n).set(bytes);
    try {
      return f(p, n);
    } finally {
      this.x.free(p, n);
    }
  }

  // Moves every outbox record: console lines to the page, editor records
  // (0x71-0x73) to `onRecord` once the outbox is cleared (a listener may
  // re-enter wasm), the rest to the worklet (through the test filter).
  flush() {
    const x = this.x;
    const len = x.outbox_len();
    if (len === 0) return;
    const view = new Uint8Array(x.memory.buffer, x.outbox_ptr(), len);
    let at = 0;
    const editor = [];
    while (at + 4 <= len) {
      const n = view[at] | (view[at + 1] << 8) | (view[at + 2] << 16) | (view[at + 3] << 24);
      const rec = view.slice(at + 4, at + 4 + n);
      at += 4 + n;
      if (rec[0] === TAG_CONSOLE) {
        const line = dec.decode(rec.subarray(1));
        this.console.push({ t: this.now, line });
        if (this.opts.onConsole) this.opts.onConsole(line);
      } else if (rec[0] >= TAG_EDITOR_FIRST && rec[0] <= TAG_EDITOR_LAST) {
        editor.push(rec);
      } else {
        this.toWorklet(rec.buffer);
      }
    }
    x.outbox_clear();
    if (this.opts.onRecord) {
      for (const rec of editor) this.opts.onRecord(rec[0], rec.subarray(1));
    }
  }

  toWorklet(buf) {
    let verdict = 'pass';
    if (this.filter) verdict = this.filter(buf);
    if (verdict === 'hold') {
      this.held.push(buf);
    } else if (verdict === 'drop') {
      this.dropped += 1;
    } else if (verdict instanceof ArrayBuffer) {
      this.postRaw(verdict);
    } else {
      this.postRaw(buf);
    }
  }

  // Posts one record straight to the worklet (transferred).
  postRaw(buf) {
    this.posted += 1;
    this.node.port.postMessage(buf, [buf]);
  }

  // Calls a worklet probe export (`worklet_probe_*`, `worklet_reset_watch`).
  workletCall(name, ...args) {
    this.node.port.postMessage({ type: 'call', name, args });
  }

  // Posts the held records in order (optionally reordered by `order`).
  release(order) {
    const held = this.held;
    this.held = [];
    const list = order ? order(held) : held;
    for (const b of list) this.postRaw(b);
  }

  // Evaluates source text; returns the number of diagnostics and failures.
  eval(text) {
    const n = this.withBytes(enc.encode(text), (p, len) => this.x.eval(p, len));
    this.flush();
    return n;
  }

  // Calls a main-half export, then moves the outbox.
  call(name, ...args) {
    const r = this.x[name](...args);
    this.flush();
    return r;
  }

  // Calls a main-half export whose first two arguments are a UTF-8 string.
  callStr(name, s, ...args) {
    const r = this.withBytes(enc.encode(s), (p, n) => this.x[name](p, n, ...args));
    this.flush();
    return r;
  }

  // Hands over interleaved f32 frames under `key` (`bank:index` or a path).
  putSample(key, frames, rate, channels) {
    const kb = enc.encode(key);
    const bytes = new Uint8Array(frames.buffer, frames.byteOffset, frames.byteLength);
    return this.withBytes(kb, (kp, kn) =>
      this.withBytes(bytes, (dp) =>
        this.x[this.fn.samplePut](kp, kn, dp, frames.length, rate, channels),
      ),
    );
  }

  // Decodes a fetched file with decodeAudioData and hands it over.
  async loadSample(key, url) {
    const data = await (await fetch(url)).arrayBuffer();
    const audio = await this.ctx.decodeAudioData(data);
    const ch = audio.numberOfChannels;
    const frames = new Float32Array(audio.length * ch);
    for (let c = 0; c < ch; c += 1) {
      const src = audio.getChannelData(c);
      for (let i = 0; i < audio.length; i += 1) frames[i * ch + c] = src[i];
    }
    return this.putSample(key, frames, audio.sampleRate, ch);
  }

  // The `i`th newest sent event: {time, slot, kind, value}.
  sent(i) {
    const p = this.x.alloc(32);
    try {
      if (this.x.main_sent(i, p) === 0) return null;
      const f = new Float64Array(this.x.memory.buffer, p, 4);
      return { time: f[0], slot: f[1], kind: f[2], value: f[3] };
    } finally {
      this.x.free(p, 32);
    }
  }
}

// Starts both halves. Options: wasmUrl, processorUrl, arenaBytes, voices,
// sampleRate, onConsole, onLog, tickEvery, init ('main' | 'session'),
// onRecord(tag, payload).
export async function startHost(opts) {
  const bytes = await (await fetch(opts.wasmUrl)).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const x = instance.exports;
  const ctx = new AudioContext({
    sampleRate: opts.sampleRate ?? 48000,
    latencyHint: 'interactive',
  });
  await ctx.audioWorklet.addModule(opts.processorUrl);
  const init = opts.init === 'session' ? EXPORTS.session.init : EXPORTS.main.init;
  x[init](ctx.sampleRate, opts.arenaBytes ?? 0);
  const node = new AudioWorkletNode(ctx, 'vactrol-processor', {
    numberOfInputs: 0,
    numberOfOutputs: 1,
    outputChannelCount: [2],
    processorOptions: { arenaBytes: opts.arenaBytes ?? 0, voices: opts.voices ?? 0 },
  });
  node.connect(ctx.destination);
  const host = new VactrolHost(ctx, node, x, opts);
  host.flush();
  const copy = bytes.slice(0);
  node.port.postMessage({ type: 'module', bytes: copy }, [copy]);
  return host;
}
