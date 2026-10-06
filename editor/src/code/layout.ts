import { Text, type ChangeSet } from '@codemirror/state';
import type { CodeRect, CodeRange } from '../app/apis';
import { RESOURCE_LIMITS } from './resources';
import type { PhaseTimer } from './frame';
import { AdvanceTable } from './advances';

export interface TextMetricsSource { font: string; measureText(text: string): { width: number } }
export interface LayoutFont { font: string; fallback?: string; generation?: number; lineHeight: number; baseline: number }
export interface LayoutViewport { width: number; height: number; scrollLeft: number; scrollTop: number; left?: number; top?: number; gutter?: number }
export interface ShapedRun { text: string; from: number; to: number; x: number; width: number }
export interface ShapedLine { number: number; from: number; to: number; runs: readonly ShapedRun[]; width: number; rtlUnsupported: boolean }
interface LineIndex { from: number; to: number; next: number }
interface RunWidthChunk {
  start: number; end: number; width: number; cumulative: number;
  prefixWidths: Map<number, number>; boundaries?: number[];
}
interface MeasuredChunk { start: number; end: number; width: number }
interface RunWidthIndex { chunks: RunWidthChunk[]; clusterRanges: Array<[number, number]>; additive: boolean }
const rtl = /[\u0590-\u08ff\ufb1d-\ufdff\ufe70-\ufeff]/u;
const complexScript = /[\u0590-\u08ff\u0900-\u0dff\u0e00-\u0eff\u1000-\u109f\u1780-\u17ff\ufb1d-\ufeff]/u;
const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
const MAX_CLUSTERS_PER_LINE = 4096;
const MAX_RUN_CHARS = 256;
function splitsSurrogatePair(text: string, index: number): boolean {
  const before = text.charCodeAt(index - 1), after = text.charCodeAt(index);
  return before >= 0xd800 && before <= 0xdbff && after >= 0xdc00 && after <= 0xdfff;
}

