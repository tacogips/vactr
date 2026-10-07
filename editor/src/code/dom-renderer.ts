import type { Text } from '@codemirror/state';
import type { CodeAnnotation } from '../app/apis';
import { TextLayout, type LayoutViewport, type ShapedLine, type ShapedRun } from './layout';
import type { PhaseTimer } from './frame';
import { FALLBACK_PALETTE, type Palette } from './palette';
import type { CodeRenderer } from './renderer-types';
import type { GpuStatus, RenderFeedback } from './renderer';
import { DomOverlays } from './dom-overlay';

export const DOM_RENDERER_LIMITS = Object.freeze({ maxLines: 1024, maxSpansPerLine: 512, maxOverlayElements: 1024, maxHighlightKeys: 512 });
export function domWindowMargin(rows: number): number { return Math.max(8, Math.ceil(rows / 2)); }
export interface DomRendererOptions { onStatus?: (status: GpuStatus) => void }

interface LineEntry {
  line: HTMLDivElement;
  number: HTMLDivElement;
  runs: readonly ShapedRun[] | null;
  lineNumber: number;
  signature: string;
  nodeCount: number;
  lineChildren: number;
  numberHasText: boolean;
  inWindow: boolean;
}

const TOKEN_CLASSES: Readonly<Record<string, string>> = Object.freeze({
  'vact-tok-comment': 'vact-dom-tok-comment', 'vact-tok-directive': 'vact-dom-tok-directive',
  'vact-tok-keyword': 'vact-dom-tok-keyword', 'vact-tok-number': 'vact-dom-tok-number',
  'vact-tok-string': 'vact-dom-tok-string', 'vact-tok-path': 'vact-dom-tok-path',
  'vact-tok-head': 'vact-dom-tok-head', 'vact-tok-bracket': 'vact-dom-tok-bracket',
});

export class DomRenderer implements CodeRenderer {
  readonly stats = {
    frames: 0, lineBuilds: 0, lineMoves: 0, lineReuses: 0, lineReleases: 0,
    gutterWrites: 0, overlayWrites: 0, caretWrites: 0, highlightToggles: 0,
    highlightBuilds: 0, highlightDropped: 0, transformWrites: 0, windowShifts: 0,
    windowFrom: 0, windowTo: 0, liveLines: 0, liveNodes: 0, peakNodes: 0,
    spansMerged: 0, overlayDropped: 0,
  };
  readonly gutter: HTMLDivElement;
  readonly gutterRows: HTMLDivElement;
  readonly text: HTMLDivElement;
  readonly content: HTMLDivElement;
  readonly lines: HTMLDivElement;
  readonly overlays: DomOverlays;
  readonly layout: TextLayout;
  readonly textPending = false;
  private view: LayoutViewport = { width: 1, height: 1, scrollLeft: 0, scrollTop: 0 };
  private requestedDpr = 1;
  private statusValue: GpuStatus;
  private palette: Palette = FALLBACK_PALETTE;
  private lastDoc: Text | null = null;
  private textRevision = 0;
  private lastTextRevision = -1;
  private lastWindow = '';
  private lastSyntax = '';
  private lastFont = '';
  private lastRenderedFont = '';
  private lastChunkView = '';
  private lastContentTransform = '';
  private lastGutterTransform = '';
  private readonly lineByRuns = new WeakMap<readonly ShapedRun[], LineEntry>();
  private readonly activeLines = new Map<number, LineEntry>();
  private readonly freeLines: LineEntry[] = [];
  private readonly fontDocument: Document;
  private disposed = false;
  private readonly fontsChanged = (): void => {
    if (this.disposed) return;
    this.layout.setFont({ ...this.layout.font, generation: (this.layout.font.generation ?? 0) + 1 });
    this.lastRenderedFont = '';
    this.lastFont = '';
  };

  constructor(readonly element: HTMLElement, layout: TextLayout, private readonly options: DomRendererOptions = {}) {
    this.layout = layout;
    this.fontDocument = element.ownerDocument;
    element.classList.add('vact-code-dom');
    element.setAttribute('aria-hidden', 'true');
    this.gutter = this.div('vact-dom-gutter');
    this.gutterRows = this.div('vact-dom-gutter-rows');
    this.gutter.append(this.gutterRows);
    this.text = this.div('vact-dom-text');
    this.content = this.div('vact-dom-content');
    this.lines = this.div('vact-dom-lines');
    this.content.append(this.lines);
    this.text.append(this.content);
    element.append(this.gutter, this.text);
    this.stats.liveNodes = 5;
    this.stats.peakNodes = 5;
    this.overlays = new DomOverlays(this.content, layout, this.stats, delta => this.adjustNodes(delta));
    this.ensureStylesheet();
    this.syncFont();
    this.statusValue = { kind: 'ready', message: 'DOM renderer', effectiveDpr: this.requestedDpr, saveText: () => this.layout.document };
    this.options.onStatus?.(this.statusValue);
    this.fontDocument.fonts?.addEventListener('loadingdone', this.fontsChanged);
  }

