// The Session Protocol v1 client (design 15.1.4).
//
// The client owns `seq`, correlates replies by `re`, hands every decoded
// server message to the store (which keeps the kinds it tracks), and
// notifies per-kind listeners. Every write helper first flushes the
// document's pending `doc-changed` (14.4 rule 1) and stamps the edit epoch
// current when its input was produced. `setTweak` is rate-limited per
// target: at most one per 16 ms, latest wins, the trailing value always
// sent (the session coalesces further per tick, 14.5.6).

import { DocSync, defaultTimers, type Timers } from './document';
import { decodeServer, encodeClient, type DecodeError } from './envelope';
import type { Transport } from './transport';
import type {
  ClientMsg,
  ServerEnvelope,
  ServerKind,
  Span,
  SubscribeBody,
  InstrumentSelector,
} from './types';

export const TWEAK_INTERVAL_MS = 16;
export const MAX_PENDING_REQUESTS = 64;
export const MAX_TELEMETRY_QUEUE = 64;
const telemetry = (env: ServerEnvelope): boolean => env.re === undefined &&
  (env.kind === 'playing' || env.kind === 'tempo' || env.kind === 'levels');

/** Where the client hands every decoded server message (the store). */
export interface MessageSink {
  apply(env: ServerEnvelope): void;
  beginSongApply?(file: string, revision: number, request: number, notify?: boolean): void;
  publishSongApply?(request: number): void;
  songDocumentChanged?(file: string, revision: number): void;
  cancelSongApply?(request: number): void;
}

export interface ClientOptions {
  store?: MessageSink;
  timers?: Timers;
  /** Milliseconds, for the rate limit. */
  now?: () => number;
  /** The `doc-changed` debounce. */
  debounceMs?: number;
  onError?: (error: unknown) => void;
}

type Listener = (env: ServerEnvelope) => void;

interface Pending {
  resolve: (env: ServerEnvelope) => void;
  reject: (err: Error) => void;
}

interface TweakSlot {
  last: number;
  timer: unknown;
  queued: ClientMsg | null;
}

export class Client {
  private readonly transport: Transport;
  private readonly store: MessageSink | undefined;
  private readonly timers: Timers;
  private readonly now: () => number;
  private readonly debounceMs: number | undefined;
  private readonly onError: (error: unknown) => void;
  private readonly docs = new Map<string, DocSync>();
  private readonly pending = new Map<number, Pending>();
  private readonly listeners = new Map<ServerKind | '*', Listener[]>();
  private readonly errorListeners: ((e: DecodeError) => void)[] = [];
  private readonly tweaks = new Map<string, TweakSlot>();
  private readonly momentaries = new Map<string, TweakSlot>();
  private readonly receiveQueue: ServerEnvelope[] = [];
  private receiving = false;
  private telemetryDropped = 0;
  private telemetryCoalesced = 0;
  get queueStats(): { telemetryQueued: number; dropped: number; coalesced: number; pendingRequests: number } {
    return { telemetryQueued: this.receiveQueue.filter(telemetry).length,
      dropped: this.telemetryDropped, coalesced: this.telemetryCoalesced, pendingRequests: this.pending.size };
  }
  private seq = 0;
  private closed = false;

  constructor(transport: Transport, opts: ClientOptions = {}) {
    this.transport = transport;
    this.store = opts.store;
    this.timers = opts.timers ?? defaultTimers;
    this.now = opts.now ?? (() => Date.now());
    this.debounceMs = opts.debounceMs;
    this.onError = opts.onError ?? ((e) => console.error('client subscriber failed', e));
    transport.onText((text) => this.receive(text));
  }

  /** The sync state of `file`, created at revision 1 on first use. */
  document(file: string): DocSync {
    let d = this.docs.get(file);
    if (!d) {
      const opts = this.debounceMs === undefined ? { timers: this.timers } : { timers: this.timers, debounceMs: this.debounceMs };
      d = new DocSync(file, (m) => this.send(m), opts);
      this.docs.set(file, d);
    }
    return d;
  }

  /** Forgets `file` (closed buffer); its pending edit is dropped. */
  closeDocument(file: string): void {
    this.docs.get(file)?.dispose();
    this.docs.delete(file);
  }

  /** Sends one message; returns its `seq`. */
  send(msg: ClientMsg): number {
    if (msg.kind === 'doc-changed') this.store?.songDocumentChanged?.(msg.body.file, msg.body.doc_revision);
    this.seq += 1;
    if (!this.closed) this.transport.send(encodeClient(this.seq, msg));
    return this.seq;
  }

