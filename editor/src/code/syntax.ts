import { Text, type ChangeSet, type EditorState } from '@codemirror/state';
import type { CodeAnnotation } from '../app/apis';
import { CAPTURE_CLASSES, styleSpans, treeEditFromDocs, type ParsedVact, type VactSyntax } from './syntax-core';
import { SYNTAX_CAPTURE_CLASSES, type SyntaxWorkerReply, type SyntaxWorkerRequest, type SyntaxWindow } from './syntax-worker-core';
import { LineStream, vactParser } from './language';

export { CAPTURE_CLASSES, createVactSyntax, loadVactSyntax } from './syntax-core';
export type { ParsedVact, StyleSpan, SyntaxCapture, SyntaxLoader, VactSyntax } from './syntax-core';

export interface SpanResult { spans: CodeAnnotation[]; truncated: boolean }
export interface SpanProvider {
  spans(state: EditorState, from: number, to: number, limit: number): SpanResult;
  noteChanges?(changes: ChangeSet, state: EditorState): void;
  dispose?(): void;
}

export interface SyntaxWorkerPort {
  postMessage(message: SyntaxWorkerRequest, transfer?: Transferable[]): void;
  terminate(): void;
  addEventListener(type: 'message' | 'error' | 'messageerror', listener: EventListener): void;
}

type CachedLine = { from: number; spans: { from: number; to: number; className: string }[] };

function remapSpanLines(lineCache: Map<number, CachedLine>, changes: ChangeSet, base: Text, next: Text): void {
  const moved = new Map<number, CachedLine>();
  for (const [number, cached] of lineCache) {
    const newFrom = changes.mapPos(cached.from, 1), line = next.lineAt(newFrom);
    const spans: CachedLine['spans'] = [];
    for (const span of cached.spans) {
      const from = changes.mapPos(cached.from + span.from, 1), to = changes.mapPos(cached.from + span.to, -1);
      if (to > from) spans.push({ from: from - line.from, to: to - line.from, className: span.className });
    }
    lineCache.delete(number); moved.set(line.number - 1, { from: line.from, spans });
  }
  for (const [number, cached] of moved) lineCache.set(number, cached);
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
  private pending: (() => void) | null = null;
  private currentState: EditorState | null = null;
  private captureWindow: { from: number; to: number } | null = null;
  private readonly lineCache = new Map<number, { from: number; spans: { from: number; to: number; className: string }[] }>();
  private readonly touched = new Set<number>();
  private disposed = false;
  readonly stats = { syncParses: 0, deferredParses: 0, captures: 0, capturedLines: 0 };
  constructor(private readonly syntax: VactSyntax, private readonly onSyntax: () => void = () => {},
    private readonly afterPresent: (cb: () => void) => () => void = (cb) => {
      if (typeof globalThis.requestAnimationFrame === 'function') {
        let timer: ReturnType<typeof setTimeout> | null = null;
        let frame = globalThis.requestAnimationFrame(() => { timer = setTimeout(cb, 0); });
        return () => { globalThis.cancelAnimationFrame(frame); if (timer !== null) clearTimeout(timer); frame = 0; };
      }
      const timer = setTimeout(cb, 0); return () => clearTimeout(timer);
    }) {}

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

  flush(): void { if (this.pending) { this.pending(); this.pending = null; this.reparse(); } }

  private schedule(): void {
    if (this.pending || this.disposed) return;
    this.pending = this.afterPresent(() => { this.pending = null; this.reparse(); });
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
    remapSpanLines(this.lineCache, changes, base, next);
  }

  dispose(): void { if (this.pending) this.pending(); this.pending = null; this.disposed = true; this.parsed?.delete(); this.parsed = null; this.lineCache.clear(); }
}

/** Main-thread cache and bounded message producer for the dedicated syntax worker. */
export class WorkerSyntaxSpans implements SpanProvider {
  private seq = 0;
  private lastDoc: Text | null = null;
  private window: SyntaxWindow = [0, 0];
  private workerCacheReady = false;
  private everApplied = false;
  private failed = false;
  private disposed = false;
  private readyTimer: ReturnType<typeof setTimeout> | null;
  private readonly fallback = new FallbackSpans();
  private readonly lineCache = new Map<number, CachedLine>();
  private readonly inFlight = new Set<string>();
  private truncated = false;
  private resyncAfterStale = false;
  readonly stats = { posts: 0, resets: 0, replies: 0, staleReplies: 0, workerFailures: 0, captures: 0, capturedLines: 0 };

  constructor(private readonly worker: SyntaxWorkerPort, base: string, private readonly onSyntax: () => void = () => {},
    private readonly onFail: () => void = () => {}, private readonly onFirstApply: () => void = () => {}) {
    worker.addEventListener('message', this.message);
    worker.addEventListener('error', this.error);
    worker.addEventListener('messageerror', this.error);
    this.readyTimer = setTimeout(() => this.fail(), 10_000);
    this.post({ type: 'init', base });
  }