  get status(): GpuStatus { return this.statusValue; }
  setPalette(palette: Palette): void { this.palette = palette; }
  setPhases(phases: PhaseTimer | null): void { this.layout.phases = phases; }
  setText(doc: Text): void {
    if (this.disposed || doc === this.lastDoc) return;
    this.layout.setText(doc);
    this.lastDoc = doc;
    this.textRevision++;
  }
  setViewport(view: LayoutViewport, dpr = 1): void {
    if (![view.width, view.height, view.scrollLeft, view.scrollTop, view.left ?? 0, view.top ?? 0, view.gutter ?? 48].every(Number.isFinite) || view.width <= 0 || view.height <= 0 || view.scrollLeft < 0 || view.scrollTop < 0 || dpr <= 0 || !Number.isFinite(dpr)) throw new RangeError('Invalid viewport');
    this.view = { ...view };
    this.requestedDpr = dpr;
    if (this.statusValue && this.statusValue.effectiveDpr !== dpr) this.reportStatus();
  }

  render(feedback: RenderFeedback = {}): boolean {
    if (this.disposed) return false;
    this.syncFont();
    const lineHeight = this.layout.font.lineHeight;
    const lineCount = this.layout.lineCount;
    const rows = Math.ceil(this.view.height / lineHeight);
    const margin = domWindowMargin(rows);
    const visibleFrom = Math.max(0, Math.min(lineCount, Math.floor(this.view.scrollTop / lineHeight)));
    const visibleTo = Math.max(visibleFrom, Math.min(lineCount, Math.ceil((this.view.scrollTop + this.view.height) / lineHeight)));
    let from = this.stats.windowFrom, to = this.stats.windowTo;
    if (visibleFrom < from || visibleTo > to || to > lineCount) {
      const maxLines = DOM_RENDERER_LIMITS.maxLines;
      from = Math.max(0, visibleFrom - margin);
      to = Math.min(lineCount, visibleTo + margin);
      if (to - from > maxLines) {
        const center = Math.floor((visibleFrom + visibleTo) / 2);
        from = Math.max(0, Math.min(lineCount - maxLines, center - Math.floor(maxLines / 2)));
        to = Math.min(lineCount, from + maxLines);
      }
      this.stats.windowFrom = from;
      this.stats.windowTo = to;
      this.stats.windowShifts++;
    }
    const windowKey = `${from}:${to}`;
    const annotations = feedback.annotations ?? [];
    const syntaxRows = annotations.filter(row => row.kind === 'syntax');
    const syntaxKey = syntaxRows.map(row => `${row.from}:${row.to}:${row.className ?? ''}`).join(';');
    const textChanged = this.lastTextRevision !== (feedback.textRevision ?? this.textRevision);
    const windowChanged = this.lastWindow !== windowKey;
    const syntaxChanged = this.lastSyntax !== syntaxKey;
    const fontChanged = this.lastRenderedFont !== this.fontKey();
    const chunkView = `${this.view.scrollLeft}:${this.view.width}`;
    const chunkViewChanged = this.lastChunkView !== chunkView;
    if (textChanged || windowChanged || syntaxChanged || fontChanged || chunkViewChanged) {
      this.renderLines(from, to, syntaxRows);
      this.lastTextRevision = feedback.textRevision ?? this.textRevision;
      this.lastWindow = windowKey;
      this.lastSyntax = syntaxKey;
      this.lastRenderedFont = this.fontKey();
      this.lastChunkView = chunkView;
    }
    const contentView = this.contentView(from, to);
    const staticAnnotations = annotations.filter(row => row.kind !== 'playing' && row.kind !== 'eval');
    this.overlays.updateStatic(staticAnnotations, contentView, this.lastTextRevision);
    this.overlays.updateCaret(feedback, contentView);
    this.overlays.updateHighlights([...annotations, ...(feedback.animated ?? [])], contentView, this.lastTextRevision);
    this.writeTransforms();
    this.stats.frames++;
    this.stats.liveLines = this.activeLines.size;
    return true;
  }

