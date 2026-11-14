// Vactr AudioWorklet processor: the worklet half (wasm #2, DSP only).
// Design: design-docs/specs/design-implementation.md 12.8.10, 16, 16.1.
//
// Plain JS, no build step. The main thread posts a COPY of the module bytes
// ({type: 'module', bytes}); the processor instantiates wasm #2 during init
// and renders silence until it is ready. Every record from the main thread
// arrives as its own ArrayBuffer; `port.onmessage` only stores the reference
// in a preallocated fixed-size slot array (a full array drops and counts),
// so the handler is O(1) and copies nothing. `process()` moves the held
// records into wasm (one staging copy each, then `worklet_inbox`; the
// engine applies install records under the 16.1 per-quantum credit), renders
// one quantum, copies the planar output to the outputs, and posts the frame
// time (the worklet is the timebase), the outbox records and, every 4
// quanta, a copy of the report array. The dev harness may also post
// {type: 'call', name, args} for the `worklet_probe_*` and
// `worklet_reset_watch` exports.

const SLOTS = 64;
// INBOX_SLOT_BYTES of src/dsp/ring.rs: 64 + one 64 KB slice.
const STAGING_BYTES = 64 + 65536;
const REPORT_EVERY = 4;

class VactrProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    const o = (options && options.processorOptions) || {};
    this.arenaBytes = o.arenaBytes | 0;
    this.voices = o.voices | 0;
    this.outputChannels = o.outputChannels === 4 ? 4 : 2;
    this.x = null;
    this.slots = new Array(SLOTS).fill(null);
    this.head = 0;
    this.count = 0;
    this.slotDrops = 0;
    this.tooLarge = 0;
    this.errors = 0;
    this.quanta = 0;
    this.lastFrame = -1;
    this.gaps = 0;
    this.port.onmessage = (e) => {
      const d = e.data;
      if (d instanceof ArrayBuffer) {
        if (this.count < SLOTS) {
          this.slots[(this.head + this.count) % SLOTS] = d;
          this.count += 1;
        } else {
          this.slotDrops += 1;
        }
        return;
      }
      if (d && d.type === 'module' && !this.x) {
        this.init(d.bytes);
      } else if (d && d.type === 'call' && this.x && /^worklet_(probe|reset)/.test(d.name)) {
        // Dev-harness probe setters: one bounded call, no copy.
        this.x[d.name](...d.args);
      }
    };
  }

  async init(bytes) {
    try {
      const { instance } = await WebAssembly.instantiate(bytes, {});
      const x = instance.exports;
      if (this.outputChannels === 4) {
        x.worklet_init_quad(sampleRate, this.arenaBytes, this.voices);
      } else {
        x.worklet_init(sampleRate, this.arenaBytes, this.voices);
      }
      this.bind(x);
      this.x = x;
      this.port.postMessage({ type: 'ready', memory: x.memory.buffer.byteLength });
    } catch (err) {
      this.port.postMessage({ type: 'error', message: 'worklet init: ' + String(err) });
    }
  }

  // Typed views over wasm memory (memory never grows after init, 16.1; the
  // views are rebuilt only if it ever did).
  bind(x) {
    this.mem = x.memory.buffer;
    this.u8 = new Uint8Array(this.mem);
    this.staging = x.staging_ptr();
    this.inputPtr = x.input_ptr();
    this.input = new Float32Array(this.mem, this.inputPtr, 2 * 512);
    this.report = new Float64Array(this.mem, x.report_ptr(), x.report_len());
    this.planarPtr = -1;
  }

  process(inputs, outputs) {
    const out = outputs[0];
    const x = this.x;
    if (!x) {
      return true;
    }
    try {
      if (x.memory.buffer !== this.mem) {
        this.bind(x);
      }
      if (this.lastFrame >= 0 && currentFrame - this.lastFrame !== out[0].length) {
        this.gaps += 1;
      }
      this.lastFrame = currentFrame;
      while (this.count > 0) {
        const buf = this.slots[this.head];
        if (buf.byteLength > STAGING_BYTES) {
          this.tooLarge += 1;
        } else {
          this.u8.set(new Uint8Array(buf), this.staging);
          if (x.worklet_inbox(buf.byteLength) === 0) {
            break; // inbox full: keep it (and the order) for the next quantum
          }
        }
        this.slots[this.head] = null;
        this.head = (this.head + 1) % SLOTS;
        this.count -= 1;
      }
      const frames = out[0].length;
      const incoming = inputs[0] || [];
      const left = incoming[0];
      const right = incoming[1] || left;
      for (let frame = 0; frame < frames; frame++) {
        const l = left && frame < left.length ? left[frame] : 0;
        const r = right && frame < right.length ? right[frame] : 0;
        this.input[2 * frame] = Number.isFinite(l) ? l : 0;
        this.input[2 * frame + 1] = Number.isFinite(r) ? r : 0;
      }
      const p = x.process(frames);
      if (p !== this.planarPtr || !this.planar || this.planar.length !== this.outputChannels * frames) {
        this.planar = new Float32Array(this.mem, p, this.outputChannels * frames);
        this.planarPtr = p;
      }
      for (let ch = 0; ch < this.outputChannels && ch < out.length; ch++) {
        out[ch].set(this.planar.subarray(ch * frames, (ch + 1) * frames));
      }
      let o = null;
      const len = x.outbox_len();
      if (len > 0) {
        const at = x.outbox_ptr();
        o = this.u8.slice(at, at + len).buffer;
        x.outbox_clear();
      }
      this.quanta += 1;
      const msg = {
        t: x.worklet_now(),
        o,
        js: [this.slotDrops, this.tooLarge, this.errors, this.gaps, this.count],
      };
      if (this.quanta % REPORT_EVERY === 0) {
        msg.r = this.report.slice();
      }
      this.port.postMessage(msg, o ? [o] : []);
    } catch (err) {
      this.errors += 1;
      if (this.errors <= 5) {
        this.port.postMessage({ type: 'error', message: 'process: ' + String(err) });
      }
    }
    return true;
  }
}

registerProcessor('vactr-processor', VactrProcessor);
