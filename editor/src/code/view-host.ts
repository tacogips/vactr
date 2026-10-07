import type { CodeSurface } from './surface';
import type { FrameHost, ViewportInfo } from './frame';
import type { LayoutViewport, TextLayout } from './layout';
import type { PhaseTimer } from './frame';

/** Scroll state and coordinate bridge for the canvas; it owns no document text. */
export class CodeViewHost {
  private scrollLeft = 0;
  private scrollTop = 0;
  private width = 1;
  private height = 1;
  private inset = 0;
  private focusInput: () => void = () => {};
  private readonly cleanups: (() => void)[] = [];
  private readonly detachBridge: () => void;
  private phases: PhaseTimer | null = null;
  private rect: { left: number; top: number } | null = null;
  private rectStale = true;
  private lastEvaluated: { state: CodeSurface['state']; scrollTop: number; height: number; inset: number } | null = null;
  readonly stats = { caretEvaluations: 0, rectReads: 0 };

  constructor(private readonly element: HTMLElement, private readonly surface: CodeSurface,
    private readonly layout: TextLayout, private readonly frameHost: FrameHost,
    private readonly onViewportChange: () => void) {
    this.detachBridge = surface.attachBridge({
      focus: () => this.focusInput(),
      posAtCoords: (coords) => layout.posAtCoords(coords, this.viewport),
      coordsAtPos: (pos) => layout.coordsAtPos(pos, this.viewport),
    });
    this.listen(element, 'wheel', (event) => {
      const wheel = event as WheelEvent;
      if (wheel.deltaX === 0 && wheel.deltaY === 0) return;
      wheel.preventDefault(); this.scrollBy(wheel.deltaX, wheel.deltaY);
    }, { passive: false });
    this.listen(element, 'pointerdown', () => this.invalidateRect());
    this.listen(window, 'scroll', () => this.invalidateRect(), { capture: true, passive: true });
    this.listen(window, 'resize', () => this.invalidateRect());
    this.cleanups.push(surface.subscribe((update) => {
      if (update.selectionSet) this.keepCaretVisible();
    }));
  }

  get viewport(): LayoutViewport {
    const rect = this.readRect();
    return { width: Math.max(1, this.width), height: Math.max(1, this.height - this.inset),
      scrollLeft: this.scrollLeft, scrollTop: this.scrollTop, left: rect.left, top: rect.top, gutter: 48 };
  }

  get scrollPosition(): { left: number; top: number } { return { left: this.scrollLeft, top: this.scrollTop }; }
  setPhases(phases: PhaseTimer | null): void { this.phases = phases; }
  setFocus(focus: () => void): void { this.focusInput = focus; }

  setViewport(info: ViewportInfo): void {
    this.width = Math.max(1, info.width); this.height = Math.max(1, info.height); this.inset = Math.max(0, info.keyboardInset);
    this.invalidateRect();
    this.clampScroll(); this.keepCaretVisible(); this.onViewportChange();
  }

  scrollBy(dx: number, dy: number): void {
    this.scrollLeft = Math.max(0, this.scrollLeft + dx); this.scrollTop = Math.max(0, this.scrollTop + dy);
    this.clampScroll(); this.onViewportChange();
  }

  scrollCaret(): void { this.keepCaretVisible(); }

  invalidateRect(): void { this.rectStale = true; }
  refreshRect(): void { this.readRect(); }

  dispose(): void { for (const remove of this.cleanups.splice(0)) remove(); this.detachBridge(); this.focusInput = () => {}; }

  private keepCaretVisible(): void {
    this.phases?.begin('caret');
    try {
    const height = Math.max(1, this.height - this.inset);
    const state = this.surface.state;
    const previous = this.lastEvaluated;
    if (previous?.state === state && previous.scrollTop === this.scrollTop && previous.height === this.height && previous.inset === this.inset) return;
    this.stats.caretEvaluations++;
    const line = state.doc.lineAt(state.selection.main.head).number - 1;
    const top = line * this.layout.font.lineHeight, bottom = top + this.layout.font.lineHeight;
    if (top < this.scrollTop) this.scrollTop = top;
    else if (bottom > this.scrollTop + height) this.scrollTop = bottom - height;
    this.clampScroll();
    this.lastEvaluated = { state, scrollTop: this.scrollTop, height: this.height, inset: this.inset };
    } finally { this.phases?.end('caret'); }
  }

  private clampScroll(): void {
    const maxTop = Math.max(0, this.surface.state.doc.lines * this.layout.font.lineHeight - Math.max(1, this.height - this.inset));
    this.scrollTop = Math.min(this.scrollTop, maxTop);
    this.scrollLeft = Math.min(this.scrollLeft, Math.max(0, this.layout.widestShaped - this.width + 48));
  }

  private readRect(): { left: number; top: number } {
    if (this.rectStale || !this.rect) {
      const rect = this.element.getBoundingClientRect();
      this.stats.rectReads++; this.rect = { left: rect.left, top: rect.top }; this.rectStale = false;
    }
    return this.rect;
  }

  private listen(target: EventTarget, name: string, fn: EventListener, options?: AddEventListenerOptions): void {
    target.addEventListener(name, fn, options); this.cleanups.push(() => target.removeEventListener(name, fn, options));
  }
}
