import type { CodeSurface } from './surface';
import type { FrameHost, ViewportInfo } from './frame';
import type { LayoutViewport, TextLayout } from './layout';

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
    this.cleanups.push(surface.subscribe((update) => {
      if (update.selectionSet) this.keepCaretVisible();
    }));
  }

  get viewport(): LayoutViewport {
    const rect = this.element.getBoundingClientRect();
    return { width: Math.max(1, this.width), height: Math.max(1, this.height - this.inset),
      scrollLeft: this.scrollLeft, scrollTop: this.scrollTop, left: rect.left, top: rect.top, gutter: 48 };
  }

  get scrollPosition(): { left: number; top: number } { return { left: this.scrollLeft, top: this.scrollTop }; }
  setFocus(focus: () => void): void { this.focusInput = focus; }

  setViewport(info: ViewportInfo): void {
    this.width = Math.max(1, info.width); this.height = Math.max(1, info.height); this.inset = Math.max(0, info.keyboardInset);
    this.clampScroll(); this.keepCaretVisible(); this.onViewportChange();
  }

  scrollBy(dx: number, dy: number): void {
    this.scrollLeft = Math.max(0, this.scrollLeft + dx); this.scrollTop = Math.max(0, this.scrollTop + dy);
    this.clampScroll(); this.onViewportChange();
  }

  scrollCaret(): void { this.keepCaretVisible(); }

  dispose(): void { for (const remove of this.cleanups.splice(0)) remove(); this.detachBridge(); this.focusInput = () => {}; }

  private keepCaretVisible(): void {
    const rect = this.element.getBoundingClientRect(); const caret = this.surface.coordsAtPos(this.surface.state.selection.main.head);
    if (!caret) return;
    const height = Math.max(1, this.height - this.inset);
    if (caret.top < rect.top) this.scrollTop = Math.max(0, this.scrollTop - (rect.top - caret.top));
    else if (caret.bottom > rect.top + height) this.scrollTop += caret.bottom - (rect.top + height);
    this.clampScroll();
  }

  private clampScroll(): void {
    const maxTop = Math.max(0, this.layout.lineCount * this.layout.font.lineHeight - Math.max(1, this.height - this.inset));
    this.scrollTop = Math.min(this.scrollTop, maxTop);
    const maxWidth = Math.max(0, ...Array.from({ length: Math.min(this.layout.lineCount, 1024) }, (_, i) => this.layout.shape(i).width));
    this.scrollLeft = Math.min(this.scrollLeft, Math.max(0, maxWidth - this.width + 48));
  }

  private listen(target: EventTarget, name: string, fn: EventListener, options?: AddEventListenerOptions): void {
    target.addEventListener(name, fn, options); this.cleanups.push(() => target.removeEventListener(name, fn, options));
  }
}
