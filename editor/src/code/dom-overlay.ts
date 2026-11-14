import type { CodeAnnotation, CodeRect } from '../app/apis';
import type { LayoutViewport, TextLayout } from './layout';
import type { RenderFeedback } from './renderer';

type Stats = Record<string, number>;
const MAX_OVERLAY_ELEMENTS = 1024;
interface HighlightEntry { elements: HTMLDivElement[]; on: boolean; geometry: string }

function setStyle(element: HTMLElement, property: string, value: string, changed: () => void): void {
  if (element.style.getPropertyValue(property) === value) return;
  element.style.setProperty(property, value);
  changed();
}

export class DomOverlays {
  readonly selectionLayer: HTMLDivElement;
  readonly highlightLayer: HTMLDivElement;
  readonly underlineLayer: HTMLDivElement;
  readonly labelLayer: HTMLDivElement;
  readonly caret: HTMLDivElement;
  readonly handles: [HTMLDivElement, HTMLDivElement];
  private selectionNodes: HTMLDivElement[] = [];
  private underlineNodes: HTMLDivElement[] = [];
  private labelNodes: HTMLDivElement[] = [];
  private labelTextNodes = new Set<HTMLDivElement>();
  private highlightKeys = new Map<string, HighlightEntry>();
  private readonly highlightPool: HTMLDivElement[] = [];
  private highlightElementCount = 0;
  private selectionKey = '';
  private staticKey = '';
  private caretKey = '';
  private handlesKey = '';
  private disposed = false;

  constructor(
    private readonly content: HTMLElement,
    private readonly layout: TextLayout,
    private readonly stats: Stats,
    private readonly nodeDelta: (delta: number) => void,
  ) {
    const doc = content.ownerDocument;
    this.selectionLayer = this.layer(doc, 'vact-dom-sel');
    this.highlightLayer = this.layer(doc, 'vact-dom-hl');
    this.underlineLayer = this.layer(doc, 'vact-dom-under');
    this.labelLayer = this.layer(doc, 'vact-dom-labels');
    this.caret = this.layer(doc, 'vact-dom-caret');
    const first = this.layer(doc, 'vact-dom-handle');
    const second = this.layer(doc, 'vact-dom-handle');
    this.handles = [first, second];
    content.append(this.selectionLayer, this.highlightLayer, this.underlineLayer, this.labelLayer, this.caret, first, second);
    this.nodeDelta(7);
  }

  private layer(doc: Document, className: string): HTMLDivElement {
    const element = doc.createElement('div');
    element.className = className;
    return element;
  }

  updateStatic(annotations: readonly CodeAnnotation[], view: LayoutViewport, revision: number): void {
    const relevant = annotations.filter(row => row.kind === 'selection' || row.kind === 'diagnostic' || row.kind === 'composition' || row.kind === 'call-head' || (row.kind === 'binding' && row.label));
    const key = relevant.map(row => `${row.kind}:${row.from}:${row.to}:${row.label ?? ''}`).join('|') + `@${revision}:${view.top}:${view.height}:${this.layout.font.generation ?? 0}`;
    if (key === this.staticKey) return;
    this.staticKey = key;
    const selection = relevant.filter(row => row.kind === 'selection');
    let selectionDropped = 0;
    const selectionKey = selection.map(row => `${row.from}:${row.to}`).join('|') + `@${revision}:${view.top}:${view.height}:${this.layout.font.generation ?? 0}`;
    if (selectionKey !== this.selectionKey) {
      this.selectionKey = selectionKey;
      const rects = selection.flatMap(row => this.layout.rangeRects(row, view));
      selectionDropped = Math.max(0, rects.length - 1024);
      this.syncRectNodes(this.selectionLayer, this.selectionNodes, rects, 'vact-dom-selr', () => this.stats.overlayWrites++);
      this.stats.overlayWrites++;
    }
    const bars: Array<{ rect: CodeRect; kind: string }> = [];
    const labels: Array<{ rect: CodeRect; text: string }> = [];
    for (const row of relevant) {
      if (row.kind === 'diagnostic' || row.kind === 'composition' || row.kind === 'call-head') {
        for (const rect of this.layout.rangeRects(row, view)) {
          const height = row.kind === 'call-head' ? 1 : 2;
          bars.push({ rect: { ...rect, top: rect.bottom - height }, kind: row.kind });
        }
      } else if (row.kind === 'binding' && row.label) {
        const rect = this.layout.coordsAtPos(row.to, view);
        if (rect) labels.push({ rect, text: row.label.length > 64 ? `${row.label.slice(0, 61)}…` : row.label });
      }
    }
    this.syncRectNodes(this.underlineLayer, this.underlineNodes, bars.map(row => row.rect), 'vact-dom-bar', () => this.stats.overlayWrites++);
    bars.slice(0, this.underlineNodes.length).forEach((row, index) => {
      const kind = row.kind === 'diagnostic' ? 'vact-dom-diag' : row.kind === 'composition' ? 'vact-dom-comp' : 'vact-dom-call';
      this.classIfChanged(this.underlineNodes[index]!, `vact-dom-bar ${kind}`, () => this.stats.overlayWrites++);
    });
    this.syncLabels(labels, view);
    this.stats.overlayDropped += selectionDropped + Math.max(0, rectsCount(bars.length, 1024)) + Math.max(0, labels.length - 1024);
  }

