// The browser tier's wasm core and transport (design 15.1.2 G1, 15.1.4;
// command.md "Browser transport (raw wasm ABI, TASK-010)").
//
// `WasmCore` drives wasm #1 in session mode through `worklet/host.js`
// (`init: 'session'`) and routes the editor records the host hands to
// `onRecord`: `0x71` server envelopes to the transport, the `0x71` check
// record to `check()`'s caller, `0x72` render records and `0x73` package
// replies to their listeners. `WasmTransport` is the `Transport` over
// `session_apply` and the `0x71` records. Only `app/main.ts` builds these.

import { startHost, type HostOptions, type VactrolHost } from '../../worklet/host.js';
import type { Transport } from './transport';
import {
  TAG_PKG,
  TAG_RENDER,
  TAG_SESSION,
  type Diagnostic,
  type PkgReply,
  type PkgResolveRequest,
  type RenderRecord,
  type SessionCheckRecord,
} from './types';

const dec = new TextDecoder();
const enc = new TextEncoder();

export type WasmStartOptions = Omit<HostOptions, 'init' | 'onRecord'>;

type Fn = (...args: number[]) => unknown;

export class WasmCore {
  private hostRef: VactrolHost | null = null;
  private readonly sessionListeners: ((text: string) => void)[] = [];
  private readonly renderListeners: ((r: RenderRecord) => void)[] = [];
  private readonly pkgListeners: ((r: PkgReply) => void)[] = [];
  private readonly errorListeners: ((msg: string) => void)[] = [];
  private checking: Diagnostic[] | null = null;

  /** Starts wasm #1 in session mode plus the worklet. */
  static async start(opts: WasmStartOptions, start = startHost): Promise<WasmCore> {
    const core = new WasmCore();
    const host = await start({ ...opts, init: 'session', onRecord: core.onRecord });
    core.attach(host);
    return core;
  }

  /** Binds an already constructed host (tests construct one over fake exports). */
  attach(host: VactrolHost): void {
    this.hostRef = host;
  }

  get host(): VactrolHost {
    if (!this.hostRef) throw new Error('wasm core not started');
    return this.hostRef;
  }

  /** The host's `onRecord` option. */
  readonly onRecord = (tag: number, payload: Uint8Array): void => {
    if (tag === TAG_SESSION) {
      const text = dec.decode(payload);
      if (this.checking !== null && this.captureCheck(text)) return;
      for (const cb of this.sessionListeners) cb(text);
    } else if (tag === TAG_RENDER) {
      const r = this.parse<RenderRecord>(payload, 'render');
      if (r) for (const cb of this.renderListeners) cb(r);
    } else if (tag === TAG_PKG) {
      const r = this.parse<PkgReply>(payload, 'pkg');
      if (r) for (const cb of this.pkgListeners) cb(r);
    }
  };

  onSession(cb: (text: string) => void): () => void {
    return subscribe(this.sessionListeners, cb);
  }

  onRender(cb: (r: RenderRecord) => void): () => void {
    return subscribe(this.renderListeners, cb);
  }

  onPkg(cb: (r: PkgReply) => void): () => void {
    return subscribe(this.pkgListeners, cb);
  }

  /** Undecodable render or package records. */
  onError(cb: (msg: string) => void): () => void {
    return subscribe(this.errorListeners, cb);
  }

  /** `session_apply`: one client envelope (JSON text). */
  apply(text: string): void {
    this.host.callStr('session_apply', text);
  }

  /** `session_check`: static diagnostics of `text`; never executes. */
  check(text: string): Diagnostic[] {
    this.checking = [];
    try {
      this.host.callStr('session_check', text);
      return this.checking;
    } finally {
      this.checking = null;
    }
  }

  /** `session_frame`: resolves the active visual uniform plans. */
  frame(now: number): void {
    this.host.call('session_frame', now);
  }

  /** `session_midi_in`: raw MIDI bytes at an audio-clock time. */
  midiIn(bytes: Uint8Array, time: number): void {
    const h = this.host;
    h.withBytes(bytes, (p, n) => (h.x.session_midi_in as Fn)(p, n, time));
    h.flush();
  }

  /** `session_sample_put`: interleaved frames under `key`. */
  samplePut(key: string, frames: Float32Array, rate: number, channels: number): void {
    this.host.putSample(key, frames, rate, channels);
    this.host.flush();
  }

  /** `pkg_resolve`: one package driver step; the reply arrives on `onPkg`. */
  pkgResolve(req: PkgResolveRequest): void {
    this.host.callStr('pkg_resolve', JSON.stringify(req));
  }

  /** `pkg_supply`: a fetched body (200), a not-found (404) or a failure. */
  pkgSupply(url: string, status: number, body: Uint8Array): void {
    const h = this.host;
    h.withBytes(enc.encode(url), (up, un) =>
      h.withBytes(body, (bp, bn) => (h.x.pkg_supply as Fn)(up, un, status, bp, bn)),
    );
    h.flush();
  }

  private captureCheck(text: string): boolean {
    try {
      const rec = JSON.parse(text) as Partial<SessionCheckRecord>;
      if (rec.kind === 'check' && Array.isArray(rec.diagnostics)) {
        this.checking = rec.diagnostics;
        return true;
      }
    } catch {
      // Not a check record: an ordinary envelope.
    }
    return false;
  }

  private parse<T>(payload: Uint8Array, what: string): T | null {
    try {
      return JSON.parse(dec.decode(payload)) as T;
    } catch (e) {
      for (const cb of this.errorListeners) cb(`${what} record: ${String(e)}`);
      return null;
    }
  }
}

function subscribe<T>(list: T[], cb: T): () => void {
  list.push(cb);
  return () => {
    const i = list.indexOf(cb);
    if (i >= 0) list.splice(i, 1);
  };
}

/** The `Transport` over the wasm session half. */
export class WasmTransport implements Transport {
  private readonly core: WasmCore;
  private readonly offs: (() => void)[] = [];
  private closed = false;

  constructor(core: WasmCore) {
    this.core = core;
  }

  send(text: string): void {
    if (!this.closed) this.core.apply(text);
  }

  onText(cb: (text: string) => void): void {
    this.offs.push(
      this.core.onSession((t) => {
        if (!this.closed) cb(t);
      }),
    );
  }

  close(): void {
    this.closed = true;
    for (const off of this.offs) off();
    this.offs.length = 0;
  }
}
