import { ChangeSet, Text } from '@codemirror/state';
import { CAPTURE_CLASSES, styleSpans, treeEditFromDocs, type ParsedVact, type VactSyntax } from './syntax-core';

export type SyntaxWindow = [first: number, last: number];
export type SyntaxWorkerRequest =
  | { type: 'init'; base: string }
  | { type: 'reset'; seq: number; text: string; window: SyntaxWindow }
  | { type: 'edit'; seq: number; changes: [fromA: number, toA: number, insert: string][]; window: SyntaxWindow }
  | { type: 'lines'; seq: number; first: number; last: number }
  | { type: 'dispose' };
export type SyntaxWorkerReply =
  | { type: 'ready' }
  | { type: 'failed'; reason: string }
  | { type: 'spans'; seq: number; lines: Uint32Array; spans: Uint32Array; truncated: boolean };
export type SyntaxCaptureClass = keyof typeof CAPTURE_CLASSES;
export const SYNTAX_CAPTURE_CLASSES = Object.keys(CAPTURE_CLASSES) as SyntaxCaptureClass[];
export interface SyntaxWorkerCoreOptions {
  post(reply: SyntaxWorkerReply, transfer?: Transferable[]): void;
  load(base: string): Promise<VactSyntax>;
  defer?(cb: () => void): void;
}

const MAX_SPANS = 16_384;

/** Stateful worker engine. Importing this module has no worker-global side effects. */
export class SyntaxWorkerCore {
  private syntax: VactSyntax | null = null;
  private parsed: ParsedVact | null = null;
  private doc = Text.of(['']);
  private seq = 0;
  private window: SyntaxWindow = [0, 0];
  private fullParse = true;
  private pending = false;
  private disposed = false;
  private readonly touched = new Set<number>();
  private readonly requested = new Set<number>();
  private readonly defer: (cb: () => void) => void;

  constructor(private readonly options: SyntaxWorkerCoreOptions) {
    this.defer = options.defer ?? ((cb) => { setTimeout(cb, 0); });
  }

  handle(request: SyntaxWorkerRequest): void {
    if (this.disposed && request.type !== 'init') return;
    try {
      switch (request.type) {
        case 'init': void this.initialize(request.base); break;
        case 'reset': this.reset(request); break;
        case 'edit': this.edit(request); break;
        case 'lines': this.lines(request); break;
        case 'dispose': this.dispose(); break;
      }
    } catch (error) {
      this.fail(error);
    }
  }

  private async initialize(base: string): Promise<void> {
    try {
      this.syntax = await this.options.load(base);
      if (this.disposed) return;
      this.options.post({ type: 'ready' });
      if (this.pending || !this.parsed) this.schedule();
    } catch (error) { this.fail(error); }
  }

  private reset(request: Extract<SyntaxWorkerRequest, { type: 'reset' }>): void {
    this.seq = request.seq;
    this.doc = Text.of(request.text.split('\n'));
    this.window = request.window;
    this.parsed?.delete(); this.parsed = null;
    this.fullParse = true; this.touched.clear(); this.requested.clear();
    this.schedule();
  }

  private edit(request: Extract<SyntaxWorkerRequest, { type: 'edit' }>): void {
    const base = this.doc;
    const changes = ChangeSet.of(request.changes.map(([from, to, insert]) => ({ from, to, insert })), base.length);
    const next = changes.apply(base);
    const edits: ReturnType<typeof treeEditFromDocs>[] = [];
    const nextTouched = new Set<number>();
    changes.iterChanges((fromA, toA, fromB, toB) => {
      edits.push(treeEditFromDocs(base, next, fromA, toA, fromB, toB));
      const first = next.lineAt(fromB).number - 1;
      const last = next.lineAt(Math.min(next.length, toB)).number - 1;
      for (let line = first; line <= last; line += 1) nextTouched.add(line);
    });
    if (this.parsed && !this.fullParse) {
      try { for (const treeEdit of edits.reverse()) this.syntax?.edit(this.parsed, treeEdit); }
      catch { this.fullParse = true; }
    } else this.fullParse = true;

    if (this.pending) {
      this.remapSet(this.touched, changes, base, next);
      this.remapSet(this.requested, changes, base, next);
    }
    for (const line of nextTouched) this.touched.add(line);
    this.doc = next; this.seq = request.seq; this.window = request.window;
    this.schedule();
  }