  private syncLabels(rows: Array<{ rect: CodeRect; text: string }>, view: LayoutViewport): void {
    const selected = rows.slice(0, 1024);
    while (this.labelNodes.length < selected.length) {
      const node = this.content.ownerDocument.createElement('div'); node.className = 'vact-dom-label'; this.labelLayer.append(node); this.labelNodes.push(node); this.nodeDelta(1);
    }
    while (this.labelNodes.length > selected.length) {
      const node = this.labelNodes.pop()!; node.remove();
      this.nodeDelta(this.labelTextNodes.delete(node) ? -2 : -1);
    }
    selected.forEach(({ rect, text }, index) => {
      const node = this.labelNodes[index]!;
      if (node.textContent !== text) {
        node.textContent = text;
        const hasText = text.length > 0;
        const hadText = this.labelTextNodes.has(node);
        if (hasText !== hadText) this.nodeDelta(hasText ? 1 : -1);
        if (hasText) this.labelTextNodes.add(node); else this.labelTextNodes.delete(node);
        this.stats.overlayWrites++;
      }
      const width = Math.min(256, Math.max(16, text.length * 8 + 8));
      this.position(node, rect.left + 3, rect.top, width, this.layout.font.lineHeight, () => this.stats.overlayWrites++);
    });
  }

  updateCaret(feedback: RenderFeedback, view: LayoutViewport): void {
    const pos = feedback.cursorVisible !== false ? feedback.cursor : null;
    const rect = pos == null ? null : this.layout.coordsAtPos(pos, view);
    const next = rect ? `${rect.left}:${rect.top}:${rect.bottom}` : 'hidden';
    if (next !== this.caretKey) {
      this.caretKey = next;
      setStyle(this.caret, 'visibility', rect ? 'visible' : 'hidden', () => this.stats.caretWrites++);
      if (rect) this.position(this.caret, rect.left, rect.top, 1, rect.bottom - rect.top, () => this.stats.caretWrites++);
    }
    const handles = feedback.handles ?? [];
    const key = handles.map(handle => `${handle.pos}:${handle.end ? 1 : 0}`).join('|') + `@${view.top}:${view.height}`;
    if (key !== this.handlesKey) {
      this.handlesKey = key;
      this.handles.forEach((node, index) => {
        const handle = handles[index];
        const point = handle ? this.layout.coordsAtPos(handle.pos, view) : null;
        setStyle(node, 'visibility', point ? 'visible' : 'hidden', () => this.stats.caretWrites++);
        if (handle && point) {
          const y = handle.end ? point.bottom : point.top;
          this.position(node, point.left - 5, y - 4, 10, 10, () => this.stats.caretWrites++);
        }
      });
    }
  }

  updateHighlights(rows: readonly CodeAnnotation[], view: LayoutViewport, revision: number): void {
    if (this.disposed) return;
    const active = new Set<string>();
    const animated = rows.filter(row => row.kind === 'playing' || row.kind === 'eval');
    for (const row of animated) {
      const key = `${row.kind}:${row.className ?? ''}:${row.from}-${row.to}`;
      if (active.has(key)) continue;
      if (active.size >= 512) { this.stats.highlightDropped++; continue; }
      active.add(key);
      const geometry = `${revision}:${view.top}:${view.height}:${this.layout.font.generation ?? 0}`;
      let entry = this.highlightKeys.get(key);
      if (!entry) {
        entry = { elements: [], on: false, geometry: '' };
        this.highlightKeys.set(key, entry);
      } else { this.highlightKeys.delete(key); this.highlightKeys.set(key, entry); }
      if (entry.geometry !== geometry) {
        const rects = this.layout.rangeRects(row, view);
        this.syncHighlightRects(entry, rects, row.kind, row.className ?? '');
        entry.geometry = geometry;
      }
      if (!entry.on) {
        for (const element of entry.elements) {
          element.classList.add('on');
          this.stats.highlightToggles++;
        }
        entry.on = true;
      }
    }
    for (const [key, entry] of this.highlightKeys) {
      if (active.has(key) || !entry.on) continue;
      for (const element of entry.elements) {
        element.classList.remove('on');
        this.stats.highlightToggles++;
      }
      entry.on = false;
    }
    this.evictHighlights();
  }

