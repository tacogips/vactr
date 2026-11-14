// Document sync from CodeMirror (design 14.4, 15.1.4 "Revisions and
// epochs", "Offsets").
//
// Per document-changing transaction, SYNCHRONOUSLY inside the dispatch:
// 1. the UTF-16 change set becomes byte changes against the base text,
//    plus the new-revision dirty byte spans (inserted ranges and a
//    zero-length span at each pure deletion), through `Utf8Index`;
// 2. `DocSync.edit` bumps the revision and the edit epoch and re-arms the
//    200 ms `doc-changed` debounce (the client flushes it before writes);
// 3. the change set is recorded in the `RevisionHistory` under the new
//    revision, so any span of a kept revision maps to the current text.

import type { ChangeSet, Text } from '@codemirror/state';
import type { CodeSurface } from './surface';
import type { DocSync } from '../protocol/document';
import type { ByteChange, Span } from '../protocol/types';
import { Utf8Index, utf8Length } from '../protocol/utf8';
import { RevisionHistory, type Range16 } from './history';

const BYTE_SCAN_CHUNK = 16 * 1024;

function utf8LengthRange(doc: Text, from: number, to: number): number {
  let bytes = 0;
  let pos = from;
  let limit = to;
  if (limit > from && limit < doc.length) {
    const last = doc.sliceString(limit - 1, limit).charCodeAt(0);
    const next = doc.sliceString(limit, limit + 1).charCodeAt(0);
    if (last >= 0xd800 && last <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) limit -= 1;
  }
  while (pos < limit) {
    let end = Math.min(limit, pos + BYTE_SCAN_CHUNK);
    if (end < limit) {
      const last = doc.sliceString(end - 1, end).charCodeAt(0);
      const next = doc.sliceString(end, end + 1).charCodeAt(0);
      if (last >= 0xd800 && last <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) end -= 1;
    }
    bytes += utf8Length(doc.sliceString(pos, end));
    pos = end;
  }
  return bytes;
}

/** UTF-8 line lengths for the edit path; only changed line text is rescanned. */
class LineBytes {
  private readonly lengths: number[];
  private prefix: number[];
  constructor(private current: Text) {
    this.lengths = Array.from({ length: current.lines }, (_, index) => this.lineLength(current, index + 1));
    this.prefix = [];
    this.rebuildPrefix(0);
  }

  private lineLength(doc: Text, number: number): number {
    const line = doc.line(number);
    return utf8LengthRange(doc, line.from, line.to);
  }

  matches(doc: Text): boolean { return doc === this.current; }

  private rebuildPrefix(from: number): void {
    if (from === 0) this.prefix = [0];
    else this.prefix.length = from + 1;
    for (let i = from; i < this.lengths.length; i += 1) {
      this.prefix[i + 1] = (this.prefix[i] ?? 0) + (this.lengths[i] ?? 0) + 1;
    }
    if (this.lengths.length > 0) this.prefix[this.lengths.length] = (this.prefix[this.lengths.length] ?? 0) - 1;
  }

  toByte(doc: Text, pos: number): number {
    if (doc !== this.current) throw new Error('Line byte index does not match document');
    const bounded = Math.max(0, Math.min(doc.length, Math.floor(pos)));
    const line = doc.lineAt(bounded);
    const index = line.number - 1;
    return (this.prefix[index] ?? 0) + utf8LengthRange(doc, line.from, bounded);
  }

  insertLength(inserted: Text): number {
    let total = Math.max(0, inserted.lines - 1);
    for (let number = 1; number <= inserted.lines; number += 1) total += this.lineLength(inserted, number);
    return total;
  }

  update(changes: ChangeSet, base: Text, next: Text): void {
    const edits: { fromA: number; toA: number; fromB: number; toB: number }[] = [];
    changes.iterChanges((fromA, toA, fromB, toB) => edits.push({ fromA, toA, fromB, toB }));
    for (const edit of edits.reverse()) {
      const startA = base.lineAt(edit.fromA).number - 1;
      const endA = base.lineAt(edit.toA).number;
      const startB = next.lineAt(edit.fromB).number - 1;
      const endB = next.lineAt(edit.toB).number;
      const replacement = Array.from({ length: endB - startB }, (_, offset) => this.lineLength(next, startB + offset + 1));
      this.lengths.splice(startA, endA - startA, ...replacement);
      this.rebuildPrefix(startA);
    }
    this.current = next;
  }
}

/** The part of a CodeMirror `Transaction` the sync reads. */
export interface DocTransaction {
  docChanged: boolean;
  changes: ChangeSet;
  startState: { doc: Text };
  newDoc: Text;
}

export class DocumentSync {
  readonly doc: DocSync;
  readonly history: RevisionHistory;
  private readonly listeners: ((rev: number) => void)[] = [];
  private bytes: LineBytes;

  constructor(doc: DocSync, initial: Text, historyLimit?: number, byteLimit?: number, indexByteLimit?: number) {
    this.doc = doc;
    this.history = new RevisionHistory(initial, doc.revision, historyLimit, byteLimit, indexByteLimit);
    this.bytes = new LineBytes(initial);
  }

  get file(): string {
    return this.doc.file;
  }

  /** The current document revision. */
  get revision(): number {
    return this.doc.revision;
  }

  /** Applies one transaction; a transaction that changes nothing is ignored. */
  apply(tr: DocTransaction): void {
    if (!tr.docChanged) return;
    const base = tr.startState.doc;
    if (!this.bytes.matches(base)) this.bytes = new LineBytes(base);
    const list: ByteChange[] = [];
    tr.changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
      const from = this.bytes.toByte(base, fromA);
      const to = this.bytes.toByte(base, toA);
      const insertLen = this.bytes.insertLength(inserted);
      list.push({ from, to, insert_len: insertLen });
    });
    const changes: ByteChange[] = list;
    const dirty: Span[] = [];
    let delta = 0;
    for (const change of changes) {
      const start = change.from + delta;
      dirty.push({ start, end: start + change.insert_len });
      delta += change.insert_len - (change.to - change.from);
    }
    this.doc.edit(changes, dirty);
    this.history.record(this.doc.revision, tr.changes, tr.newDoc);
    this.bytes.update(tr.changes, base, tr.newDoc);
    for (const cb of [...this.listeners]) cb(this.doc.revision);
  }

  /** Called after each recorded revision. */
  onChange(cb: (rev: number) => void): () => void {
    this.listeners.push(cb);
    return () => {
      const i = this.listeners.indexOf(cb);
      if (i >= 0) this.listeners.splice(i, 1);
    };
  }

  /** Bytes at revision `rev` -> the current UTF-16 range, or null when gone. */
  mapWireSpan(span: Span, rev: number): Range16 | null {
    return this.history.mapWireSpan(span, rev);
  }

  /** A current UTF-16 range -> a byte span of the current revision. */
  toWireSpan(from: number, to: number): Span {
    const idx = this.history.index(this.history.current) as Utf8Index;
    return idx.spanToBytes(from, to);
  }

  /** Headless observer binding: the surface has already applied and recorded the transaction. */
  bind(surface: CodeSurface, cb: (rev: number) => void): () => void {
    if (surface.sync !== this) throw new Error('Surface belongs to a different DocumentSync');
    return surface.subscribe((update) => { if (update.docChanged) cb(this.revision); });
  }

}
