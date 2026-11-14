// The revision history (design 14.4 "doc_revision mapping", 15.1.5
// "Highlighting"): the last `HISTORY_LIMIT` revisions of one document, each
// with its text and the CodeMirror change set that produced it. A span of
// any kept revision maps to any later one by composing the change sets. A
// span that a change touches, or a revision older than the history, maps
// to null: the caller drops it.
//
// Texts are CodeMirror `Text` ropes (structure shared between revisions);
// the per-revision `Utf8Index` a wire span needs is built on demand and
// cached for the few most recent revisions asked for. Every UTF-8 <-> UTF-16
// conversion goes through `Utf8Index`.

import { Text, type ChangeSet } from '@codemirror/state';
import { Utf8Index } from '../protocol/utf8';
import type { Span } from '../protocol/types';

export const HISTORY_LIMIT = 256;

const INDEX_CACHE = 4;
export const HISTORY_UNDO_BYTES = 32 * 1024 * 1024;
export const INDEX_BYTES = 8 * 1024 * 1024;

/** Conservative retained-heap charge; count copies rather than assuming rope sharing. */
export function retainedBytes(value: unknown, seen = new Set<object>()): number {
  if (typeof value === 'string') return 32 + value.length * 2;
  if (value === null || value === undefined) return 0;
  if (typeof value !== 'object') return 16;
  if (seen.has(value)) return 0;
  seen.add(value);
  if (value instanceof Text) return 128 + value.length * 2 + value.lines * 64;
  if (ArrayBuffer.isView(value)) return 128 + value.byteLength;
  if (value instanceof ArrayBuffer) return 128 + value.byteLength;
  if (value instanceof Map) return 128 + [...value].reduce((n, [k, v]) => n + retainedBytes(k, seen) + retainedBytes(v, seen) + 64, 0);
  if (value instanceof Set) return 128 + [...value].reduce((n, v) => n + retainedBytes(v, seen) + 32, 0);
  return 128 + Object.values(value).reduce<number>((n, v) => n + 16 + retainedBytes(v, seen), 0);
}

/** Adapter for pinned @codemirror/commands 6.11.1; no serialized effects are lost. */
export function trimUndoHistory(value: unknown, budget: number): { value: unknown; bytes: number; reduced: boolean } {
  const h = value as { done: readonly unknown[]; undone: readonly unknown[] };
  if (!h || !Array.isArray(h.done) || !Array.isArray(h.undone)) throw new Error('Unsupported CodeMirror history layout');
  const done = [...h.done], undone = [...h.undone];
  const cost = (events: readonly unknown[]): number => events.reduce<number>((n, event) => n + retainedBytes(event), 0);
  let bytes = cost(done) + cost(undone);
  let reduced = false;
  while (bytes > budget && (done.length || undone.length)) {
    // Oldest complete groups are at the front on both branches.
    const branch = done.length ? done : undone;
    bytes -= retainedBytes(branch.shift());
    reduced = true;
  }
  if (!reduced) return { value, bytes, reduced };
  const replacement = Object.create(Object.getPrototypeOf(value));
  Object.defineProperties(replacement, Object.getOwnPropertyDescriptors(value));
  replacement.done = done;
  replacement.undone = undone;
  return { value: replacement, bytes, reduced };
}

/** A UTF-16 range of one revision. */
export interface Range16 {
  from: number;
  to: number;
}

interface Entry {
  rev: number;
  text: Text;
  /** The change set from `rev - 1` to `rev` (null for the oldest kept). */
  changes: ChangeSet | null;
}

/**
 * True when a change of `changes` touches `[from, to)`: it overlaps the
 * span's interior, or inserts strictly inside it. A change that only
 * abuts the span (typing right before or after it) does not touch it. A
 * zero-length span is touched by a change that strictly covers it.
 */
export function touches(changes: ChangeSet, from: number, to: number): boolean {
  let hit = false;
  changes.iterChangedRanges((fromA, toA) => {
    if (hit) return;
    if (from === to) hit = fromA < from && from < toA;
    else if (fromA === toA) hit = from < fromA && fromA < to;
    else hit = fromA < to && toA > from;
  });
  return hit;
}

export class RevisionHistory {
  readonly limit: number;
  private entries: Entry[];
  private readonly indexes = new Map<number, Utf8Index>();
  readonly byteLimit: number;
  readonly indexByteLimit: number;
  private reduced = false;

  get retainedBytes(): number {
    return this.entries.reduce((n, e, i) => n + (i < this.entries.length - 1 ? retainedBytes(e.text) : 0) + retainedBytes(e.changes), 0);
  }
  get indexBytes(): number {
    return [...this.indexes.values()].reduce((n, idx) => n + 128 + idx.text.length * 2 + (idx.text.length + 1) * 4, 0);
  }
  get indexCount(): number { return this.indexes.size; }
  get reducedDepth(): boolean { return this.reduced; }