  private renderLines(from: number, to: number, syntax: readonly CodeAnnotation[]): void {
    const wanted = new Set<LineEntry>();
    const lineHeight = this.layout.font.lineHeight;
    const previousEntries = new Set(this.activeLines.values());
    this.activeLines.clear();
    let syntaxIndex = 0;
    for (let number = from; number < to; number++) {
      const shaped = this.layout.shape(number);
      while (syntaxIndex < syntax.length && syntax[syntaxIndex]!.to <= shaped.from) syntaxIndex++;
      const lineSyntax: CodeAnnotation[] = [];
      for (let index = syntaxIndex; index < syntax.length && syntax[index]!.from < shaped.to; index++) {
        if (syntax[index]!.to > shaped.from) lineSyntax.push(syntax[index]!);
      }
      let entry = this.lineByRuns.get(shaped.runs);
      if (!entry) {
        const pooled = this.freeLines.pop();
        entry = pooled ?? this.createLineEntry();
        if (pooled) this.adjustNodes(2);
        if (entry.runs) this.lineByRuns.delete(entry.runs);
        entry.runs = shaped.runs;
        this.lineByRuns.set(shaped.runs, entry);
        entry.signature = '';
      }
      wanted.add(entry);
      const signature = this.signature(shaped, lineSyntax);
      if (entry.signature !== signature) {
        this.buildLine(entry, shaped, lineSyntax);
        entry.signature = signature;
        this.stats.lineBuilds++;
      } else if (entry.lineNumber === number) {
        this.stats.lineReuses++;
      }
      if (entry.lineNumber !== number) {
        entry.lineNumber = number;
        this.writeStyle(entry.line, 'transform', `translateY(${number * lineHeight}px)`);
        this.writeStyle(entry.number, 'transform', `translateY(${number * lineHeight}px)`);
        this.setLineNumber(entry, String(number + 1));
        this.stats.lineMoves++;
        this.stats.gutterWrites++;
      }
      if (!entry.inWindow) {
        this.lines.append(entry.line);
        this.gutterRows.append(entry.number);
        entry.inWindow = true;
      }
      this.activeLines.set(number, entry);
    }
    for (const entry of previousEntries) {
      if (wanted.has(entry)) continue;
      entry.line.remove(); entry.number.remove();
      entry.inWindow = false;
      if (entry.runs) this.lineByRuns.delete(entry.runs);
      entry.runs = null;
      entry.line.replaceChildren(); entry.number.replaceChildren();
      this.adjustNodes(-entry.nodeCount);
      entry.nodeCount = 0;
      entry.lineChildren = 0; entry.numberHasText = false;
      this.freeLines.push(entry);
      this.stats.lineReleases++;
      if (this.freeLines.length > DOM_RENDERER_LIMITS.maxLines) this.freeLines.shift();
    }
    this.stats.windowFrom = from; this.stats.windowTo = to;
    this.stats.liveLines = wanted.size;
  }

  private createLineEntry(): LineEntry {
    const line = this.div('vact-dom-line');
    const number = this.div('vact-dom-num');
    this.adjustNodes(2);
    return { line, number, runs: null, lineNumber: -1, signature: '', nodeCount: 2, lineChildren: 0, numberHasText: false, inWindow: false };
  }

  private signature(line: ShapedLine, syntax: readonly CodeAnnotation[]): string {
    const syntaxPart = syntax.map(row => `${Math.max(0, row.from - line.from)}:${Math.min(line.to, row.to) - line.from}:${row.className ?? ''}`).join(';');
    const chunks = this.visibleChunks(line);
    return `${this.fontKey()}|${syntaxPart}|${chunks.map(chunk => chunk.chunk).join(',')}`;
  }

  private buildLine(entry: LineEntry, line: ShapedLine, syntax: readonly CodeAnnotation[]): void {
    const pieces = this.visibleChunks(line);
    const children: Node[] = [];
    let childNodes = 0;
    let spans = 0;
    for (const piece of pieces) {
      for (const run of piece.runs) {
        const wrapper = this.fontDocument.createElement('span');
        wrapper.className = 'vact-dom-run';
        wrapper.style.left = `${run.x}px`;
        children.push(wrapper);
        const rows = syntax.filter(row => row.from < run.to && row.to > run.from);
        const boundaries = new Set<number>([run.from, run.to]);
        for (const row of rows) { boundaries.add(Math.max(run.from, row.from)); boundaries.add(Math.min(run.to, row.to)); }
        const offsets = [...boundaries].sort((a, b) => a - b);
        for (let index = 0; index < offsets.length - 1; index++) {
          const start = offsets[index]!, end = offsets[index + 1]!;
          if (end <= start) continue;
          const row = rows.find(style => style.from <= start && style.to >= end && style.className);
          const text = run.text.slice(start - run.from, end - run.from);
          const className = row?.className ? TOKEN_CLASSES[row.className] : undefined;
          if (className && spans < DOM_RENDERER_LIMITS.maxSpansPerLine) {
            const token = this.fontDocument.createElement('span'); token.className = className;
            token.append(this.fontDocument.createTextNode(text)); wrapper.append(token); spans++;
            childNodes += 2;
          } else {
            if (className) this.stats.spansMerged++;
            wrapper.append(this.fontDocument.createTextNode(text));
            childNodes++;
          }
        }
        childNodes++;
      }
    }
    entry.line.replaceChildren(...children);
    const delta = childNodes - entry.lineChildren;
    this.adjustNodes(delta);
    entry.lineChildren = childNodes;
    this.setLineNumber(entry, String(line.number + 1));
    entry.nodeCount = 2 + entry.lineChildren + (entry.numberHasText ? 1 : 0);
  }

