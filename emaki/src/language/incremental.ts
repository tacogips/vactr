import type { ChangeSet, Text } from '@codemirror/state';
import type { LineLanguage, LineToken, TokenSpan } from '../types';

type Options = { checkpointEvery?: number; maxLinesPerFrame?: number; maxMsPerFrame?: number; cacheLines?: number; now?: () => number };
export class IncrementalHighlighter<S> {
  private doc!: Text;
  private checkpoints = new Map<number, S>();
  private stale = new Map<number, S>();
  private cache = new Map<string, { tokens: readonly LineToken[]; state: S }>();
  private ids = new WeakMap<object, number>();
  private nextId = 0;
  private furthest = 0;
  private changedThrough = 0;
  private cursor = 0;
  private cursorState: S;
  private readonly every: number;
  private readonly maxLines: number;
  private readonly maxMs: number;
  private readonly capacity: number;
  private readonly now: () => number;
  approximate = false;
  stats = { linesTokenized: 0, checkpointsValid: 0, cacheHits: 0 };
  constructor(private readonly lang: LineLanguage<S>, opts: Options = {}) {
    this.every = opts.checkpointEvery ?? 32; this.maxLines = opts.maxLinesPerFrame ?? 2000;
    this.maxMs = opts.maxMsPerFrame ?? 2; this.capacity = opts.cacheLines ?? 4096;
    this.now = opts.now ?? (() => performance.now()); this.cursorState = lang.startState();
  }
  setDoc(doc: Text): void {
    this.doc = doc; this.checkpoints.clear(); this.stale.clear(); this.cache.clear();
    this.cursor = 0; this.cursorState = this.lang.startState(); this.checkpoints.set(0, this.cursorState);
    this.furthest = 0; this.changedThrough = 0; this.approximate = false; this.stats = { linesTokenized: 0, checkpointsValid: 1, cacheHits: 0 };
  }
  applyChanges(changes: ChangeSet, newDoc: Text): void {
    let first = Infinity, lastOld = 0, lastNew = 0;
    changes.iterChangedRanges((fromA, toA, _fromB, toB) => { first = Math.min(first, this.doc.lineAt(fromA).number - 1); lastOld = Math.max(lastOld, this.doc.lineAt(toA).number - 1); lastNew = Math.max(lastNew, newDoc.lineAt(toB).number - 1); });
    if (first === Infinity) { this.doc = newDoc; return; }
    const delta = newDoc.lines - this.doc.lines;
    const old = this.checkpoints;
    this.checkpoints = new Map(); this.stale = new Map();
    for (const [line, state] of old) {
      if (line <= first) this.checkpoints.set(line, state);
      else if (delta === 0) this.stale.set(line, state);
      else if (line > lastOld && delta % this.every === 0) this.stale.set(line + delta, state);
    }
    this.doc = newDoc; this.changedThrough = lastNew;
    const start = Math.floor(first / this.every) * this.every;
    this.cursor = start; this.cursorState = this.checkpoints.get(start) ?? this.lang.startState();
    if (!this.checkpoints.has(start)) { this.cursor = 0; this.cursorState = this.lang.startState(); }
    this.stats.checkpointsValid = this.checkpoints.size;
  }
  private key(state: S): string {
    if ((typeof state === 'object' && state !== null) || typeof state === 'function') {
      const object = state as object; let id = this.ids.get(object);
      if (id === undefined) { id = ++this.nextId; this.ids.set(object, id); }
      return `o${id}`;
    }
    return `${typeof state}:${String(state)}`;
  }
  private tokenize(text: string, state: S): { tokens: readonly LineToken[]; state: S } {
    const key = `${text}\u0000${this.key(state)}`; const cached = this.cache.get(key);
    if (cached) { this.cache.delete(key); this.cache.set(key, cached); this.stats.cacheHits++; return cached; }
    const result = this.lang.tokenizeLine(text, state); this.stats.linesTokenized++;
    if (this.capacity > 0) { this.cache.set(key, result); if (this.cache.size > this.capacity) this.cache.delete(this.cache.keys().next().value!); }
    return result;
  }
  private checkpoint(line: number, state: S): void {
    if (line % this.every !== 0) return;
    this.checkpoints.set(line, state);
    const previous = this.stale.get(line);
    if (line > this.changedThrough && previous !== undefined && this.lang.sameState(previous, state)) {
      for (const [n, s] of this.stale) if (n > line) this.checkpoints.set(n, s);
      this.stale.clear();
    } else this.stale.delete(line);
    this.stats.checkpointsValid = this.checkpoints.size;
  }
  private advance(target: number): boolean {
    const started = this.now(); let count = 0;
    while (this.cursor < target && count < this.maxLines && this.now() - started < this.maxMs) {
      this.cursorState = this.tokenize(this.doc.line(this.cursor + 1).text, this.cursorState).state;
      this.cursor++; count++; this.checkpoint(this.cursor, this.cursorState);
      const known = this.checkpoints.get(Math.floor(target / this.every) * this.every);
      if (known !== undefined && Math.floor(target / this.every) * this.every > this.cursor) {
        this.cursor = Math.floor(target / this.every) * this.every; this.cursorState = known;
      }
    }
    return this.cursor >= target;
  }
  tokens(fromLine: number, toLine: number): TokenSpan[] {
    const from = Math.max(0, Math.min(fromLine, this.doc.lines)); const to = Math.max(from, Math.min(toLine, this.doc.lines));
    this.furthest = Math.max(this.furthest, from);
    let nearest = 0; for (const line of this.checkpoints.keys()) if (line <= from && line > nearest) nearest = line;
    if (this.cursor > from || nearest > this.cursor) { this.cursor = nearest; this.cursorState = this.checkpoints.get(nearest)!; }
    this.approximate = !this.advance(from);
    let state = this.cursorState; const result: TokenSpan[] = [];
    for (let n = from; n < to; n++) {
      const line = this.doc.line(n + 1); const tokenized = this.tokenize(line.text, state);
      for (const token of tokenized.tokens) result.push({ from: line.from + token.from, to: line.from + token.to, tag: token.tag });
      state = tokenized.state;
      if (!this.approximate) this.checkpoint(n + 1, state);
    }
    if (!this.approximate) { this.cursor = to; this.cursorState = state; }
    return result;
  }
  catchUp(): boolean { this.advance(this.furthest); return this.cursor < this.furthest; }
}
