// Per-document sync state (design 14.4, 15.1.4 "Revisions and epochs").
//
// `revision` starts at 1 on open and `epoch` at 0. Every local edit bumps
// both SYNCHRONOUSLY, composes its byte changes into the pending
// `doc-changed` (all offsets stay in `base_revision` bytes), and re-arms a
// 200 ms debounce. `flush()` sends the pending `doc-changed` at once; the
// client calls it before every write for the document (14.4 rule 1).

import type { ByteChange, ClientMsg, Span } from './types';

export const DOC_DEBOUNCE_MS = 200;

export interface Timers {
  set(fn: () => void, ms: number): unknown;
  clear(handle: unknown): void;
}

export const defaultTimers: Timers = {
  set: (fn, ms) => setTimeout(fn, ms),
  clear: (h) => clearTimeout(h as ReturnType<typeof setTimeout>),
};

export interface DocStamp {
  doc_revision: number;
  edit_epoch: number;
}

function changeDelta(c: ByteChange): number {
  return c.insert_len - (c.to - c.from);
}

/**
 * Composes `first` (base -> mid) with `second` (mid -> new) into one change
 * list in base bytes. Both inputs are sorted and non-overlapping. Changes
 * that overlap or touch in mid coordinates merge into one change.
 */
export function composeChanges(first: readonly ByteChange[], second: readonly ByteChange[]): ByteChange[] {
  interface Item {
    s: number;
    e: number;
    delta: number;
    fromFirst: boolean;
  }
  const items: Item[] = [];
  let d = 0;
  for (const c of first) {
    const s = c.from + d;
    items.push({ s, e: s + c.insert_len, delta: changeDelta(c), fromFirst: true });
    d += changeDelta(c);
  }
  for (const c of second) {
    items.push({ s: c.from, e: c.to, delta: changeDelta(c), fromFirst: false });
  }
  items.sort((a, b) => a.s - b.s || a.e - b.e);

  const out: ByteChange[] = [];
  let dFirst = 0;
  let i = 0;
  while (i < items.length) {
    const head = items[i] as Item;
    const ms = head.s;
    let me = head.e;
    let gFirst = 0;
    let gSecond = 0;
    while (i < items.length && (items[i] as Item).s <= me) {
      const it = items[i] as Item;
      me = Math.max(me, it.e);
      if (it.fromFirst) gFirst += it.delta;
      else gSecond += it.delta;
      i += 1;
    }
    out.push({ from: ms - dFirst, to: me - dFirst - gFirst, insert_len: me - ms + gSecond });
    dFirst += gFirst;
  }
  return out;
}

/** Maps a mid-revision offset through `changes` into the new revision. */
function mapPoint(x: number, changes: readonly ByteChange[], right: boolean): number {
  let delta = 0;
  for (const c of changes) {
    if (c.from < x && x < c.to) {
      const start = c.from + delta;
      return right ? start + c.insert_len : start;
    }
    if (c.to < x || (c.to === x && (c.from < x || right))) delta += changeDelta(c);
    else break;
  }
  return x + delta;
}

/** Sorts spans and merges overlapping or touching ones. */
export function mergeSpans(spans: readonly Span[]): Span[] {
  const sorted = [...spans].sort((a, b) => a.start - b.start || a.end - b.end);
  const out: Span[] = [];
  for (const s of sorted) {
    const last = out[out.length - 1];
    if (last && s.start <= last.end) last.end = Math.max(last.end, s.end);
    else out.push({ start: s.start, end: s.end });
  }
  return out;
}

export interface DocSyncOptions {
  debounceMs?: number;
  timers?: Timers;
}

export class DocSync {
  readonly file: string;
  private readonly send: (msg: ClientMsg) => void;
  private readonly debounceMs: number;
  private readonly timers: Timers;
  private rev = 1;
  private ep = 0;
  private base = 1;
  private pending: ByteChange[] = [];
  private pendingDirty: Span[] = [];
  private timer: unknown = null;

  constructor(file: string, send: (msg: ClientMsg) => void, opts: DocSyncOptions = {}) {
    this.file = file;
    this.send = send;
    this.debounceMs = opts.debounceMs ?? DOC_DEBOUNCE_MS;
    this.timers = opts.timers ?? defaultTimers;
  }

  /** The current document revision. */
  get revision(): number {
    return this.rev;
  }

  /** The current edit epoch. */
  get epoch(): number {
    return this.ep;
  }

  /** The revision the session last heard of. */
  get baseRevision(): number {
    return this.base;
  }

  get hasPending(): boolean {
    return this.rev !== this.base;
  }

  /** The revision and epoch a write produced now is stamped with. */
  stamp(): DocStamp {
    return { doc_revision: this.rev, edit_epoch: this.ep };
  }

  /**
   * One local edit: `changes` in the previous revision's bytes, `dirty` in
   * the new revision's bytes.
   */
  edit(changes: readonly ByteChange[], dirty: readonly Span[]): void {
    this.ep += 1;
    this.rev += 1;
    const sorted = [...changes].sort((a, b) => a.from - b.from);
    if (this.pending.length === 0 && this.pendingDirty.length === 0) {
      this.pending = sorted.map((c) => ({ ...c }));
      this.pendingDirty = mergeSpans(dirty);
    } else {
      const moved = this.pendingDirty.map((s) => ({
        start: mapPoint(s.start, sorted, false),
        end: mapPoint(s.end, sorted, true),
      }));
      this.pending = composeChanges(this.pending, sorted);
      this.pendingDirty = mergeSpans([...moved, ...dirty]);
    }
    this.arm();
  }

  /** Sends the pending `doc-changed` now; false when nothing is pending. */
  flush(): boolean {
    this.disarm();
    if (!this.hasPending) return false;
    const body = {
      file: this.file,
      doc_revision: this.rev,
      base_revision: this.base,
      changes: this.pending,
      dirty: this.pendingDirty,
      edit_epoch: this.ep,
    };
    this.base = this.rev;
    this.pending = [];
    this.pendingDirty = [];
    this.send({ kind: 'doc-changed', body });
    return true;
  }

  /** Cancels the debounce without sending. */
  dispose(): void {
    this.disarm();
  }

  private arm(): void {
    this.disarm();
    this.timer = this.timers.set(() => {
      this.timer = null;
      this.flush();
    }, this.debounceMs);
  }

  private disarm(): void {
    if (this.timer !== null) {
      this.timers.clear(this.timer);
      this.timer = null;
    }
  }
}
