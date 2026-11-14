import { ChangeSet, Text } from '@codemirror/state';
import { LineTable } from './line-bytes';
import type { Span } from '../protocol/types';

export const HISTORY_LIMIT = 256;
const INDEX_CACHE = 4;
const MAX_PINS = 32;
const MEMO_LIMIT = 32_768;
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
export function trimUndoHistory(value: unknown, budget: number, memo?: WeakMap<object, number>): { value: unknown; bytes: number; reduced: boolean } {
  const h = value as { done: readonly unknown[]; undone: readonly unknown[] };
  if (!h || !Array.isArray(h.done) || !Array.isArray(h.undone)) throw new Error('Unsupported CodeMirror history layout');
  const done = [...h.done], undone = [...h.undone];
  const charge = (event: unknown): number => {
    if (event === null || typeof event !== 'object') return retainedBytes(event);
    const known = memo?.get(event);
    if (known !== undefined) return known;
    const bytes = retainedBytes(event);
    memo?.set(event, bytes);
    return bytes;
  };
  const cost = (events: readonly unknown[]): number => events.reduce<number>((n, event) => n + charge(event), 0);
  let bytes = cost(done) + cost(undone);
  let reduced = false;
  while (bytes > budget && (done.length || undone.length)) {
    const branch = done.length ? done : undone;
    bytes -= charge(branch.shift());
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
export interface Range16 { from: number; to: number }

interface Entry {
  rev: number;
  text: Text;
  changes: ChangeSet | null;
  bytes: number;
}

interface Pin {
  rev: number;
  text: Text;
  composed: ChangeSet;
  composedBytes: number;
  table: LineTable;
  memo: Map<string, Range16>;
  owners: Set<string>;
}

/** True when an individual edit touches `[from, to)`. */
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

function applyChanges(changes: ChangeSet, range: Range16): Range16 | null {
  if (touches(changes, range.from, range.to)) return null;
  if (range.from === range.to) {
    const from = changes.mapPos(range.from, 1);
    return { from, to: from };
  }
  return { from: changes.mapPos(range.from, 1), to: changes.mapPos(range.to, -1) };
}

export class RevisionHistory {
  readonly limit: number;
  private entries: Entry[];
  private readonly indexes = new Map<number, LineTable>();
  private readonly pins = new Map<string, Pin>();
  private readonly pinsByRev = new Map<number, Pin>();
  private currentTable: LineTable | null = null;
  readonly byteLimit: number;
  readonly indexByteLimit: number;
  readonly stats = { pinsEvicted: 0, pinsRefused: 0, lineTableBuilds: 0 };
  private total = 0;
  private reduced = false;

  get retainedBytes(): number { return this.total + this.pinBytes(); }
  get indexBytes(): number {
    let bytes = 0;
    for (const table of this.indexes.values()) bytes += table.bytes;
    for (const pin of this.pinsByRev.values()) bytes += pin.table.bytes + pin.memo.size * 64;
    if (this.currentTable && ![...this.indexes.values()].includes(this.currentTable)
      && ![...this.pinsByRev.values()].some((pin) => pin.table === this.currentTable)) bytes += this.currentTable.bytes;
    return bytes;
  }
  get indexCount(): number {
    return this.indexes.size + this.pinsByRev.size + (this.currentTable && ![...this.indexes.values()].includes(this.currentTable)
      && ![...this.pinsByRev.values()].some((pin) => pin.table === this.currentTable) ? 1 : 0);
  }
  get reducedDepth(): boolean { return this.reduced; }

  trimToBytes(budget: number): void {
    while (this.entries.length > 1 && (this.entries.length > this.limit || this.total + this.pinBytes() > budget)) {
      const dropped = this.entries.shift() as Entry;
      this.total -= dropped.bytes;
      this.indexes.delete(dropped.rev);
      const first = this.entries[0] as Entry;
      this.total -= first.bytes;
      first.bytes = 0;
      first.changes = null;
      this.reduced = true;
    }
    if (this.entries.length === 1) {
      const first = this.entries[0] as Entry;
      this.total -= first.bytes;
      first.bytes = 0;
      first.changes = null;
    }
    while (this.entries.length === 1 && this.total + this.pinBytes() > budget && this.pins.size > 0) this.evictLeastRecentPin();
  }

  constructor(initial: Text, rev = 1, limit = HISTORY_LIMIT, byteLimit = HISTORY_UNDO_BYTES, indexByteLimit = INDEX_BYTES) {
    this.limit = Math.max(1, Math.min(HISTORY_LIMIT, Math.floor(limit)));
    this.byteLimit = Math.max(0, Math.min(HISTORY_UNDO_BYTES, byteLimit));
    this.indexByteLimit = Math.max(0, Math.min(INDEX_BYTES, indexByteLimit));
    this.entries = [{ rev, text: initial, changes: null, bytes: 0 }];
  }

  get current(): number { return (this.entries[this.entries.length - 1] as Entry).rev; }
  get oldest(): number { return (this.entries[0] as Entry).rev; }
  text(rev: number): Text | null { return this.entry(rev)?.text ?? null; }

  /** Supplies the current document's already-maintained line starts. */
  setCurrentStarts(starts: Uint32Array): void {
    const text = this.text(this.current);
    if (text) {
      this.indexes.delete(this.current);
      this.currentTable = new LineTable(text, starts);
    }
  }

  record(rev: number, changes: ChangeSet, text: Text): void {
    if (rev !== this.current + 1) {
      this.entries = [{ rev, text, changes: null, bytes: 0 }];
      this.total = 0;
      this.clearPins();
      this.indexes.clear();
      this.currentTable = null;
      return;
    }
    this.currentTable = null;
    const base = (this.entries[this.entries.length - 1] as Entry).text;
    let bytes = 512;
    changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
      const first = base.lineAt(fromA);
      const last = base.lineAt(toA);
      const lineLength = Math.max(0, last.to - first.from + (last.number < base.lines ? 1 : 0));
      bytes += 2 * inserted.length + 2 * lineLength + 64 * (last.number - first.number + 1);
    });
    for (const pin of this.pinsByRev.values()) {
      pin.composed = pin.composed.compose(changes);
      pin.composedBytes = retainedBytes(pin.composed);
    }
    this.entries.push({ rev, text, changes, bytes });
    this.total += bytes;
    this.trimToBytes(this.byteLimit);
  }

  /** Pin one retained revision for a named consumer. */
  pin(owner: string, rev: number, liveStarts?: Uint32Array): boolean {
    const existing = this.pins.get(owner);
    if (existing?.rev === rev) { this.refreshOwner(owner, existing); return true; }
    if (!this.entry(rev) && !this.pinsByRev.has(rev)) return false;
    if (existing) this.unpin(owner);
    let pin = this.pinsByRev.get(rev);
    if (!pin) {
      const entry = this.entry(rev);
      if (!entry) return false;
      const table = rev === this.current ? this.currentTable ?? (liveStarts ? new LineTable(entry.text, liveStarts) : this.buildLineTable(entry.text))
        : this.indexes.get(rev) ?? this.buildLineTable(entry.text);
      const extraBytes = [...this.indexes.values()].includes(table) || table === this.currentTable ? 0 : table.bytes;
      while (this.indexBytes + extraBytes > this.indexByteLimit && this.indexes.size > 0) this.indexes.delete(this.indexes.keys().next().value as number);
      if (this.indexBytes + extraBytes > this.indexByteLimit) { this.stats.pinsRefused += 1; return false; }
      this.indexes.delete(rev);
      let composed = ChangeSet.empty(entry.text.length);
      const start = this.position(rev);
      const end = this.entries.length - 1;
      for (let i = start + 1; i <= end; i += 1) {
        const next = (this.entries[i] as Entry).changes;
        if (next) composed = composed.compose(next);
      }
      pin = { rev, text: entry.text, composed, composedBytes: retainedBytes(composed), table, memo: new Map(), owners: new Set() };
      this.pinsByRev.set(rev, pin);
    }
    if (this.pins.size >= MAX_PINS) this.evictLeastRecentPin();
    this.pins.set(owner, pin);
    this.pinsByRev.set(rev, pin);
    pin.owners.add(owner);
    return true;
  }

  unpin(owner: string): void {
    const pin = this.pins.get(owner);
    if (!pin) return;
    this.pins.delete(owner);
    pin.owners.delete(owner);
    if (pin.owners.size === 0) this.pinsByRev.delete(pin.rev);
  }

  pinnedRevisions(): readonly number[] { return [...this.pinsByRev.keys()]; }

  lineTable(rev: number): LineTable | null {
    const pinned = this.pinsByRev.get(rev);
    if (pinned) return pinned.table;
    const cached = this.indexes.get(rev);
    if (cached) { this.indexes.delete(rev); this.indexes.set(rev, cached); return cached; }
    const entry = this.entry(rev);
    if (!entry) return null;
    if (rev === this.current && this.currentTable) return this.currentTable;
    const table = this.buildLineTable(entry.text);
    if (table.bytes > this.indexByteLimit) return table;
    while (this.indexBytes + table.bytes > this.indexByteLimit && this.indexes.size > 0) this.indexes.delete(this.indexes.keys().next().value as number);
    if (this.indexBytes + table.bytes <= this.indexByteLimit) {
      this.indexes.set(rev, table);
      while (this.indexes.size > INDEX_CACHE) this.indexes.delete(this.indexes.keys().next().value as number);
    }
    return table;
  }

  mapWireSpan(span: Span, rev: number): Range16 | null {
    const pin = this.pinsByRev.get(rev);
    if (pin) {
      if (!this.validSpan(span, pin.table.byteLength)) return null;
      const key = `${span.start}:${span.end}`;
      let range = pin.memo.get(key);
      if (!range) {
        range = pin.table.spanToUtf16(span);
        if (pin.memo.size >= MEMO_LIMIT) pin.memo.delete(pin.memo.keys().next().value as string);
        pin.memo.set(key, range);
      }
      if (rev === this.current) return range;
      return applyChanges(pin.composed, range);
    }
    const table = rev === this.current ? this.currentTable : this.lineTable(rev);
    if (!table || !this.validSpan(span, table.byteLength)) return null;
    return this.mapSpan(table.spanToUtf16(span), rev);
  }

  mapSpan(span: Range16, fromRev: number, toRev: number = this.current): Range16 | null {
    const start = this.position(fromRev), end = this.position(toRev);
    if (start < 0 || end < 0 || end < start) return null;
    const text = (this.entries[start] as Entry).text;
    if (!Number.isInteger(span.from) || !Number.isInteger(span.to) || span.from < 0 || span.to < span.from || span.to > text.length) return null;
    let range = span;
    for (let i = start + 1; i <= end; i += 1) {
      const changes = (this.entries[i] as Entry).changes;
      if (!changes) return null;
      const mapped = applyChanges(changes, range);
      if (!mapped) return null;
      range = mapped;
    }
    return range;
  }

  private validSpan(span: Span, byteLength: number): boolean {
    return Number.isInteger(span.start) && Number.isInteger(span.end) && span.start >= 0 && span.end >= span.start && span.end <= byteLength;
  }
  private buildLineTable(text: Text): LineTable { this.stats.lineTableBuilds += 1; return LineTable.build(text); }
  private pinBytes(): number { let bytes = 0; for (const pin of this.pinsByRev.values()) bytes += pin.composedBytes; return bytes; }
  private refreshOwner(owner: string, pin: Pin): void { this.pins.delete(owner); this.pins.set(owner, pin); }
  private evictLeastRecentPin(): void {
    const owner = this.pins.keys().next().value as string | undefined;
    if (owner !== undefined) { this.unpin(owner); this.stats.pinsEvicted += 1; }
  }
  private clearPins(): void { this.pins.clear(); this.pinsByRev.clear(); }
  private position(rev: number): number {
    const i = rev - this.oldest;
    if (!Number.isInteger(i) || i < 0 || i >= this.entries.length) return -1;
    return (this.entries[i] as Entry).rev === rev ? i : -1;
  }
  private entry(rev: number): Entry | null { const i = this.position(rev); return i < 0 ? null : (this.entries[i] as Entry); }
}
