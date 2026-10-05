import { Text, type ChangeSet, type EditorState } from '@codemirror/state';
import type { CodeAnnotation } from '../app/apis';
import { styleSpans, type ParsedVact, type VactSyntax } from './syntax-core';
import { tokenizerSpans } from './language';

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
  noteChanges(changes: ChangeSet, state: EditorState): void { this.tokenizer.noteChanges(changes, state); }
  spans(state: EditorState, from: number, to: number, limit: number): SpanResult {
    const spans = this.tokenizer.spans(state, from, to).map((span) => ({ from: span.from, to: span.to, className: span.className }));
    return bounded(spans, from, to, limit);
  }
}

class FallbackTokenizerCache {
  private cachedText = '';
  private cached: { from: number; to: number; className: string }[] = [];
  private dirty = true;
  noteChanges(changes: ChangeSet, state: EditorState): void {
    if (changes.empty) return;
    this.dirty = true;
    this.cachedText = state.doc.toString();
  }
  spans(state: EditorState, from: number, to: number): { from: number; to: number; className: string }[] {
    if (this.dirty || this.cachedText !== state.doc.toString()) {
      this.cached = tokenizerSpans(state.doc).map((span) => ({ from: span.from, to: span.to, className: span.className }));
      this.cachedText = state.doc.toString();
      this.dirty = false;
    }
    return this.cached.filter((span) => span.to > from && span.from < to);
  }
}

export class SyntaxSpans implements SpanProvider {
  private parsed: ParsedVact | null = null;
  private text: string;
  private nextText: string;
  private pending: { from: number; to: number; insert: string }[] = [];
  private fullReparse = false;
  constructor(private readonly syntax: VactSyntax) {
    this.text = '';
    this.nextText = '';
  }
  noteChanges(_changes: ChangeSet, state: EditorState): void {
    const next = state.doc.toString();
    if (next === this.nextText) return;
    if (this.pending.length || this.fullReparse) this.fullReparse = true;
    else _changes.iterChanges((fromA, toA, fromB, toB) => this.pending.push({ from: fromA, to: toA, insert: state.doc.sliceString(fromB, toB) }));
    this.nextText = next;
  }
  spans(state: EditorState, from: number, to: number, limit: number): SpanResult {
    if (!this.parsed || this.nextText !== state.doc.toString()) { this.fullReparse = true; this.nextText = state.doc.toString(); }
    if (this.fullReparse || this.pending.length) {
      const old = this.parsed;
      try { this.parsed = old && !this.fullReparse && this.syntax.reparse ? this.syntax.reparse(old, this.nextText, this.pending) : this.syntax.parse(this.nextText); }
      catch { this.parsed = this.syntax.parse(this.nextText); }
      if (old && old !== this.parsed) old.delete();
      this.text = this.nextText; this.pending = []; this.fullReparse = false;
    }
    const spans = styleSpans(this.parsed!, from, to).map((span) => ({ from: span.from, to: span.to, className: span.cls }));
    return bounded(spans, from, to, limit);
  }
  dispose(): void { this.parsed?.delete(); this.parsed = null; }
}
