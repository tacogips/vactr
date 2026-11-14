// The real host-wasm artifact for node tests (design 15.1.2 G1; command.md
// "Browser transport (raw wasm ABI, TASK-010)").
//
// `loadVactrWasm()` reads `$VACTR_WASM`, else
// `../target/wasm32-unknown-unknown/debug/vactr.wasm` relative to editor/
// (vitest's working directory), and instantiates it with no imports. It
// THROWS when the file is missing or the module has no `session_init`, so a
// test using it fails rather than skips. The helpers write inputs into
// `alloc` memory, call exports, and read the framed outbox records
// (`[u32 LE length][tag][payload]`) exactly as `worklet/host.js` does.

// No @types/node in the pinned dependency set: the node APIs used here are
// typed locally and loaded through a non-literal dynamic import.
interface NodeFs {
  readFileSync(path: string): Uint8Array<ArrayBuffer>;
}

interface NodeProcess {
  cwd(): string;
  env: Record<string, string | undefined>;
}

const proc = (globalThis as unknown as { process: NodeProcess }).process;

export const DEFAULT_WASM = '../target/wasm32-unknown-unknown/debug/vactr.wasm';

export const TAG_CONSOLE = 0x70;
export const TAG_SESSION = 0x71;
export const TAG_RENDER = 0x72;
export const TAG_PKG = 0x73;

/** One outbox record: its tag byte and the payload after it. */
export interface WasmRecord {
  tag: number;
  bytes: Uint8Array;
}

type Export = (...args: number[]) => number | bigint | void;

const enc = new TextEncoder();
const dec = new TextDecoder();

export interface VactrWasm {
  readonly path: string;
  readonly exports: Record<string, unknown>;
  /** Calls export `name` with numeric arguments. */
  call(name: string, ...args: number[]): number;
  /** Runs `f(ptr, len)` over a copy of `bytes` in `alloc` memory, then frees it. */
  withBytes<T>(bytes: Uint8Array, f: (ptr: number, len: number) => T): T;
  /** Calls `name(ptr, len, ...rest)` over the UTF-8 bytes of `text`. */
  callStr(name: string, text: string, ...rest: number[]): number;
  /** Every framed record in the outbox, in order; clears the outbox. */
  drainRecords(): WasmRecord[];
}

function joinPath(root: string, p: string): string {
  if (p.startsWith('/') || /^[A-Za-z]:[\\/]/.test(p)) return p;
  return `${root.replace(/[\\/]+$/, '')}/${p}`;
}

// One compile per artifact path; every `loadVactrWasm()` gets a fresh
// instance (fresh memory, fresh thread-local state).
const compiled = new Map<string, Promise<WebAssembly.Module>>();

async function compile(path: string): Promise<WebAssembly.Module> {
  const spec: string = 'node:fs';
  const fs = (await import(/* @vite-ignore */ spec)) as NodeFs;
  let bytes: Uint8Array<ArrayBuffer>;
  try {
    bytes = fs.readFileSync(path);
  } catch {
    throw new Error(
      `the wasm artifact ${path} is missing: build it with ` +
        '`cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm` ' +
        'or set VACTR_WASM',
    );
  }
  return WebAssembly.compile(bytes);
}

/** Loads and instantiates the real artifact; throws on any problem. */
export async function loadVactrWasm(): Promise<VactrWasm> {
  const path = joinPath(proc.cwd(), proc.env.VACTR_WASM || DEFAULT_WASM);
  let mod = compiled.get(path);
  if (!mod) {
    mod = compile(path);
    compiled.set(path, mod);
  }
  const instance = await WebAssembly.instantiate(await mod, {});
  const x = instance.exports as Record<string, unknown>;
  if (typeof x.session_init !== 'function') {
    // The `vactr` bin and cdylib share the uplifted file name on wasm32;
    // the cdylib is always `target/wasm32-unknown-unknown/debug/deps/vactr.wasm`.
    throw new Error(`${path} does not export session_init (not the host-wasm cdylib)`);
  }
  const fn = (name: string): Export => {
    const f = x[name];
    if (typeof f !== 'function') throw new Error(`${path} does not export ${name}`);
    return f as Export;
  };
  const memory = (): Uint8Array => new Uint8Array((x.memory as WebAssembly.Memory).buffer);

  const withBytes = <T>(input: Uint8Array, f: (ptr: number, len: number) => T): T => {
    const len = input.length;
    const ptr = Number(fn('alloc')(len));
    memory().set(input, ptr);
    try {
      return f(ptr, len);
    } finally {
      fn('free')(ptr, len);
    }
  };

  return {
    path,
    exports: x,
    call: (name, ...args) => Number(fn(name)(...args) ?? 0),
    withBytes,
    callStr: (name, text, ...rest) => withBytes(enc.encode(text), (p, n) => Number(fn(name)(p, n, ...rest) ?? 0)),
    drainRecords: () => {
      const ptr = Number(fn('outbox_ptr')());
      const len = Number(fn('outbox_len')());
      const out = memory().slice(ptr, ptr + len);
      fn('outbox_clear')();
      const view = new DataView(out.buffer);
      const records: WasmRecord[] = [];
      let at = 0;
      while (at + 4 <= out.length) {
        const n = view.getUint32(at, true);
        if (n === 0 || at + 4 + n > out.length) break;
        records.push({ tag: out[at + 4] as number, bytes: out.slice(at + 5, at + 4 + n) });
        at += 4 + n;
      }
      return records;
    },
  };
}