  private syncHighlightRects(entry: HighlightEntry, rects: readonly CodeRect[], kind: string, className: string): void {
    const rows = rects.slice(0, Math.max(0, MAX_OVERLAY_ELEMENTS - this.highlightElementCount + entry.elements.length));
    this.stats.overlayDropped += Math.max(0, rects.length - rows.length);
    while (entry.elements.length < rows.length) {
      const element = this.highlightPool.pop() ?? this.content.ownerDocument.createElement('div');
      if (!element.parentElement) { this.highlightLayer.append(element); this.nodeDelta(1); this.highlightElementCount++; }
      entry.elements.push(element);
    }
    while (entry.elements.length > rows.length) {
      const element = entry.elements.pop()!; element.classList.remove('on'); element.remove(); this.nodeDelta(-1); this.highlightElementCount--; this.highlightPool.push(element);
    }
    rows.forEach((rect, index) => {
      const element = entry.elements[index]!;
      const type = kind === 'playing' ? 'vact-dom-playing' : className.includes('error') ? 'vact-dom-flash-error' : 'vact-dom-flash';
      const elementClass = `vact-dom-hlr ${type}${entry.on ? ' on' : ''}`;
      const values = [rect.left, rect.top, Math.max(1, rect.right - rect.left), Math.max(1, rect.bottom - rect.top)].map(value => `${value}px`);
      const properties = ['left', 'top', 'width', 'height'];
      let changed = element.className !== elementClass;
      if (changed) element.className = elementClass;
      properties.forEach((property, propertyIndex) => {
        const value = values[propertyIndex]!;
        if (element.style.getPropertyValue(property) === value) return;
        element.style.setProperty(property, value);
        changed = true;
      });
      if (changed) this.stats.highlightBuilds++;
    });
  }

  private evictHighlights(): void {
    for (const [key, entry] of this.highlightKeys) {
      if (this.highlightKeys.size <= 512) break;
      if (entry.on) continue;
      this.highlightKeys.delete(key);
      for (const element of entry.elements) { element.remove(); this.nodeDelta(-1); this.highlightElementCount--; this.highlightPool.push(element); }
    }
  }

  private syncRectNodes(layer: HTMLElement, nodes: HTMLDivElement[], rects: readonly CodeRect[], className: string, changed: () => void): void {
    const rows = rects.slice(0, 1024);
    while (nodes.length < rows.length) {
      const node = this.content.ownerDocument.createElement('div'); node.className = className; layer.append(node); nodes.push(node); this.nodeDelta(1); changed();
    }
    while (nodes.length > rows.length) { nodes.pop()!.remove(); this.nodeDelta(-1); changed(); }
    rows.forEach((rect, index) => this.position(nodes[index]!, rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top, changed));
  }

  private position(element: HTMLElement, left: number, top: number, width: number, height: number, changed: () => void): void {
    setStyle(element, 'left', `${left}px`, changed);
    setStyle(element, 'top', `${top}px`, changed);
    setStyle(element, 'width', `${Math.max(1, width)}px`, changed);
    setStyle(element, 'height', `${Math.max(1, height)}px`, changed);
  }

  private classIfChanged(element: HTMLElement, value: string, changed: () => void): void {
    if (element.className === value) return;
    element.className = value;
    changed();
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    const childCount = this.content.childNodes.length;
    this.content.replaceChildren();
    this.nodeDelta(-childCount);
    this.selectionNodes = []; this.underlineNodes = []; this.labelNodes = [];
    this.labelTextNodes.clear(); this.highlightElementCount = 0;
    this.highlightKeys.clear(); this.highlightPool.length = 0;
  }
}

function rectsCount(count: number, cap: number): number { return Math.max(0, count - cap); }