  /** Sends one message and resolves with the first reply whose `re` is its `seq`. */
  request(msg: ClientMsg, beforeSend?: (seq: number) => void): Promise<ServerEnvelope> {
    if (this.closed) return Promise.reject(new Error('client closed'));
    if (this.pending.size >= MAX_PENDING_REQUESTS) return Promise.reject(new Error('client busy: 64 pending requests'));
    return new Promise((resolve, reject) => {
      // Registered before sending: the wasm transport replies synchronously.
      const seq = ++this.seq;
      this.pending.set(seq, { resolve, reject });
      try {
        beforeSend?.(seq);
        if (!this.closed) this.transport.send(encodeClient(seq, msg));
      } catch (e) { this.pending.delete(seq); reject(e instanceof Error ? e : new Error(String(e))); }
    });
  }

  /** Listens to one server kind, or to every message with `'*'`. */
  on(kind: ServerKind | '*', cb: Listener): () => void {
    if (this.closed) return () => {};
    const list = this.listeners.get(kind) ?? [];
    list.push(cb);
    this.listeners.set(kind, list);
    return () => {
      const l = this.listeners.get(kind);
      if (l) this.listeners.set(kind, l.filter((x) => x !== cb));
    };
  }

  /** Listens to undecodable frames (dropped after reporting). */
  onDecodeError(cb: (e: DecodeError) => void): () => void {
    if (this.closed) return () => {};
    this.errorListeners.push(cb);
    return () => { const i = this.errorListeners.indexOf(cb); if (i >= 0) this.errorListeners.splice(i, 1); };
  }

  clockProbe(pageSend: number): Promise<ServerEnvelope> {
    return this.request({ kind: 'clock-probe', body: { page_send: pageSend } });
  }

  // ------------------------------------------------------------- helpers

  /** Evaluates the whole document or one span of it. */
  eval(file: string, code: string, span?: Span): Promise<ServerEnvelope> {
    const doc = this.document(file);
    doc.flush();
    const body = { file, code, ...doc.stamp(), ...(span ? { span } : {}) };
    return this.request({ kind: 'eval', body });
  }

  /** Prepares a whole song. Only an Applied receipt confirms playback. */
  applySong(file: string, code: string): Promise<ServerEnvelope> {
    const doc = this.document(file);
    doc.flush();
    let request = 0;
    // The wasm host can acknowledge synchronously inside request().
    const response = this.request({ kind: 'apply-song', body: { file, code, ...doc.stamp() } }, (seq) => {
      request = seq;
      this.store?.beginSongApply?.(file, doc.revision, seq, false);
    });
    // Notify after sending so subscriber writes cannot overtake this request.
    this.store?.publishSongApply?.(request);
    return response.catch((error: unknown) => {
        this.store?.cancelSongApply?.(request);
        throw error;
    });
  }

  muteInstrument(epoch: string, selector: InstrumentSelector, muted: boolean): Promise<ServerEnvelope> {
    return this.request({ kind: 'mute-instrument', body: { epoch, selector, muted } });
  }

  hush(): void {
    this.send({ kind: 'hush', body: {} });
  }

  stop(slot: string): void {
    this.send({ kind: 'stop', body: { slot } });
  }

  stopAll(): void {
    this.send({ kind: 'stop-all', body: {} });
  }

  /** A controller write to a tweak site; rate-limited per `(file, id)`. */
  setTweak(file: string, id: number, formGen: number, value: number): void {
    const doc = this.document(file);
    const msg: ClientMsg = {
      kind: 'set-tweak',
      body: { file, id, form_gen: formGen, value, edit_epoch: doc.epoch },
    };
    const key = `${file}\u0000${id}`;
    let slot = this.tweaks.get(key);
    if (!slot) {
      slot = { last: -Infinity, timer: null, queued: null };
      this.tweaks.set(key, slot);
    }
    const t = this.now();
    if (slot.timer === null && t - slot.last >= TWEAK_INTERVAL_MS) {
      slot.last = t;
      this.write(file, msg);
      return;
    }
    slot.queued = msg;
    if (slot.timer === null) {
      const s = slot;
      s.timer = this.timers.set(() => {
        s.timer = null;
        const q = s.queued;
        s.queued = null;
        if (q) {
          s.last = this.now();
          this.write(file, q);
        }
      }, Math.max(0, s.last + TWEAK_INTERVAL_MS - t));
    }
  }

