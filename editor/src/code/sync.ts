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
import { LineBytes } from './line-bytes';
import { RevisionHistory, type Range16 } from './history';

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
    this.history.setCurrentStarts(this.bytes.starts());
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
    this.history.setCurrentStarts(this.bytes.starts());
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
    const text = this.history.text(this.history.current);
    if (!text) return { start: 0, end: 0 };
    return { start: this.bytes.toByte(text, from), end: this.bytes.toByte(text, to) };
  }

  pin(owner: string, rev: number): boolean { return this.history.pin(owner, rev, rev === this.revision ? this.bytes.starts() : undefined); }
  unpin(owner: string): void { this.history.unpin(owner); }

  /** Headless observer binding: the surface has already applied and recorded the transaction. */
  bind(surface: CodeSurface, cb: (rev: number) => void): () => void {
    if (surface.sync !== this) throw new Error('Surface belongs to a different DocumentSync');
    return surface.subscribe((update) => { if (update.docChanged) cb(this.revision); });
  }

}