  trimToBytes(budget: number): void {
    while (this.entries.length > 1 && (this.entries.length > this.limit || this.retainedBytes > budget)) {
      const dropped = this.entries.shift() as Entry;
      this.indexes.delete(dropped.rev);
      (this.entries[0] as Entry).changes = null;
      this.reduced = true;
    }
    if (this.entries.length === 1) (this.entries[0] as Entry).changes = null;
  }

  constructor(initial: Text, rev = 1, limit = HISTORY_LIMIT, byteLimit = HISTORY_UNDO_BYTES, indexByteLimit = INDEX_BYTES) {
    this.limit = Math.max(1, Math.min(HISTORY_LIMIT, Math.floor(limit)));
    this.byteLimit = Math.max(0, Math.min(HISTORY_UNDO_BYTES, byteLimit));
    this.indexByteLimit = Math.max(0, Math.min(INDEX_BYTES, indexByteLimit));
    this.entries = [{ rev, text: initial, changes: null }];
  }

  /** The newest revision. */
  get current(): number {
    return (this.entries[this.entries.length - 1] as Entry).rev;
  }

  /** The oldest revision still mappable. */
  get oldest(): number {
    return (this.entries[0] as Entry).rev;
  }

  /** The text of `rev`, or null when it is not kept. */
  text(rev: number): Text | null {
    return this.entry(rev)?.text ?? null;
  }

  /** Records revision `rev` (the next one), produced by `changes`. */
  record(rev: number, changes: ChangeSet, text: Text): void {
    if (rev !== this.current + 1) {
      // A gap would make every older mapping wrong: restart the history.
      this.entries = [{ rev, text, changes: null }];
      this.indexes.clear();
      return;
    }
    this.entries.push({ rev, text, changes });
    this.trimToBytes(this.byteLimit);
  }

  /** The `Utf8Index` of `rev`, or null when it is not kept. */
  index(rev: number): Utf8Index | null {
    const cached = this.indexes.get(rev);
    if (cached) {
      // Refresh the LRU position.
      this.indexes.delete(rev);
      this.indexes.set(rev, cached);
      return cached;
    }
    const e = this.entry(rev);
    if (!e) return null;
    const idx = new Utf8Index(e.text.toString());
    const bytes = 128 + idx.text.length * 2 + (idx.text.length + 1) * 4;
    // Oversized conversions remain available, but are never retained in the cache.
    if (bytes > this.indexByteLimit) return idx;
    this.indexes.set(rev, idx);
    while (this.indexes.size > INDEX_CACHE || this.indexBytes > this.indexByteLimit) {
      const oldest = this.indexes.keys().next().value as number;
      this.indexes.delete(oldest);
    }
    return idx;
  }

  /**
   * Maps a UTF-16 span of `fromRev` to `toRev` (default: the current one).
   * Null when a change on the way touches the span, or either revision is
   * outside the history, or `toRev < fromRev`.
   */
  mapSpan(span: Range16, fromRev: number, toRev: number = this.current): Range16 | null {
    const start = this.position(fromRev);
    const end = this.position(toRev);
    if (start < 0 || end < 0 || end < start) return null;
    let from = span.from;
    let to = span.to;
    const text = (this.entries[start] as Entry).text;
    if (!Number.isInteger(from) || !Number.isInteger(to) || from < 0 || to < from || to > text.length) return null;
    for (let i = start + 1; i <= end; i += 1) {
      const changes = (this.entries[i] as Entry).changes;
      if (!changes) return null;
      if (touches(changes, from, to)) return null;
      if (from === to) {
        from = changes.mapPos(from, 1);
        to = from;
      } else {
        from = changes.mapPos(from, 1);
        to = changes.mapPos(to, -1);
      }
    }
    return { from, to };
  }

  /** A wire (UTF-8 byte) span of `rev` mapped to the current UTF-16 range. */
  mapWireSpan(span: Span, rev: number): Range16 | null {
    const idx = this.index(rev);
    if (!idx) return null;
    if (!Number.isInteger(span.start) || !Number.isInteger(span.end) || span.start < 0 || span.end < span.start || span.end > idx.byteLength) return null;
    return this.mapSpan(idx.spanToUtf16(span), rev);
  }

  private position(rev: number): number {
    const i = rev - this.oldest;
    if (!Number.isInteger(i) || i < 0 || i >= this.entries.length) return -1;
    return (this.entries[i] as Entry).rev === rev ? i : -1;
  }

  private entry(rev: number): Entry | null {
    const i = this.position(rev);
    return i < 0 ? null : (this.entries[i] as Entry);
  }
}