  /** Sends a momentary target, rate-limited per `(file, id)`; null releases immediately. */
  momentary(file: string, id: number, formGen: number, target: number | null, rampMs: number): void {
    const doc = this.document(file);
    const msg: ClientMsg = {
      kind: 'momentary',
      body: {
        file,
        id,
        form_gen: formGen,
        edit_epoch: doc.epoch,
        target,
        ramp_ms: Math.min(10_000, Math.max(0, Math.trunc(rampMs || 0))),
      },
    };
    const key = `${file}\u0000${id}`;
    if (target === null) {
      const slot = this.momentaries.get(key);
      if (slot?.timer !== null && slot?.timer !== undefined) this.timers.clear(slot.timer);
      this.momentaries.delete(key);
      this.write(file, msg);
      return;
    }

    let slot = this.momentaries.get(key);
    if (!slot) {
      slot = { last: -Infinity, timer: null, queued: null };
      this.momentaries.set(key, slot);
    }
    const t = this.now();
    if (slot.timer === null && t - slot.last >= TWEAK_INTERVAL_MS) {
      slot.last = t;
      this.write(file, msg);
      return;
    }
    slot.queued = msg;
    if (slot.timer === null) {
      const s = slot;
      s.timer = this.timers.set(() => {
        s.timer = null;
        const queued = s.queued;
        s.queued = null;
        if (queued) {
          s.last = this.now();
          this.write(file, queued);
        }
      }, Math.max(0, s.last + TWEAK_INTERVAL_MS - t));
    }
  }

  /** A `bind` variable write. */
  setVar(file: string, name: string, value: number | boolean, definingFormGen: number): void {
    const doc = this.document(file);
    this.write(file, {
      kind: 'set-var',
      body: { file, name, value, defining_form_gen: definingFormGen, edit_epoch: doc.epoch },
    });
  }

  /** Directive-mode learn; resolves with `directive-edit`, `stale-binding` or `protocol-error`. */
  learn(file: string, binding: string | number, cc: number, ch?: number): Promise<ServerEnvelope> {
    const doc = this.document(file);
    doc.flush();
    const body = { file, binding, cc, edit_epoch: doc.epoch, ...(ch === undefined ? {} : { ch }) };
    return this.request({ kind: 'learn', body });
  }

  subscribe(body: SubscribeBody): void {
    this.send({ kind: 'subscribe', body });
  }

  manifest(): Promise<ServerEnvelope> {
    return this.request({ kind: 'manifest?', body: {} });
  }

  /** Closes the transport; pending requests reject and queued tweaks drop. */
  close(): void {
    if (this.closed) return;
    this.closed = true;
    for (const s of this.tweaks.values()) {
      if (s.timer !== null) this.timers.clear(s.timer);
    }
    this.tweaks.clear();
    for (const s of this.momentaries.values()) {
      if (s.timer !== null) this.timers.clear(s.timer);
    }
    this.momentaries.clear();
    for (const d of this.docs.values()) d.dispose();
    for (const p of this.pending.values()) p.reject(new Error('client closed'));
    this.pending.clear();
    this.docs.clear();
    this.receiveQueue.length = 0;
    this.listeners.clear();
    this.errorListeners.length = 0;
    this.transport.close();
  }

  // ------------------------------------------------------------ internals

  /** Flushes the document's pending edit, then sends the write. */
  private write(file: string, msg: ClientMsg): void {
    this.docs.get(file)?.flush();
    this.send(msg);
  }

  private receive(text: string): void {
    if (this.closed) return;
    const d = decodeServer(text);
    if (!d.ok) {
      for (const cb of this.errorListeners) cb(d.error);
      return;
    }
    const env = d.env;
    if (telemetry(env)) {
      if (env.kind === 'tempo' || env.kind === 'levels') {
        const i = this.receiveQueue.findIndex((m) => telemetry(m) && m.kind === env.kind);
        const prev = i >= 0 ? this.receiveQueue[i] : undefined;
        if (env.kind === 'tempo' && prev?.kind === 'tempo') {
          const next = env.body.transport;
          const before = prev.body.transport;
          if (next && before && next.epoch === before.epoch && next.sample_time < before.sample_time) return;
        }
        if (i >= 0) { this.receiveQueue.splice(i, 1); this.telemetryCoalesced += 1; }
      }
      if (this.receiveQueue.filter(telemetry).length >= MAX_TELEMETRY_QUEUE) {
        const i = this.receiveQueue.findIndex(telemetry);
        this.receiveQueue.splice(i, 1);
        this.telemetryDropped += 1;
      }
    }
    this.receiveQueue.push(env);
    if (this.receiving) return;
    this.receiving = true;
    try {
      while (!this.closed && this.receiveQueue.length) this.deliver(this.receiveQueue.shift()!);
    } finally { this.receiving = false; }
  }

  private deliver(env: ServerEnvelope): void {
    // Control correlation is independent of telemetry overflow and subscriber failures.
    if (env.re !== undefined) {
      const p = this.pending.get(env.re);
      if (p) { this.pending.delete(env.re); p.resolve(env); }
    }
    try { this.store?.apply(env); } catch (e) { this.onError(e); }
    for (const cb of [...(this.listeners.get(env.kind) ?? []), ...(this.listeners.get('*') ?? [])]) {
      if (this.closed) break;
      try { cb(env); } catch (e) { this.onError(e); }
    }
  }
}