  noteChanges(changes: ChangeSet, state: EditorState): void {
    const next = state.doc, base = this.lastDoc;
    this.fallback.noteChanges(changes, state);
    if (changes.empty) { this.lastDoc = next; return; }
    let applied: Text;
    try { if (!base) throw new Error('Missing syntax base document'); applied = changes.apply(base); }
    catch { this.sendReset(next); return; }
    if (!base || applied.length !== next.length) { this.sendReset(next); return; }
    remapSpanLines(this.lineCache, changes, base, next);
    this.lastDoc = next; this.seq += 1; this.inFlight.clear();
    let count = 0, insertedUnits = 0;
    const entries: [number, number, string][] = [];
    changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
      count += 1; insertedUnits += inserted.length;
      if (count <= 1024 && insertedUnits <= 65_536) entries.push([fromA, toA, inserted.toString()]);
    });
    if (count > 1024 || insertedUnits > 65_536) this.postReset(next);
    else this.post({ type: 'edit', seq: this.seq, changes: entries, window: this.window });
  }

  spans(state: EditorState, from: number, to: number, limit: number): SpanResult {
    const first = state.doc.lineAt(Math.max(0, Math.min(state.doc.length, from))).number - 1;
    const last = state.doc.lineAt(Math.max(from, Math.min(state.doc.length, to > from ? to - 1 : to))).number - 1;
    this.window = [Math.max(0, first - 1), Math.min(state.doc.lines - 1, last + 1)];
    if (this.lastDoc !== state.doc) this.sendReset(state.doc);
    if (!this.workerCacheReady || this.failed) return this.fallback.spans(state, from, to, limit);
    const missing: number[] = [];
    for (let line = first; line <= last; line += 1) if (!this.lineCache.has(line)) missing.push(line);
    if (missing.length) {
      const low = missing[0]!, high = missing[missing.length - 1]!;
      const covered = [...this.inFlight].some((request) => {
        const [seq, first, last] = request.split(':').map(Number);
        return seq === this.seq && first <= low && last >= high;
      });
      if (!covered) { this.inFlight.add(`${this.seq}:${low}:${high}`); this.post({ type: 'lines', seq: this.seq, first: low, last: high }); }
    }
    const result: { from: number; to: number; className: string }[] = [];
    for (let lineNo = first; lineNo <= last; lineNo += 1) {
      const line = state.doc.line(lineNo + 1), cached = this.lineCache.get(lineNo);
      if (cached) for (const span of cached.spans) result.push({ from: line.from + span.from, to: line.from + span.to, className: span.className });
    }
    return { ...bounded(result, from, to, limit), truncated: this.truncated };
  }

  dispose(): void {
    if (this.disposed) return;
    if (!this.failed) { this.post({ type: 'dispose' }); this.worker.terminate(); }
    this.clearReadyTimer(); this.failed = true; this.disposed = true;
    this.lineCache.clear(); this.inFlight.clear();
  }

  private post(message: SyntaxWorkerRequest): void { if (this.failed || this.disposed) return; this.stats.posts += 1; this.worker.postMessage(message); }
  private postReset(doc: Text): void {
    this.stats.resets += 1; this.lineCache.clear(); this.workerCacheReady = false;
    this.post({ type: 'reset', seq: this.seq, text: doc.toString(), window: this.window });
  }
  private sendReset(doc: Text): void {
    this.lastDoc = doc; this.seq += 1; this.inFlight.clear(); this.postReset(doc);
  }
  private clearReadyTimer(): void { if (this.readyTimer !== null) clearTimeout(this.readyTimer); this.readyTimer = null; }
  private readonly message: EventListener = (event) => {
    const reply = (event as MessageEvent<SyntaxWorkerReply>).data;
    if (!reply || this.failed) return;
    if (reply.type === 'ready') { this.clearReadyTimer(); return; }
    if (reply.type === 'failed') { this.fail(); return; }
    this.stats.replies += 1;
    if (reply.seq !== this.seq) { this.stats.staleReplies += 1; this.resyncAfterStale = true; return; }
    for (let row = 0; row + 2 < reply.lines.length; row += 3) {
      const number = reply.lines[row]!, first = reply.lines[row + 1]!, count = reply.lines[row + 2]!;
      const line = this.lastDoc?.line(number + 1); if (!line) continue;
      const spans: CachedLine['spans'] = [];
      for (let index = first; index < first + count; index += 1) {
        const offset = index * 3, key = SYNTAX_CAPTURE_CLASSES[reply.spans[offset + 2]!];
        const className = key ? CAPTURE_CLASSES[key] : undefined;
        if (className) spans.push({ from: reply.spans[offset]!, to: reply.spans[offset + 1]!, className });
      }
      this.lineCache.set(number, { from: line.from, spans });
    }
    this.truncated = reply.truncated; this.inFlight.clear();
    if (this.resyncAfterStale) {
      this.resyncAfterStale = false;
      const lastLine = Math.max(0, (this.lastDoc?.lines ?? 1) - 1);
      const first = Math.max(0, Math.min(this.window[0], lastLine));
      const last = Math.max(first, Math.min(this.window[1], lastLine));
      this.window = [first, last];
      for (const line of this.lineCache.keys()) if (line < first || line > last) this.lineCache.delete(line);
      this.inFlight.add(`${this.seq}:${first}:${last}`);
      this.post({ type: 'lines', seq: this.seq, first, last });
    }
    const firstApply = !this.everApplied; this.workerCacheReady = true; this.everApplied = true;
    if (firstApply) this.onFirstApply();
    this.onSyntax();
  };
  private readonly error: EventListener = () => this.fail();
  private fail(): void {
    if (this.failed || this.disposed) return;
    this.failed = true; this.clearReadyTimer(); this.stats.workerFailures += 1;
    this.worker.terminate(); this.onFail();
  }
}
