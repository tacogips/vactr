// WebSocket transport to `vactrol serve` (command.md: text frames on
// `ws://127.0.0.1:<port>/session?token=<hex>`). The token lives only in the
// URL the user pasted: it is never written to localStorage, sessionStorage
// or any other store. Frames sent before the socket opens are queued.

import type { Transport } from './transport';

/** The subset of `WebSocket` this transport uses (tests pass a fake). */
export interface SocketLike {
  readyState: number;
  send(data: string): void;
  close(): void;
  onopen: ((ev: Event) => void) | null;
  onmessage: ((ev: MessageEvent) => void) | null;
  onclose: ((ev: CloseEvent) => void) | null;
  onerror: ((ev: Event) => void) | null;
}

export type SocketFactory = (url: string) => SocketLike;

const OPEN = 1;

export class SocketTransport implements Transport {
  private readonly ws: SocketLike;
  private readonly listeners: ((text: string) => void)[] = [];
  private readonly closeListeners: ((reason: string) => void)[] = [];
  private queue: string[] = [];
  private closed = false;

  constructor(url: string, factory: SocketFactory = (u) => new WebSocket(u)) {
    this.ws = factory(url);
    this.ws.onopen = () => {
      const q = this.queue;
      this.queue = [];
      for (const t of q) this.ws.send(t);
    };
    this.ws.onmessage = (ev) => {
      if (this.closed || typeof ev.data !== 'string') return;
      for (const cb of this.listeners) cb(ev.data);
    };
    this.ws.onclose = (ev) => {
      this.closed = true;
      for (const cb of this.closeListeners) cb(`closed (${ev.code})`);
    };
    this.ws.onerror = () => {
      for (const cb of this.closeListeners) cb('connection error');
    };
  }

  send(text: string): void {
    if (this.closed) return;
    if (this.ws.readyState === OPEN) this.ws.send(text);
    else this.queue.push(text);
  }

  onText(cb: (text: string) => void): void {
    this.listeners.push(cb);
  }

  /** Connection loss or error, for the status line. */
  onClose(cb: (reason: string) => void): void {
    this.closeListeners.push(cb);
  }

  close(): void {
    this.closed = true;
    this.queue = [];
    this.ws.close();
  }
}