  private remapSet(lines: Set<number>, changes: ReturnType<typeof ChangeSet.of>, base: Text, next: Text): void {
    const mapped = new Set<number>();
    for (const number of lines) {
      if (number < 0 || number >= base.lines) continue;
      const pos = changes.mapPos(base.line(number + 1).from, 1);
      if (pos <= next.length) mapped.add(next.lineAt(pos).number - 1);
    }
    lines.clear(); for (const number of mapped) lines.add(number);
  }

  private lines(request: Extract<SyntaxWorkerRequest, { type: 'lines' }>): void {
    if (request.seq !== this.seq || !this.syntax) return;
    if (this.pending) {
      for (let line = request.first; line <= request.last; line += 1) this.requested.add(line);
      return;
    }
    this.postSpans(this.capture(new Set(Array.from({ length: Math.max(0, request.last - request.first + 1) }, (_, i) => request.first + i))));
  }

  private schedule(): void {
    if (this.pending || this.disposed || !this.syntax) return;
    this.pending = true;
    this.defer(() => { if (!this.disposed) this.parsePending(); });
  }

  private parsePending(): void {
    if (!this.pending || !this.syntax || this.disposed) return;
    this.pending = false;
    const old = this.parsed;
    let next: ParsedVact;
    try { next = this.syntax.parseDoc(this.doc, old && !this.fullParse ? old : null); }
    catch {
      try { next = this.syntax.parseDoc(this.doc, null); this.fullParse = true; }
      catch (error) { this.fail(error); return; }
    }
    let changed: { from: number; to: number }[] | null = null;
    if (old && !this.fullParse && next.changedRanges) {
      try { changed = next.changedRanges(old); } catch { changed = null; }
    }
    this.parsed = next;
    const lines = new Set([...this.touched, ...this.requested]);
    if (changed === null || !old || this.fullParse) this.addWindow(lines);
    else {
      const firstWindow = Math.max(0, Math.min(this.doc.lines - 1, this.window[0]));
      const lastWindow = Math.max(firstWindow, Math.min(this.doc.lines - 1, this.window[1]));
      for (const range of changed) {
        const first = Math.max(firstWindow, this.doc.lineAt(Math.max(0, Math.min(this.doc.length, range.from))).number - 1);
        const last = Math.min(lastWindow, this.doc.lineAt(Math.max(0, Math.min(this.doc.length, range.to))).number - 1);
        for (let line = first; line <= last; line += 1) lines.add(line);
      }
    }
    if (old && old !== next) old.delete();
    this.fullParse = false; this.touched.clear(); this.requested.clear();
    this.postSpans(this.capture(lines));
    if (this.pending) this.schedule();
  }

  private addWindow(lines: Set<number>): void {
    const first = Math.max(0, Math.min(this.doc.lines - 1, this.window[0]));
    const last = Math.max(first, Math.min(this.doc.lines - 1, this.window[1]));
    for (let line = first; line <= last; line += 1) lines.add(line);
  }

  private capture(lines: Set<number>): Extract<SyntaxWorkerReply, { type: 'spans' }> {
    const lineRows: number[] = [], spanRows: number[] = [];
    let truncated = false;
    for (const number of [...lines].sort((a, b) => a - b)) {
      if (number < 0 || number >= this.doc.lines || !this.parsed) continue;
      const line = this.doc.line(number + 1), firstSpan = spanRows.length / 3;
      for (const span of styleSpans(this.parsed, line.from, line.to)) {
        if (spanRows.length / 3 >= MAX_SPANS) { truncated = true; break; }
        const classIndex = SYNTAX_CAPTURE_CLASSES.findIndex((key) => CAPTURE_CLASSES[key] === span.cls);
        const from = Math.max(line.from, span.from), to = Math.min(line.to, span.to);
        if (classIndex >= 0 && from < to) spanRows.push(from - line.from, to - line.from, classIndex);
      }
      lineRows.push(number, firstSpan, spanRows.length / 3 - firstSpan);
      if (truncated) break;
    }
    return { type: 'spans', seq: this.seq, lines: new Uint32Array(lineRows), spans: new Uint32Array(spanRows), truncated };
  }

  private postSpans(reply: Extract<SyntaxWorkerReply, { type: 'spans' }>): void {
    this.options.post(reply, [reply.lines.buffer, reply.spans.buffer]);
  }

  private fail(error: unknown): void {
    if (this.disposed) return;
    this.options.post({ type: 'failed', reason: error instanceof Error ? error.message : String(error) });
  }

  private dispose(): void {
    this.disposed = true; this.parsed?.delete(); this.parsed = null;
    this.touched.clear(); this.requested.clear();
  }
}
