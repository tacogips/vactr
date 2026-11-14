import { Text, type ChangeSet, type EditorState } from '@codemirror/state';
import type { CodeAnnotation } from '../app/apis';
import { styleSpans, treeEditFromDocs, type ParsedVact, type VactSyntax } from './syntax-core';
import { LineStream, vactParser } from './language';

export { CAPTURE_CLASSES, createVactSyntax, loadVactSyntax } from './syntax-core';
export type { ParsedVact, StyleSpan, SyntaxCapture, SyntaxLoader, VactSyntax } from './syntax-core';

export interface SpanResult { spans: CodeAnnotation[]; truncated: boolean }
export interface SpanProvider {
  spans(state: EditorState, from: number, to: number, limit: number): SpanResult;
  noteChanges?(changes: ChangeSet, state: EditorState): void;
  dispose?(): void;
}

function bounded(spans: readonly { from: number; to: number; className: string }[], from: number, to: number, limit: number): SpanResult {
  const visible = spans.filter((span) => span.to > from && span.from < to);
  const cap = Math.max(0, limit);
  return { spans: visible.slice(0, cap).map((span) => ({ from: Math.max(from, span.from), to: Math.min(to, span.to), kind: 'syntax', className: span.className })), truncated: visible.length > cap };
}

export class FallbackSpans implements SpanProvider {
  private readonly tokenizer = new FallbackTokenizerCache();
  private lastDoc: Text | null = null;
  noteChanges(changes: ChangeSet, state: EditorState): void {
    this.tokenizer.noteChanges(changes, this.lastDoc, state.doc);
    this.lastDoc = state.doc;
  }
  spans(state: EditorState, from: number, to: number, limit: number): SpanResult {
    const spans = this.tokenizer.spans(state.doc, from, to).map((span) => ({ from: span.from, to: span.to, className: span.className }));
    this.lastDoc = state.doc;
    return bounded(spans, from, to, limit);
  }
}

class FallbackTokenizerCache {
  private current: Text | null = null;
  private startStates: { inString: boolean }[] = [{ inString: false }];
  private validThrough = 0;

  noteChanges(changes: ChangeSet, base: Text | null, next: Text): void {
    if (changes.empty) { this.current = next; return; }
    if (!base || this.current !== base) {
      this.startStates = [{ inString: false }];
      this.validThrough = 0;
    } else {
      let first = base.lines;
      changes.iterChanges((fromA) => { first = Math.min(first, base.lineAt(fromA).number - 1); });
      this.startStates.length = Math.min(this.startStates.length, first + 1);
      this.validThrough = Math.min(this.validThrough, first);
    }
    this.current = next;
  }

  spans(doc: Text, from: number, to: number): { from: number; to: number; className: string }[] {
    if (this.current !== doc) {
      this.current = doc;
      this.startStates = [{ inString: false }];
      this.validThrough = 0;
    }
    const first = doc.lineAt(Math.max(0, Math.min(doc.length, from))).number - 1;
    const last = doc.lineAt(Math.max(from, Math.min(doc.length, to > from ? to - 1 : to))).number - 1;
    while (this.validThrough < first) this.advance(doc, this.validThrough);
    const spans: { from: number; to: number; className: string }[] = [];
    for (let index = first; index <= last; index += 1) {
      const line = doc.line(index + 1);
      const state = { ...(this.startStates[index] ?? { inString: false }) };
      this.tokenizeLine(line.text, line.from, state, spans);
      if (index === this.validThrough) {
        this.startStates[index + 1] = { ...state };
        this.validThrough += 1;
      }
    }
    return spans;
  }

  private advance(doc: Text, index: number): void {
    const line = doc.line(index + 1);
    const state = { ...(this.startStates[index] ?? { inString: false }) };
    this.tokenizeLine(line.text, line.from, state, null);
    this.startStates[index + 1] = { ...state };
    this.validThrough = index + 1;
  }

  private tokenizeLine(lineText: string, lineFrom: number, state: { inString: boolean }, out: { from: number; to: number; className: string }[] | null): void {
    if (!lineText.length) vactParser.blankLine(state);
    const stream = new LineStream(lineText);
    while (!stream.eol()) {
      stream.start = stream.pos;
      const type = vactParser.token(stream, state);
      if (stream.pos === stream.start) stream.next();
      if (type && out) out.push({ from: lineFrom + stream.start, to: lineFrom + stream.pos, className: `vact-tok-${type}` });
    }
  }
}

export class SyntaxSpans implements SpanProvider {
  private parsed: ParsedVact | null = null;
  private lastDoc: Text | null = null;
  private dirty = true;
  private fullParse = true;
  constructor(private readonly syntax: VactSyntax) {}

  noteChanges(changes: ChangeSet, state: EditorState): void {
    const next = state.doc;
    const base = this.lastDoc;
    if (changes.empty) { this.lastDoc = next; return; }
    if (!base || !this.parsed || this.fullParse) {
      this.fullParse = true;
    } else {
      const edits: ReturnType<typeof treeEditFromDocs>[] = [];
      changes.iterChanges((fromA, toA, fromB, toB) => edits.push(treeEditFromDocs(base, next, fromA, toA, fromB, toB)));
      try {
        for (const edit of edits.reverse()) this.syntax.edit(this.parsed, edit);
      } catch {
        this.fullParse = true;
      }
    }
    this.lastDoc = next;
    this.dirty = true;
  }

  spans(state: EditorState, from: number, to: number, limit: number): SpanResult {
    const missed = state.doc !== this.lastDoc;
    if (missed) { this.lastDoc = state.doc; this.fullParse = true; this.dirty = true; }
    if (!this.parsed || this.dirty) {
      const old = this.parsed;
      let next: ParsedVact;
      try { next = this.syntax.parseDoc(state.doc, old && !this.fullParse ? old : null); }
      catch { next = this.syntax.parseDoc(state.doc, null); }
      this.parsed = next;
      if (old && old !== this.parsed) old.delete();
      this.dirty = false;
      this.fullParse = false;
    }
    const spans = styleSpans(this.parsed!, from, to).map((span) => ({ from: span.from, to: span.to, className: span.cls }));
    return bounded(spans, from, to, limit);
  }
  dispose(): void { this.parsed?.delete(); this.parsed = null; }
}