// Worklet-bound sample install records (`dsp::ring::TAG_SAMPLE_BEGIN`,
// `host::wire` `SampleSlice`) and the worklet's replies (`SliceOk`,
// `Installed`): `[tag][u32 LE words]`.
const TAG_SAMPLE_SLICE = 0x18;
const TAG_SAMPLE_BEGIN = 0x1a;
const TAG_SLICE_OK = 0x45;
const TAG_INSTALLED = 0x46;

/**
 * Plays the worklet for sample installs: every `SampleBegin` in `records`
 * is answered with `Installed { resource, gen }` and every slice with
 * `SliceOk { resource, offset }`, framed and handed to `session_inbox`.
 * Returns the number of replies.
 */
export function ackSampleInstalls(w: VactrWasm, records: readonly WasmRecord[]): number {
  const replies: number[][] = [];
  for (const r of records) {
    if (r.tag !== TAG_SAMPLE_BEGIN && r.tag !== TAG_SAMPLE_SLICE) continue;
    const v = new DataView(r.bytes.buffer, r.bytes.byteOffset, r.bytes.byteLength);
    const reply = r.tag === TAG_SAMPLE_BEGIN ? TAG_INSTALLED : TAG_SLICE_OK;
    replies.push([reply, v.getUint32(0, true), v.getUint32(4, true)]);
  }
  if (replies.length === 0) return 0;
  const bytes = new Uint8Array(replies.length * 13);
  const out = new DataView(bytes.buffer);
  replies.forEach(([tag, a, b], i) => {
    out.setUint32(i * 13, 9, true);
    out.setUint8(i * 13 + 4, tag as number);
    out.setUint32(i * 13 + 5, a as number, true);
    out.setUint32(i * 13 + 9, b as number, true);
  });
  w.withBytes(bytes, (p, n) => w.call('session_inbox', p, n));
  return replies.length;
}

/** `session_sample_put` of `frames` (interleaved f32) under `key`; its result. */
export function putSample(w: VactrWasm, key: string, frames: Float32Array, rate: number, channels: number): number {
  const data = new Uint8Array(frames.buffer, frames.byteOffset, frames.byteLength);
  return w.withBytes(enc.encode(key), (kp, kn) =>
    w.withBytes(data, (dp) => w.call('session_sample_put', kp, kn, dp, frames.length, rate, channels)),
  );
}

/** The JSON of a `0x71`..`0x73` record; throws for any other tag. */
export function recordJson<T = unknown>(r: WasmRecord): T {
  if (r.tag < TAG_SESSION || r.tag > TAG_PKG) {
    throw new Error(`record tag 0x${r.tag.toString(16)} is not an editor JSON record`);
  }
  return JSON.parse(dec.decode(r.bytes)) as T;
}

/** The decoded JSON of every record tagged `tag`, in order. */
export function jsonOf<T = unknown>(records: readonly WasmRecord[], tag: number): T[] {
  return records.filter((r) => r.tag === tag).map((r) => recordJson<T>(r));
}

/** The text of every `0x70` console record. */
export function consoleLines(records: readonly WasmRecord[]): string[] {
  return records.filter((r) => r.tag === TAG_CONSOLE).map((r) => dec.decode(r.bytes));
}
