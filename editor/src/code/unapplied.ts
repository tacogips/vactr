import { ChangeSet, type Text } from '@codemirror/state';
import type { CodeAnnotation, CodeRange, CodeSurfaceUpdate } from '../app/apis';
import type { EvalResultBody } from '../protocol/types';
import type { DocumentSync } from './sync';

interface PendingEdit extends CodeRange { id: number; before: string; revision: number }

/** Pending edits retain their replaced text so undo can restore the applied baseline. */
export class UnappliedEdits {
  private edits: PendingEdit[] = [];
  private nextId = 0;
  constructor(private doc: Text) {}

  update(update: CodeSurfaceUpdate, revision: number): void {
    if (!update.docChanged) return;
    // Apply changes from right to left: old-document offsets stay valid.
    const changes: { from: number; to: number; insert: string }[] = [];
    update.changes.iterChanges((from, to, _a, _b, insert) => changes.push({ from, to, insert: insert.toString() }));
    let doc = this.doc;
    for (const change of changes.reverse()) {
      const touching = this.edits.filter(e => e.from <= change.to && e.to >= change.from);
      const from = Math.min(change.from, ...touching.map(e => e.from));
      const to = Math.max(change.to, ...touching.map(e => e.to));
      let before = doc.sliceString(from, to);
      for (const edit of [...touching].sort((a, b) => b.from - a.from)) {
        before = before.slice(0, edit.from - from) + edit.before + before.slice(edit.to - from);
      }
      const delta = change.insert.length - (change.to - change.from);
      const next = ChangeSet.of({ from: change.from, to: change.to, insert: change.insert }, doc.length).apply(doc);
      this.edits = this.edits.filter(e => !touching.includes(e)).map(e => e.from > change.to
        ? { ...e, from: e.from + delta, to: e.to + delta } : e);
      const end = to + delta;
      const after = next.sliceString(from, end);
      if (after !== before) {
        // Keep unchanged prefixes/suffixes out of the highlight and preserve undo.
        let prefix = 0, suffix = 0;
        while (prefix < Math.min(before.length, after.length) && before[prefix] === after[prefix]) prefix++;
        while (suffix < Math.min(before.length, after.length) - prefix &&
          before[before.length - suffix - 1] === after[after.length - suffix - 1]) suffix++;
        this.edits.push({ from: from + prefix, to: end - suffix,
          before: before.slice(prefix, before.length - suffix), id: ++this.nextId, revision });
      }
      doc = next;
    }
    this.doc = update.state.doc;
    this.edits.sort((a, b) => a.from - b.from);
  }

  snapshot(range: CodeRange): ReadonlySet<number> {
    // New text has no old content to partition: a form can be applied even when
    // it was inserted together with other forms (for example, loading a document).
    for (const position of [range.from, range.to]) {
      this.edits = this.edits.flatMap(e => e.before === '' && e.from < position && e.to > position
        ? [{ ...e, to: position, id: ++this.nextId }, { ...e, from: position, id: ++this.nextId }] : [e]);
    }
    return new Set(this.edits.filter(e => e.from >= range.from && e.to <= range.to).map(e => e.id));
  }

  acknowledge(ids: ReadonlySet<number>, result: EvalResultBody, sync: DocumentSync): void {
    const hasErrors = result.forms.some(f => f.failure) || result.diagnostics.some(d => d.severity === 'error');
    const successful = result.forms.filter(f => !f.failure).flatMap(f => {
      const range = sync.mapWireSpan(f.span, result.doc_revision);
      return range ? [range] : [];
    });
    this.edits = this.edits.flatMap(e => {
      if (!ids.has(e.id)) return [e];
      if (!hasErrors || successful.some(r => e.from >= r.from && e.to <= r.to)) return [];
      // A mixed result can apply individual forms of newly inserted code.
      // Replacements crossing form boundaries stay pending conservatively.
      if (e.before !== '') return [e];
      let remaining = [e];
      for (const r of successful) remaining = remaining.flatMap(part => {
        if (r.to <= part.from || r.from >= part.to) return [part];
        const parts: PendingEdit[] = [];
        if (part.from < r.from) parts.push({ ...part, to: r.from, id: ++this.nextId });
        if (part.to > r.to) parts.push({ ...part, from: r.to, id: ++this.nextId });
        return parts;
      });
      return remaining;
    });
  }

  /** Source-writing controls also evaluate forms without going through EvalController. */
  acknowledgeForms(result: EvalResultBody, sync: DocumentSync): void {
    const successful = result.forms.filter(f => !f.failure).flatMap(f => {
      const range = sync.mapWireSpan(f.span, result.doc_revision);
      return range ? [range] : [];
    });
    this.edits = this.edits.filter(e => e.revision > result.doc_revision ||
      !successful.some(r => e.from >= r.from && e.to <= r.to));
  }

  annotations(doc: Text): CodeAnnotation[] {
    return this.edits.map(e => ({ from: Math.min(e.from, doc.length), to: Math.min(e.to, doc.length), kind: 'unapplied' }));
  }
}
