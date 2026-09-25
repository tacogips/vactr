// Fake wasm #1 exports for `worklet/host.js` and `WasmCore` tests: a plain
// memory, a bump allocator, a scripted outbox, and a call log. `emit(tag,
// payload)` appends an outbox record; `on(name, fn)` scripts an export.

import type { HostExports, WorkletNodeLike } from '../../worklet/host.js';

const HEAP_START = 1024;
const OUTBOX = 512 * 1024;
const MEMORY = 1024 * 1024;

export interface FakeCall {
  name: string;
  args: unknown[];
  /** The UTF-8 argument of a (ptr, len) text export. */
  text?: string;
  /** The byte argument of a (ptr, len) byte export. */
  bytes?: Uint8Array;
}

const dec = new TextDecoder();
const enc = new TextEncoder();

// Exports whose first two arguments are (ptr, len) text.
const TEXT_EXPORTS = new Set(['session_apply', 'session_check', 'pkg_resolve', 'eval']);
// Exports whose first two arguments are (ptr, len) bytes.
const BYTE_EXPORTS = new Set(['inbox', 'session_inbox', 'session_midi_in']);

const EXPORT_NAMES = [
  'main_init',
  'tick',
  'inbox',
  'sample_put',
  'eval',
  'main_sent',
  'session_init',
  'session_apply',
  'session_tick',
  'session_frame',
  'session_inbox',
  'session_sample_put',
  'session_check',
  'session_midi_in',
  'pkg_resolve',
  'pkg_supply',
];

export class FakeCore {
  readonly memory = { buffer: new ArrayBuffer(MEMORY) };
  readonly calls: FakeCall[] = [];
  private records: Uint8Array[] = [];
  private top = HEAP_START;
  private readonly hooks = new Map<string, (call: FakeCall) => unknown>();
  readonly exports: HostExports;

  constructor() {
    const x: HostExports = {
      memory: this.memory,
      alloc: (n: number) => this.alloc(n),
      free: () => {},
      outbox_ptr: () => OUTBOX,
      outbox_len: () => this.layoutOutbox(),
      outbox_clear: () => {
        this.records = [];
      },
    };
    for (const name of EXPORT_NAMES) x[name] = (...args: unknown[]) => this.invoke(name, args);
    this.exports = x;
  }

  /** Appends one outbox record `[u32 len][tag][payload]`. */
  emit(tag: number, payload: string | Uint8Array): void {
    const body = typeof payload === 'string' ? enc.encode(payload) : payload;
    const rec = new Uint8Array(1 + body.length);
    rec[0] = tag;
    rec.set(body, 1);
    this.records.push(rec);
  }

  /** Scripts export `name`; `fn`'s return value is the export's result. */
  on(name: string, fn: (call: FakeCall) => unknown): void {
    this.hooks.set(name, fn);
  }

  /** The calls of one export. */
  callsOf(name: string): FakeCall[] {
    return this.calls.filter((c) => c.name === name);
  }

  get pendingRecords(): number {
    return this.records.length;
  }

  private alloc(n: number): number {
    if (this.top + n > OUTBOX) this.top = HEAP_START;
    const p = this.top;
    this.top += (n + 7) & ~7;
    return p;
  }

  private view(p: unknown, n: unknown): Uint8Array {
    return new Uint8Array(this.memory.buffer, Number(p), Number(n)).slice();
  }

  private invoke(name: string, args: unknown[]): unknown {
    const call: FakeCall = { name, args };
    if (TEXT_EXPORTS.has(name)) call.text = dec.decode(this.view(args[0], args[1]));
    if (BYTE_EXPORTS.has(name)) call.bytes = this.view(args[0], args[1]);
    if (name === 'pkg_supply') {
      call.text = dec.decode(this.view(args[0], args[1]));
      call.bytes = this.view(args[3], args[4]);
    }
    if (name === 'sample_put' || name === 'session_sample_put') {
      call.text = dec.decode(this.view(args[0], args[1]));
    }
    this.calls.push(call);
    const hook = this.hooks.get(name);
    return hook ? hook(call) : 0;
  }

  private layoutOutbox(): number {
    const mem = new Uint8Array(this.memory.buffer);
    let at = OUTBOX;
    for (const r of this.records) {
      new DataView(this.memory.buffer).setUint32(at, r.length, true);
      mem.set(r, at + 4);
      at += 4 + r.length;
    }
    return at - OUTBOX;
  }
}

/** A worklet node whose port records every posted message. */
export function fakeNode(): WorkletNodeLike & { posted: unknown[] } {
  const posted: unknown[] = [];
  return {
    posted,
    port: {
      onmessage: null,
      postMessage(message: unknown) {
        posted.push(message);
      },
    },
  };
}
