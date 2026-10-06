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
  private readonly fallback = new FallbackSpans();
  private parsed: ParsedVact | null = null;
  private lastDoc: Text | null = null;
  private fullParse = true;
  private pending: ReturnType<typeof setTimeout> | null = null;
  private currentState: EditorState | null = null;
  private captureWindow: { from: number; to: number } | null = null;
  private readonly lineCache = new Map<number, { from: number; spans: { from: number; to: number; className: string }[] }>();
  private readonly touched = new Set<number>();
  private disposed = false;
  readonly stats = { syncParses: 0, deferredParses: 0, captures: 0, capturedLines: 0 };
  constructor(private readonly syntax: VactSyntax, private readonly onSyntax: () => void = () => {}) {}

  noteChanges(changes: ChangeSet, state: EditorState): void {
    const next = state.doc;
    const base = this.lastDoc;
    this.currentState = state;
    if (changes.empty) { this.lastDoc = next; return; }
    if (!base || !this.parsed || this.fullParse) {
      this.fullParse = true;
    } else {
      const edits: ReturnType<typeof treeEditFromDocs>[] = [];
      changes.iterChanges((fromA, toA, fromB, toB) => {
        edits.push(treeEditFromDocs(base, next, fromA, toA, fromB, toB));
        const first = next.lineAt(fromB).number - 1, last = next.lineAt(Math.min(next.length, toB)).number - 1;
        for (let line = first; line <= last; line += 1) this.touched.add(line);
      });
      try {
        for (const edit of edits.reverse()) this.syntax.edit(this.parsed, edit);
      } catch {
        this.fullParse = true;
      }
      this.remapLines(changes, base, next);
    }
    this.lastDoc = next;
    this.schedule();
  }

  spans(state: EditorState, from: number, to: number, limit: number): SpanResult {
    if (state.doc !== this.lastDoc) {
      this.lastDoc = state.doc; this.currentState = state; this.fullParse = true; this.lineCache.clear(); this.schedule();
    }
    this.captureWindow = { from, to };
    if (!this.parsed) {
      return this.fallback.spans(state, from, to, limit);
    }
    const first = state.doc.lineAt(Math.max(0, Math.min(state.doc.length, from))).number - 1;
    const last = state.doc.lineAt(Math.max(from, Math.min(state.doc.length, to > from ? to - 1 : to))).number - 1;
    if (!this.pending) this.captureMissing(state.doc, first, last);
    const spans: { from: number; to: number; className: string }[] = [];
    for (let number = first; number <= last; number += 1) {
      const line = state.doc.line(number + 1), cached = this.lineCache.get(number);
      if (cached) for (const span of cached.spans) spans.push({ ...span, from: line.from + span.from, to: line.from + span.to });
    }
    return bounded(spans, from, to, limit);
  }

  flush(): void { if (this.pending) { clearTimeout(this.pending); this.pending = null; this.reparse(); } }

  private schedule(): void {
    if (this.pending || this.disposed) return;
    this.pending = setTimeout(() => { this.pending = null; this.reparse(); }, 0);
  }

  private reparse(): void {
    const state = this.currentState, doc = state?.doc;
    if (!doc || this.disposed) return;
    const old = this.parsed;
    let next: ParsedVact;
    try { next = this.syntax.parseDoc(doc, old && !this.fullParse ? old : null); }
    catch { next = this.syntax.parseDoc(doc, null); }
    const changed = old && !this.fullParse && next.changedRanges ? next.changedRanges(old) : null;
    this.parsed = next;
    const lines = new Set(this.touched);
    if (changed === null) {
      if (this.captureWindow) this.addWindowLines(doc, lines, this.captureWindow.from, this.captureWindow.to);
    } else {
      const changedLineRanges = changed.map((range) => ({
        first: doc.lineAt(Math.max(0, Math.min(doc.length, range.from))).number - 1,
        last: doc.lineAt(Math.max(0, Math.min(doc.length, range.to))).number - 1,
      }));
      if (this.captureWindow) {
        const windowFirst = doc.lineAt(Math.max(0, Math.min(doc.length, this.captureWindow.from))).number - 1;
        const windowLast = doc.lineAt(Math.max(0, Math.min(doc.length, this.captureWindow.to))).number - 1;
        for (const range of changedLineRanges) {
          const first = Math.max(range.first, windowFirst), last = Math.min(range.last, windowLast);
          for (let number = first; number <= last; number += 1) lines.add(number);
        }
      }
      for (const number of this.lineCache.keys()) {
        const outsideWindow = !this.captureWindow ||
          number < doc.lineAt(Math.max(0, Math.min(doc.length, this.captureWindow.from))).number - 1 ||
          number > doc.lineAt(Math.max(0, Math.min(doc.length, this.captureWindow.to))).number - 1;
        if (outsideWindow && changedLineRanges.some(({ first, last }) => number >= first && number <= last)) {
          this.lineCache.delete(number);
        }
      }
    }
    if (!old || this.fullParse) {
      this.lineCache.clear();
      if (this.captureWindow) this.addWindowLines(doc, lines, this.captureWindow.from, this.captureWindow.to);
    }
    if (lines.size) this.captureLines(doc, lines);
    if (old && old !== next) old.delete();
    this.touched.clear(); this.fullParse = false; this.stats.deferredParses++;
    this.onSyntax();
  }

  private addWindowLines(doc: Text, lines: Set<number>, from: number, to: number): void {
    const first = doc.lineAt(Math.max(0, Math.min(doc.length, from))).number - 1;
    const last = doc.lineAt(Math.max(from, Math.min(doc.length, to))).number - 1;
    for (let number = first; number <= last; number += 1) lines.add(number);
  }

  private captureMissing(doc: Text, first: number, last: number): void {
    const lines = new Set<number>();
    for (let number = first; number <= last; number += 1) if (!this.lineCache.has(number)) lines.add(number);
    this.captureLines(doc, lines);
  }

  private captureLines(doc: Text, lines: Set<number>): void {
    if (!this.parsed || !lines.size) return;
    this.stats.captures++;
    this.stats.capturedLines += lines.size;
    for (const number of lines) {
      if (number < 0 || number >= doc.lines) continue;
      const line = doc.line(number + 1);
      this.lineCache.set(number, { from: line.from, spans: styleSpans(this.parsed, line.from, line.to).map((span) => ({ from: span.from - line.from, to: span.to - line.from, className: span.cls })) });
    }
  }

  private remapLines(changes: ChangeSet, base: Text, next: Text): void {
    const moved = new Map<number, { from: number; spans: { from: number; to: number; className: string }[] }>();
    for (const [number, cached] of this.lineCache) {
      const newFrom = changes.mapPos(cached.from, 1);
      const line = next.lineAt(newFrom), mapped: { from: number; to: number; className: string }[] = [];
      for (const span of cached.spans) {
        const from = changes.mapPos(cached.from + span.from, 1), to = changes.mapPos(cached.from + span.to, -1);
        if (to > from) mapped.push({ from: from - line.from, to: to - line.from, className: span.className });
      }
      this.lineCache.delete(number); moved.set(line.number - 1, { from: line.from, spans: mapped });
    }
    for (const [number, cached] of moved) this.lineCache.set(number, cached);
  }

  dispose(): void { if (this.pending) clearTimeout(this.pending); this.pending = null; this.disposed = true; this.parsed?.delete(); this.parsed = null; this.lineCache.clear(); }
}
