// Types for the plain-JS main-thread host (worklet/host.js, design 12.8.10,
// 15.1.3). `init` and `onRecord` are the TASK-010 editor options.

/** The wasm #1 exports the host calls (a subset; the rest stay untyped). */
export interface HostExports {
  memory: WebAssembly.Memory | { buffer: ArrayBuffer };
  alloc(len: number): number;
  free(ptr: number, len: number): void;
  outbox_ptr(): number;
  outbox_len(): number;
  outbox_clear(): void;
  [name: string]: unknown;
}

export interface HostOptions {
  wasmUrl: string;
  processorUrl: string;
  arenaBytes?: number;
  voices?: number;
  /** Opt-in direct stems on channels 3/4; defaults to stereo. */
  outputChannels?: 2 | 4;
  sampleRate?: number;
  tickEvery?: number;
  /** `'main'` (default: the dev harness) or `'session'` (the editor). */
  init?: 'main' | 'session';
  /** Receives 0x71-0x73 records; they never reach the worklet. */
  onRecord?: (tag: number, payload: Uint8Array) => void;
  onConsole?: (line: string) => void;
  onLog?: (line: string) => void;
}

export type FilterVerdict = 'pass' | 'hold' | 'drop' | ArrayBuffer;

export interface WorkletPortLike {
  postMessage(message: unknown, transfer?: Transferable[]): void;
  onmessage: ((e: MessageEvent) => void) | null;
}

export interface WorkletNodeLike {
  port: WorkletPortLike;
}

export class VactrHost {
  constructor(ctx: AudioContext | null, node: WorkletNodeLike, x: HostExports, opts: HostOptions);
  ctx: AudioContext;
  node: WorkletNodeLike;
  /** Four-channel splitter; outputs 2/3 expose direct stems when enabled. */
  quadSplitter: ChannelSplitterNode | null;
  x: HostExports;
  opts: HostOptions;
  /** The worklet's last posted frame time (audio seconds). */
  now: number;
  tickEvery: number;
  report: Float64Array | null;
  console: { t: number; line: string }[];
  errors: string[];
  filter: ((buf: ArrayBuffer) => FilterVerdict) | null;
  held: ArrayBuffer[];
  posted: number;
  dropped: number;
  ready: Promise<unknown>;
  onWorklet(message: unknown): void;
  withBytes<T>(bytes: Uint8Array, f: (ptr: number, len: number) => T): T;
  flush(): void;
  postRaw(buf: ArrayBuffer): void;
  /** Connects a Web Audio source to the stereo master-effect input. */
  connectInput(source: AudioNode): void;
  workletCall(name: string, ...args: unknown[]): void;
  release(order?: (held: ArrayBuffer[]) => ArrayBuffer[]): void;
  eval(text: string): number;
  call(name: string, ...args: unknown[]): unknown;
  callStr(name: string, s: string, ...args: unknown[]): unknown;
  putSample(key: string, frames: Float32Array, rate: number, channels: number): unknown;
  loadSample(key: string, url: string): Promise<unknown>;
  sent(i: number): { time: number; slot: number; kind: number; value: number } | null;
}

export function startHost(opts: HostOptions): Promise<VactrHost>;