/** No per-character geometry arrays: huge lines are measured lazily, drawn as cropped run tiles. */
export class TextLayout {
  private source = '';
  private text: Text | null = null;
  private documentCache: { doc: Text; value: string } | null = null;
  private lines: LineIndex[] = [{ from: 0, to: 0, next: 0 }];
  private cache = new Map<number, { line: ShapedLine; bytes: number; clusters: number[] | null }>();
  private runWidths = new WeakMap<ShapedRun, RunWidthIndex>();
  private runClusters = new WeakMap<ShapedRun, { lineFrom: number; clusters: number[] | null }>();
  private bytes = 0;
  private widest = 0;
  private metricsFont = '';
  private advances: AdvanceTable | null = null;
  private stringDocumentMode = false;
  private measuredSpace: number | null = null;
  phases: PhaseTimer | null = null;
  private readonly cacheLimit: number;
  readonly stats = { builds: 0, evictions: 0, segmentations: 0, measuredTextCalls: 0, measuredTextChars: 0, maxMeasuredTextLength: 0 };
  constructor(private metrics: TextMetricsSource, public font: LayoutFont, cacheLimit = RESOURCE_LIMITS.layout) {
    this.cacheLimit = Math.min(RESOURCE_LIMITS.layout, Math.max(0, cacheLimit));
    this.validateFont(font);
    this.setMetricsFont(font.font);
  }
  get document(): string {
    if (!this.text) return this.source;
    if (!this.documentCache || this.documentCache.doc !== this.text) this.documentCache = { doc: this.text, value: this.text.toString() };
    return this.documentCache.value;
  }
  get lineCount(): number { return this.text?.lines ?? this.lines.length; }
  get cacheBytes(): number { return this.bytes; }
  get widestShaped(): number { return this.widest; }
  isRunLine(run: ShapedRun): boolean { return complexScript.test(run.text); }
  textCells(run: ShapedRun, visibleFrom = 0, visibleTo = run.width): Array<{ text: string; from: number; to: number; x: number; width: number }> {
    const index = this.runWidthIndex(run);
    const pieces: Array<{ text: string; start: number; end: number }> = [];
    let ascii = true; for (const char of run.text) if (char.codePointAt(0)! > 0x7f) { ascii = false; break; }
    if (ascii) {
      let start = Math.max(0, this.offsetInRun(run, visibleFrom, -1));
      while (start < run.text.length) {
        const left = this.widthAt(run, index, start);
        if (left > visibleTo) break;
        pieces.push({ text: run.text[start]!, start, end: start + 1 }); start++;
      }
    } else {
      for (const part of segmenter.segment(run.text)) {
        const end = part.index + part.segment.length;
        const left = this.widthAt(run, index, part.index), right = this.widthAt(run, index, end);
        if (right >= visibleFrom && left <= visibleTo) pieces.push({ text: part.segment, start: part.index, end });
        if (left > visibleTo) break;
      }
    }
    return pieces.map(piece => {
      const left = this.widthAt(run, index, piece.start), right = this.widthAt(run, index, piece.end);
      return { text: piece.text, from: piece.start, to: piece.end, x: left, width: Math.max(1, right - left) };
    });
  }
  resetWidestShaped(): void { this.widest = 0; }
  setDocument(text: string): void {
    if (!this.text && text === this.source) return;
    this.stringDocumentMode = true;
    const previous = this.source; const previousLines = this.lines; this.widest = 0;
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
  setText(doc: Text, changes?: ChangeSet): void {
    if (doc === this.text) return;
    const previous = this.text;
    if (!previous || !changes || changes.length !== previous.length || changes.newLength !== doc.length) {
      this.stringDocumentMode = false;
      this.text = doc; this.source = ''; this.documentCache = null;
      this.invalidate();
      return;
    }
    this.stringDocumentMode = false;
    const changed: Array<{ from: number; to: number }> = [];
    changes.iterChangedRanges((fromA, toA) => {
      changed.push({ from: previous.lineAt(fromA).number - 1, to: previous.lineAt(Math.min(previous.length, toA)).number - 1 });
    });
    this.text = doc; this.source = ''; this.documentCache = null;
    const moved = new Map<number, { line: ShapedLine; bytes: number; clusters: number[] | null }>();
    for (const [number, entry] of this.cache) {
      if (changed.some((range) => number >= range.from && number <= range.to)) {
        this.bytes -= entry.bytes; this.cache.delete(number);
        continue;
      }
      const from = changes.mapPos(entry.line.from, 1);
      const to = changes.mapPos(entry.line.to, -1);
      if (from > to || from > doc.length) { this.bytes -= entry.bytes; this.cache.delete(number); continue; }
      const lineNumber = doc.lineAt(from).number - 1;
      const after = doc.line(lineNumber + 1);
      if (after.to - after.from !== entry.line.to - entry.line.from) { this.bytes -= entry.bytes; this.cache.delete(number); continue; }
      const delta = after.from - entry.line.from;
      if (delta !== 0 || lineNumber !== number) {
        const runs = entry.line.runs;
        for (const run of runs) { run.from += delta; run.to += delta; }
        entry.line = { ...entry.line, number: lineNumber, from: after.from, to: after.to, runs };
      }
      this.cache.delete(number);
      moved.set(lineNumber, entry);
    }
    for (const [number, entry] of moved) this.cache.set(number, entry);
  }
  private lineIndex(number: number): LineIndex | null {
    if (!this.text) return this.lines[number] ?? null;
    if (number < 0 || number >= this.text.lines) return null;
    const line = this.text.line(number + 1);
    return { from: line.from, to: line.to, next: line.to + (number + 1 < this.text.lines ? 1 : 0) };
  }
  private slice(from: number, to: number): string { return this.text ? this.text.sliceString(from, to) : this.source.slice(from, to); }
  private get length(): number { return this.text?.length ?? this.source.length; }
  private setMetricsFont(font: string): void {
    if (font === this.metricsFont) return;
    this.metrics.font = font;
    this.metricsFont = font;
    this.advances = null;
    this.measuredSpace = null;
  }
  private measureText(text: string, clusters: number[] | null = null, lineOffset = 0, measured?: MeasuredChunk[]): number {
    let width = 0;
    this.eachMeasurePart(text, (part, start, end) => {
      const partWidth = this.measureDirect(part); width += partWidth;
      measured?.push({ start, end, width: partWidth });
    }, clusters, lineOffset);
    return width;
  }
  private eachMeasurePart(text: string, visit: (part: string, from: number, to: number) => void,
    clusters: number[] | null, lineOffset: number): void {
    if (text.length <= MAX_RUN_CHARS) { visit(text, 0, text.length); return; }
    let start = 0, clusterIndex = 0;
    while (start < text.length) {
      let end = Math.min(text.length, start + MAX_RUN_CHARS);
      if (clusters) {
        while (clusterIndex < clusters.length && clusters[clusterIndex + 1]! <= lineOffset + start) clusterIndex += 2;
        if (clusterIndex < clusters.length) {
          const clusterStart = clusters[clusterIndex]!, clusterEnd = clusters[clusterIndex + 1]!;
          if (clusterStart < lineOffset + end && clusterEnd > lineOffset + end) {
            end = clusterStart > lineOffset + start ? clusterStart - lineOffset : clusterEnd - lineOffset;
          }
        }
      }
      if (end - start > MAX_RUN_CHARS || (end > 0 && end < text.length && splitsSurrogatePair(text, end))) {
        end = Math.min(text.length, start + MAX_RUN_CHARS);
        if (end < text.length && splitsSurrogatePair(text, end)) end--;
      }
      if (end <= start) end = Math.min(text.length, start + MAX_RUN_CHARS);
      visit(text.slice(start, end), start, end);
      start = end;
    }
  }
  private runWidthIndex(run: ShapedRun, measured?: readonly MeasuredChunk[]): RunWidthIndex {
    const cached = this.runWidths.get(run);
    if (cached) return cached;
    const metadata = this.runClusters.get(run);
    const clusterRanges: Array<[number, number]> = [];
    if (metadata?.clusters) {
      for (let i = 0; i < metadata.clusters.length; i += 2) {
        const start = metadata.clusters[i]! + metadata.lineFrom - run.from;
        const end = metadata.clusters[i + 1]! + metadata.lineFrom - run.from;
        if (end > 0 && start < run.text.length) clusterRanges.push([Math.max(0, start), Math.min(run.text.length, end)]);
      }
    } else if (metadata?.clusters === null) {
      for (const part of segmenter.segment(run.text)) {
        if (part.segment.length > 1) clusterRanges.push([part.index, part.index + part.segment.length]);
      }
    }
    const index: RunWidthIndex = { chunks: [], clusterRanges, additive: false };
    let cumulative = 0;
    if (measured) {
      for (const chunk of measured) {
        const prefixWidths = new Map<number, number>([[0, 0], [chunk.end - chunk.start, chunk.width]]);
        cumulative += chunk.width;
        index.chunks.push({ ...chunk, cumulative, prefixWidths });
      }
    } else {
      let start = 0, clusterIndex = 0;
      while (start < run.text.length) {
        let end = Math.min(run.text.length, start + MAX_RUN_CHARS);
        while (clusterIndex < clusterRanges.length && clusterRanges[clusterIndex]![1] <= start) clusterIndex++;
        const cluster = clusterRanges[clusterIndex];
        if (cluster && cluster[0] < end && cluster[1] > end) end = cluster[0] > start ? cluster[0] : cluster[1];
        const width = start === 0 && end === run.text.length ? run.width : this.measureDirect(run.text.slice(start, end));
        const prefixWidths = new Map<number, number>([[0, 0], [end - start, width]]);
        cumulative += width;
        index.chunks.push({ start, end, width, cumulative, prefixWidths });
        start = end;
      }
    }
    this.runWidths.set(run, index);
    return index;
  }
  private chunkBoundaries(index: RunWidthIndex, chunk: RunWidthChunk): number[] {
    if (chunk.boundaries) return chunk.boundaries;
    const boundaries = [chunk.start];
    let clusterIndex = 0;
    while (clusterIndex < index.clusterRanges.length && index.clusterRanges[clusterIndex]![1] <= chunk.start) clusterIndex++;
    for (let position = chunk.start + 1; position <= chunk.end; position++) {
      while (clusterIndex < index.clusterRanges.length && index.clusterRanges[clusterIndex]![1] <= position) clusterIndex++;
      const cluster = index.clusterRanges[clusterIndex];
      if (!cluster || position <= cluster[0] || position >= cluster[1]) boundaries.push(position);
    }
    chunk.boundaries = boundaries;
    return boundaries;
  }
  private widthAt(run: ShapedRun, index: RunWidthIndex, offset: number): number {
    let lo = 0, hi = index.chunks.length;
    while (lo < hi) { const mid = Math.floor((lo + hi) / 2); if (index.chunks[mid]!.end < offset) lo = mid + 1; else hi = mid; }
    const chunk = index.chunks[Math.min(lo, index.chunks.length - 1)]!;
    if (offset >= chunk.end) return chunk.cumulative;
    if (offset <= chunk.start) return chunk.cumulative - chunk.width;
    const local = offset - chunk.start;
    let width = chunk.prefixWidths.get(local);
    if (width === undefined) {
      if (index.additive && this.advances) {
        const clusters: number[] = [];
        for (const [start, end] of index.clusterRanges) {
          if (start >= chunk.start && end <= offset) clusters.push(start - chunk.start, end - chunk.start);
        }
        width = this.advances.width(run.text.slice(chunk.start, offset), clusters);
      } else width = this.measureDirect(run.text.slice(chunk.start, offset));
      chunk.prefixWidths.set(local, width);
    }
    return chunk.cumulative - chunk.width + width;
  }
  private measureDirect(text: string): number {
    this.stats.measuredTextCalls++;
    this.stats.measuredTextChars += text.length;
    this.stats.maxMeasuredTextLength = Math.max(this.stats.maxMeasuredTextLength, text.length);
    return this.metrics.measureText(text).width;
  }
  setFont(font: LayoutFont): void {
    this.validateFont(font);
    if (JSON.stringify(font) === JSON.stringify(this.font)) return;
    this.font = font; this.setMetricsFont(font.font); this.invalidate();
  }
  invalidate(): void { this.cache.clear(); this.bytes = 0; this.widest = 0; this.advances = null; }
  private validateFont(font: LayoutFont): void {
    if (!font.font || !Number.isFinite(font.lineHeight) || font.lineHeight <= 0 || !Number.isFinite(font.baseline) || font.baseline < 0 || font.baseline > font.lineHeight) throw new RangeError('Invalid layout font');
  }
  private clustersForLine(lineText: string): number[] | null {
    let nonAscii = false;
    for (let i = 0; i < lineText.length; i++) {
      if (lineText.charCodeAt(i) > 0x7f) { nonAscii = true; break; }
    }
    if (!nonAscii) return [];
    const clusters: number[] = [];
    this.stats.segmentations++;
    for (const part of segmenter.segment(lineText)) {
      const end = part.index + part.segment.length;
      if (end - part.index > 1) {
        clusters.push(part.index, end);
        if (clusters.length / 2 > MAX_CLUSTERS_PER_LINE) return null;
      }
    }
    return clusters;
  }
  shape(number: number): ShapedLine {
    const hit = this.cache.get(number);
    if (hit) { this.cache.delete(number); this.cache.set(number, hit); this.widest = Math.max(this.widest, hit.line.width); return hit.line; }
    this.phases?.begin('shaping');
    try {
    const index = this.lineIndex(number);
    if (!index) throw new RangeError('Line outside document');
    this.setMetricsFont(this.font.font);
    const advances = this.stringDocumentMode ? null : (this.advances ??= new AdvanceTable((text) => this.measureDirect(text)));
    const lineText = this.slice(index.from, index.to);
    const clusters = this.clustersForLine(lineText);
    const additive = !!advances && advances.additive && !complexScript.test(lineText) && clusters !== null;
    const runs: ShapedRun[] = [];
    const space = advances?.ascii[0x20] ?? (this.measuredSpace ??= this.measureDirect(' '));
    const stop = Math.max(1, space * 4);
    let x = 0;
    let from = index.from;
    while (from < index.to) {
      const tabOffset = lineText.indexOf('\t', from - index.from);
      const tab = tabOffset < 0 ? -1 : index.from + tabOffset;
      const to = tab < 0 ? index.to : Math.min(tab, index.to);
      if (to > from) {
        const text = this.slice(from, to), measured: MeasuredChunk[] = [];
        const offset = from - index.from;
        const localClusters: number[] = [];
        if (clusters) for (let i = 0; i < clusters.length; i += 2) {
          const start = clusters[i]!, end = clusters[i + 1]!;
          if (start >= offset && end <= offset + text.length) localClusters.push(start - offset, end - offset);
        }
        let width: number;
        if (additive) {
          const parts = advances.chunks(text, localClusters, MAX_RUN_CHARS);
          measured.push(...parts);
          width = parts.reduce((sum, part) => sum + part.width, 0);
        } else width = this.measureText(text, clusters, offset, measured);
        const run = { text, from, to, x, width };
        this.runClusters.set(run, { lineFrom: index.from, clusters });
        this.runWidthIndex(run, measured).additive = additive;
        runs.push(run); x += width;
      }
      if (to === index.to) break;
      x = (Math.floor(x / stop) + 1) * stop; from = to + 1;
    }
    const line: ShapedLine = { number, from: index.from, to: index.to, runs, width: x, rtlUnsupported: rtl.test(lineText) };
    const clusterBytes = clusters === null ? 0 : clusters.length * 4;
    const bytes = 128 + runs.reduce((n, r) => n + 64 + r.text.length * 2, 0) + clusterBytes;
    while (this.bytes + bytes > this.cacheLimit && this.cache.size) {
      const first = this.cache.keys().next().value!; this.bytes -= this.cache.get(first)!.bytes; this.cache.delete(first); this.stats.evictions++;
    }
    if (bytes <= this.cacheLimit) { this.cache.set(number, { line, bytes, clusters }); this.bytes += bytes; }
    this.stats.builds++; this.widest = Math.max(this.widest, line.width); return line;
    } finally { this.phases?.end('shaping'); }
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
    for (const run of line.runs) {
      if (pos < run.from) return run.x;
      if (pos <= run.to) {
        const offset = pos - run.from;
        if (offset === 0) return run.x;
        const index = this.runWidthIndex(run);
        return run.x + this.widthAt(run, index, offset);
      }
    }
    return line.width;
  }
  offsetInRun(run: ShapedRun, x: number, bias: -1 | 1): number {
    if (x <= 0) return run.from; if (x >= run.width) return run.to;
    const index = this.runWidthIndex(run);
    let chunkLo = 0, chunkHi = index.chunks.length;
    while (chunkLo < chunkHi) {
      const mid = Math.floor((chunkLo + chunkHi) / 2);
      if (index.chunks[mid]!.cumulative < x) chunkLo = mid + 1; else chunkHi = mid;
    }
    const chunkAt = Math.min(chunkLo, index.chunks.length - 1);
    const chunk = index.chunks[chunkAt]!;
    const boundaries = this.chunkBoundaries(index, chunk);
    let lo = 0, hi = boundaries.length;
    while (lo < hi) {
      const mid = Math.floor((lo + hi) / 2);
      if (this.widthAt(run, index, boundaries[mid]!) < x) lo = mid + 1; else hi = mid;
    }
    return run.from + boundaries[Math.min(bias < 0 ? Math.max(0, lo - 1) : lo, boundaries.length - 1)]!;
  }
  runOffset(run: ShapedRun, offset: number): number {
    return this.widthAt(run, this.runWidthIndex(run), Math.max(0, Math.min(run.text.length, offset)));
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