  private setLineNumber(entry: LineEntry, value: string): void {
    if (entry.number.textContent === value) return;
    const hasText = value.length > 0;
    entry.number.textContent = value;
    if (hasText !== entry.numberHasText) {
      this.adjustNodes(hasText ? 1 : -1);
      entry.numberHasText = hasText;
    }
    entry.nodeCount = 2 + entry.lineChildren + (entry.numberHasText ? 1 : 0);
    this.stats.gutterWrites++;
  }

  private chunkVisible(runs: readonly ShapedRun[]): boolean {
    const first = runs[0], last = runs[runs.length - 1];
    const from = first?.x ?? 0, to = last ? last.x + last.width : 0;
    return to >= Math.max(0, this.view.scrollLeft - this.view.width) && from <= this.view.scrollLeft + this.view.width * 2;
  }

  private visibleChunks(line: ShapedLine) {
    const chunks = this.layout.geometrySegments(line);
    return chunks.length <= 1 ? chunks : chunks.filter(chunk => this.chunkVisible(chunk.runs));
  }

  private contentView(from: number, to: number): LayoutViewport {
    const top = from * this.layout.font.lineHeight;
    return { left: 0, top, width: 2 ** 24, height: Math.max(1, (to - from) * this.layout.font.lineHeight), gutter: 0,
      scrollLeft: 0, scrollTop: top };
  }

  private writeTransforms(): void {
    const gutterWidth = `${this.view.gutter ?? 48}px`;
    if (this.gutter.style.width !== gutterWidth) { this.gutter.style.width = gutterWidth; this.stats.gutterWrites++; }
    if (this.text.style.left !== gutterWidth) { this.text.style.left = gutterWidth; this.stats.gutterWrites++; }
    const content = `translate3d(${-this.view.scrollLeft}px, ${-this.view.scrollTop}px, 0)`;
    const gutter = `translate3d(0, ${-this.view.scrollTop}px, 0)`;
    if (content !== this.lastContentTransform) { this.content.style.transform = content; this.lastContentTransform = content; this.stats.transformWrites++; }
    if (gutter !== this.lastGutterTransform) { this.gutterRows.style.transform = gutter; this.lastGutterTransform = gutter; this.stats.transformWrites++; }
  }

  private syncFont(): void {
    const key = this.fontKey();
    if (key === this.lastFont) return;
    const font = this.layout.font;
    this.content.style.font = font.font;
    this.content.style.lineHeight = `${font.lineHeight}px`;
    this.gutterRows.style.font = font.font;
    this.gutterRows.style.lineHeight = `${font.lineHeight}px`;
    this.lastFont = key;
    for (const entry of this.activeLines.values()) entry.signature = '';
  }

  private fontKey(): string { return `${this.layout.font.font}:${this.layout.font.lineHeight}:${this.layout.font.generation ?? 0}`; }
  private writeStyle(element: HTMLElement, key: string, value: string): void {
    if (element.style.getPropertyValue(key) === value) return;
    element.style.setProperty(key, value);
  }

  private ensureStylesheet(): void {
    if (this.fontDocument.querySelector('link[data-vact-dom-css]')) return;
    const link = this.fontDocument.createElement('link');
    link.rel = 'stylesheet';
    link.href = new URL('./dom-renderer.css', import.meta.url).href;
    link.dataset.vactDomCss = 'true';
    this.fontDocument.head.append(link);
  }

  private reportStatus(): void {
    this.statusValue = { ...this.statusValue, kind: 'ready', message: 'DOM renderer', effectiveDpr: this.requestedDpr, saveText: () => this.layout.document };
    this.options.onStatus?.(this.statusValue);
  }

  private div(className: string): HTMLDivElement { const element = this.fontDocument.createElement('div'); element.className = className; return element; }
  private adjustNodes(delta: number): void {
    this.stats.liveNodes = Math.max(0, this.stats.liveNodes + delta);
    this.stats.peakNodes = Math.max(this.stats.peakNodes, this.stats.liveNodes);
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.fontDocument.fonts?.removeEventListener('loadingdone', this.fontsChanged);
    this.overlays.dispose();
    this.element.replaceChildren();
    this.activeLines.clear(); this.freeLines.length = 0;
    this.stats.liveLines = 0; this.stats.liveNodes = 0;
    this.stats.windowFrom = 0; this.stats.windowTo = 0;
  }
}
