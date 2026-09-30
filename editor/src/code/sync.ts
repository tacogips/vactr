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

import type { ChangeSet, Extension, Text } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import type { CodeSurface } from './surface';
import type { DocSync } from '../protocol/document';
import type { Span } from '../protocol/types';
import { Utf8Index, type Utf16Change } from '../protocol/utf8';
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

  constructor(doc: DocSync, initial: Text, historyLimit?: number, byteLimit?: number, indexByteLimit?: number) {
    this.doc = doc;
    this.history = new RevisionHistory(initial, doc.revision, historyLimit, byteLimit, indexByteLimit);
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
    const recorded = this.history.text(this.history.current);
    // The history's current text is the base unless the view was reset.
    const base =
      recorded === tr.startState.doc
        ? (this.history.index(this.history.current) as Utf8Index)
        : new Utf8Index(tr.startState.doc.toString());
    const list: Utf16Change[] = [];
    tr.changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
      list.push({ from: fromA, to: toA, insert: inserted.toString() });
    });
    const { changes, dirty } = base.changes(list);
    this.doc.edit(changes, dirty);
    this.history.record(this.doc.revision, tr.changes, tr.newDoc);
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

  /** Legacy compatibility until CE-JOIN; never install on a headless surface. */
  extension(): Extension {
    return EditorView.updateListener.of((update) => {
      for (const tr of update.transactions) this.apply(tr);
    });
  }
}
