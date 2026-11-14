// RecordingTransport (design 15.1.4): records every client envelope and
// replays scripted server envelopes, synchronously through `respond` or
// queued through `push` + `deliver()`.

import { decodeClient, encodeServer } from '../../src/protocol/envelope';
import type { Transport } from '../../src/protocol/transport';
import type { ClientEnvelope, ClientKind, ServerMsg } from '../../src/protocol/types';

/** A scripted server message; `re` answers a client `seq`. */
export type Scripted = ServerMsg & { re?: number };

export class RecordingTransport implements Transport {
  readonly sent: string[] = [];
  closed = false;
  private readonly listeners: ((text: string) => void)[] = [];
  private queued: string[] = [];
  private responder: ((env: ClientEnvelope) => Scripted[]) | null = null;
  private serverSeq = 0;

  send(text: string): void {
    this.sent.push(text);
    if (this.responder) {
      for (const m of this.responder(this.decode(text))) this.emit(m);
    }
  }

  onText(cb: (text: string) => void): void {
    this.listeners.push(cb);
  }

  close(): void {
    this.closed = true;
  }

  /** Replies synchronously to each client envelope. */
  respond(fn: ((env: ClientEnvelope) => Scripted[]) | null): void {
    this.responder = fn;
  }

  /** Queues a server message for `deliver()`. */
  push(msg: Scripted): void {
    this.queued.push(this.encode(msg));
  }

  /** Delivers the queued messages in order; returns how many. */
  deliver(): number {
    const q = this.queued;
    this.queued = [];
    for (const t of q) this.emitRaw(t);
    return q.length;
  }

  /** Delivers one server message now. */
  emit(msg: Scripted): void {
    this.emitRaw(this.encode(msg));
  }

  emitRaw(text: string): void {
    for (const cb of this.listeners) cb(text);
  }

  /** Every client envelope sent so far. */
  get envelopes(): ClientEnvelope[] {
    return this.sent.map((t) => this.decode(t));
  }

  kinds(): ClientKind[] {
    return this.envelopes.map((e) => e.kind);
  }

  /** The envelopes of one kind. */
  of<K extends ClientKind>(kind: K): Extract<ClientEnvelope, { kind: K }>[] {
    return this.envelopes.filter((e): e is Extract<ClientEnvelope, { kind: K }> => e.kind === kind);
  }

  clear(): void {
    this.sent.length = 0;
  }

  private encode(msg: Scripted): string {
    this.serverSeq += 1;
    const env = { v: 1, seq: this.serverSeq, ...msg } as Parameters<typeof encodeServer>[0];
    return encodeServer(env);
  }

  private decode(text: string): ClientEnvelope {
    const d = decodeClient(text);
    if (!d.ok) throw new Error(`bad client envelope: ${d.error.code} ${d.error.message}`);
    return d.env;
  }
}
