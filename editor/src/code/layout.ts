import { Text } from '@codemirror/state';
import type { CodeRect, CodeRange } from '../app/apis';
import { RESOURCE_LIMITS } from './resources';

export interface TextMetricsSource { font: string; measureText(text: string): { width: number } }
export interface LayoutFont { font: string; fallback?: string; generation?: number; lineHeight: number; baseline: number }
export interface LayoutViewport { width: number; height: number; scrollLeft: number; scrollTop: number; left?: number; top?: number; gutter?: number }
export interface ShapedRun { text: string; from: number; to: number; x: number; width: number }
export interface ShapedLine { number: number; from: number; to: number; runs: readonly ShapedRun[]; width: number; rtlUnsupported: boolean }
interface LineIndex { from: number; to: number; next: number }
const rtl = /[\u0590-\u08ff\ufb1d-\ufdff\ufe70-\ufeff]/u;
const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
const MAX_CLUSTERS_PER_LINE = 4096;

/** No per-character geometry arrays: huge lines are measured lazily, drawn as cropped run tiles. */
export class TextLayout {
  private source = '';
  private text: Text | null = null;
  private documentCache: { doc: Text; value: string } | null = null;
  private lines: LineIndex[] = [{ from: 0, to: 0, next: 0 }];
  private cache = new Map<number, { line: ShapedLine; bytes: number; clusters: number[] | null }>();
  private bytes = 0;
  private readonly cacheLimit: number;
  readonly stats = { builds: 0, evictions: 0, segmentations: 0 };
  constructor(private metrics: TextMetricsSource, public font: LayoutFont, cacheLimit = RESOURCE_LIMITS.layout) {
    this.cacheLimit = Math.min(RESOURCE_LIMITS.layout, Math.max(0, cacheLimit));
    this.validateFont(font);
  }
  get document(): string {
    if (!this.text) return this.source;
    if (!this.documentCache || this.documentCache.doc !== this.text) this.documentCache = { doc: this.text, value: this.text.toString() };
    return this.documentCache.value;
  }
  get lineCount(): number { return this.text?.lines ?? this.lines.length; }
  get cacheBytes(): number { return this.bytes; }
  setDocument(text: string): void {
    if (!this.text && text === this.source) return;
    const previous = this.source; const previousLines = this.lines;
    this.text = null; this.documentCache = null;
    this.source = text; this.lines = [];
    let from = 0;
    for (const match of text.matchAll(/\r\n|\r|\n/g)) {
      const to = match.index!; const next = to + match[0].length;
      this.lines.push({ from, to, next }); from = next;
    }
    this.lines.push({ from, to: text.length, next: text.length });
    // Keep unchanged shaped runs by identity; edits invalidate only changed lines.
    for (const [n, entry] of this.cache) {
      const before = previousLines[n], after = this.lines[n];
      if (!before || !after || before.from !== after.from || before.to !== after.to || previous.slice(before.from, before.to) !== text.slice(after.from, after.to)) {
        this.bytes -= entry.bytes; this.cache.delete(n);
      }
    }
  }
  setText(doc: Text): void {
    if (doc === this.text) return;
    const previous = this.text, previousLines = this.lines, previousSource = this.source;
    this.text = doc; this.source = ''; this.documentCache = null;
    for (const [n, entry] of this.cache) {
      const before = previous ? (n < previous.lines ? previous.line(n + 1) : null) : previousLines[n] ?? null;
      const after = n < doc.lines ? doc.line(n + 1) : null;
      const beforeFrom = before?.from ?? -1, beforeTo = before?.to ?? -1;
      const beforeText = before ? (previous ? previous.line(n + 1).text : previousSource.slice(beforeFrom, beforeTo)) : '';
      if (!after || beforeFrom !== after.from || beforeTo !== after.to || beforeText !== after.text) {
        this.bytes -= entry.bytes; this.cache.delete(n);
      }
    }
  }
  private lineIndex(number: number): LineIndex | null {
    if (!this.text) return this.lines[number] ?? null;
    if (number < 0 || number >= this.text.lines) return null;
    const line = this.text.line(number + 1);
    return { from: line.from, to: line.to, next: line.to + (number + 1 < this.text.lines ? 1 : 0) };
  }
  private slice(from: number, to: number): string { return this.text ? this.text.sliceString(from, to) : this.source.slice(from, to); }
  private get length(): number { return this.text?.length ?? this.source.length; }
  setFont(font: LayoutFont): void {
    this.validateFont(font);
    if (JSON.stringify(font) === JSON.stringify(this.font)) return;
    this.font = font; this.invalidate();
  }
  invalidate(): void { this.cache.clear(); this.bytes = 0; }
  private validateFont(font: LayoutFont): void {
    if (!font.font || !Number.isFinite(font.lineHeight) || font.lineHeight <= 0 || !Number.isFinite(font.baseline) || font.baseline < 0 || font.baseline > font.lineHeight) throw new RangeError('Invalid layout font');
  }
  shape(number: number): ShapedLine {
    const hit = this.cache.get(number);
    if (hit) { this.cache.delete(number); this.cache.set(number, hit); return hit.line; }
    const index = this.lineIndex(number);
    if (!index) throw new RangeError('Line outside document');
    this.metrics.font = this.font.font;
    const runs: ShapedRun[] = []; const stop = Math.max(1, this.metrics.measureText(' ').width * 4);
    let x = 0; let from = index.from;
    while (from < index.to) {
      const lineText = this.slice(index.from, index.to);
      const tabOffset = lineText.indexOf('\t', from - index.from);
      const tab = tabOffset < 0 ? -1 : index.from + tabOffset;
      const to = tab < 0 ? index.to : Math.min(tab, index.to);
      if (to > from) {
        const text = this.slice(from, to); const width = this.metrics.measureText(text).width;
        runs.push({ text, from, to, x, width }); x += width;
      }
      if (to === index.to) break;
      x = (Math.floor(x / stop) + 1) * stop; from = to + 1;
    }
    const lineText = this.slice(index.from, index.to);
    const line: ShapedLine = { number, from: index.from, to: index.to, runs, width: x, rtlUnsupported: rtl.test(lineText) };
    let clusters: number[] | null = [];
    if (/[^\x00-\x7f]/u.test(lineText)) {
      clusters = [];
      this.stats.segmentations++;
      for (const part of segmenter.segment(lineText)) {
        const end = part.index + part.segment.length;
        if (end - part.index > 1) {
          clusters.push(part.index, end);
          if (clusters.length / 2 > MAX_CLUSTERS_PER_LINE) { clusters = null; break; }
        }
      }
    }
    const clusterBytes = clusters === null ? 0 : clusters.length * 4;
    const bytes = 128 + runs.reduce((n, r) => n + 64 + r.text.length * 2, 0) + clusterBytes;
    while (this.bytes + bytes > this.cacheLimit && this.cache.size) {
      const first = this.cache.keys().next().value!; this.bytes -= this.cache.get(first)!.bytes; this.cache.delete(first); this.stats.evictions++;
    }
    if (bytes <= this.cacheLimit) { this.cache.set(number, { line, bytes, clusters }); this.bytes += bytes; }
    this.stats.builds++; return line;
  }
  visible(view: LayoutViewport): ShapedLine[] {
    const first = Math.max(0, Math.floor(view.scrollTop / this.font.lineHeight));
    const last = Math.min(this.lineCount, Math.ceil((view.scrollTop + view.height) / this.font.lineHeight));
    const out: ShapedLine[] = [];
    for (let i = first; i < last; i++) out.push(this.shape(i));
    return out;
  }
  private lineAt(pos: number): number {
    let lo = 0, hi = this.lineCount - 1;
    while (lo < hi) { const mid = Math.ceil((lo + hi) / 2); if (this.lineIndex(mid)!.from <= pos) lo = mid; else hi = mid - 1; }
    return lo;
  }
  /** Snap within a whole browser-shaped run, preserving surrogate/combining clusters. */
  boundary(pos: number, bias: -1 | 1 = -1): number {
    pos = Math.max(0, Math.min(this.length, pos));
    const lineNumber = this.lineAt(pos), index = this.lineIndex(lineNumber)!;
    if (pos > index.to) return bias < 0 ? index.to : index.next;
    const relative = pos - index.from;
    const cached = this.cache.get(lineNumber)?.clusters;
    if (cached !== undefined && cached !== null) {
      let lo = 0, hi = cached.length / 2;
      while (lo < hi) {
        const mid = Math.floor((lo + hi) / 2), start = cached[mid * 2]!, end = cached[mid * 2 + 1]!;
        if (relative <= start) hi = mid;
        else if (relative >= end) lo = mid + 1;
        else return index.from + (bias < 0 ? start : end);
      }
      return pos;
    }
    const text = this.slice(index.from, index.to);
    if (!/[^\x00-\x7f]/u.test(text)) return pos;
    this.stats.segmentations++;
    for (const part of segmenter.segment(text)) {
      const end = part.index + part.segment.length;
      if (part.index === relative || end === relative) return pos;
      if (part.index < relative && relative < end) return index.from + (bias < 0 ? part.index : end);
    }
    return pos;
  }
  advance(line: ShapedLine, pos: number): number {
    pos = this.boundary(Math.max(line.from, Math.min(line.to, pos)));
    this.metrics.font = this.font.font;
    for (const run of line.runs) {
      if (pos < run.from) return run.x;
      if (pos <= run.to) return run.x + this.metrics.measureText(run.text.slice(0, pos - run.from)).width;
    }
    return line.width;
  }
  offsetInRun(run: ShapedRun, x: number, bias: -1 | 1): number {
    if (x <= 0) return run.from; if (x >= run.width) return run.to;
    this.metrics.font = this.font.font;
    let lo = 0, hi = run.text.length;
    while (lo < hi) {
      const mid = Math.floor((lo + hi) / 2);
      if (this.metrics.measureText(run.text.slice(0, mid)).width < x) lo = mid + 1; else hi = mid;
    }
    return this.boundary(run.from + (bias < 0 ? Math.max(0, lo - 1) : lo), bias);
  }
  coordsAtPos(pos: number, view: LayoutViewport): CodeRect | null {
    if (!Number.isInteger(pos) || pos < 0 || pos > this.length) return null;
    const number = this.lineAt(pos); const line = this.shape(number);
    const left = (view.left ?? 0) + (view.gutter ?? 48) + this.advance(line, pos) - view.scrollLeft;
    const top = (view.top ?? 0) + number * this.font.lineHeight - view.scrollTop;
    return { left, right: left + 1, top, bottom: top + this.font.lineHeight };
  }
  posAtCoords(coords: { x: number; y: number }, view: LayoutViewport): number | null {
    if (!Number.isFinite(coords.x) || !Number.isFinite(coords.y)) return null;
    const n = Math.max(0, Math.min(this.lineCount - 1, Math.floor((coords.y - (view.top ?? 0) + view.scrollTop) / this.font.lineHeight)));
    const line = this.shape(n); if (line.rtlUnsupported) return null;
    const x = coords.x - (view.left ?? 0) - (view.gutter ?? 48) + view.scrollLeft;
    if (x <= 0) return line.from;
    if (x >= line.width) return line.to;
    // Binary search measured prefixes, then choose the nearest grapheme boundary.
    let lo = line.from, hi = line.to;
    while (lo < hi) { const mid = Math.floor((lo + hi) / 2); if (this.advance(line, mid) < x) lo = mid + 1; else hi = mid; }
    const after = this.boundary(lo, 1); const before = this.boundary(Math.max(line.from, after - 1), -1);
    return Math.abs(this.advance(line, before) - x) <= Math.abs(this.advance(line, after) - x) ? before : after;
  }
  rangeRects(range: CodeRange, view: LayoutViewport): CodeRect[] {
    const from = this.boundary(range.from); const to = this.boundary(range.to, 1);
    if (to < from) return [];
    const first = Math.max(this.lineAt(from), Math.floor(view.scrollTop / this.font.lineHeight), 0);
    const last = Math.min(this.lineAt(to), Math.ceil((view.scrollTop + view.height) / this.font.lineHeight), this.lineCount - 1);
    const out: CodeRect[] = [];
    for (let n = first; n <= last; n++) {
      const line = this.shape(n); const a = this.coordsAtPos(Math.max(from, line.from), view)!;
      const b = this.coordsAtPos(Math.min(to, line.to), view)!;
      const left = Math.max((view.left ?? 0) + (view.gutter ?? 48), a.left);
      const right = Math.min((view.left ?? 0) + view.width, b.left + (to > line.to ? 8 : 0));
      if (right >= left) out.push({ left, right: Math.max(left + 1, right), top: a.top, bottom: a.bottom });
    }
    return out;
  }
}
