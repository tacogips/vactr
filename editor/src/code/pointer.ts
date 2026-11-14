import type { CodeRect } from '../app/apis';
import { CodeSurface } from './surface';
import { boundary } from './accessibility';
import { wordRange } from './keyboard';

export interface SelectionHandle { pos: number; end: boolean }
export interface NumericGesture { move(event: PointerEvent): void; end(cancelled: boolean): void }
export interface PointerOptions {
  focus?: () => void;
  scrollBy?: (x: number, y: number) => void;
  onHandles?: (handles: readonly SelectionHandle[]) => void;
  /** Return a gesture only after checking the current semantic numeric site. */
  numericDrag?: (event: PointerEvent, position: number) => NumericGesture | null;
  composing?: () => boolean;
}
interface Gesture {
  id: number; touch: boolean; startX: number; startY: number; x: number; y: number;
  position: number; anchor: number; selecting: boolean; scrolling: boolean;
  numeric?: NumericGesture; handle?: 'anchor' | 'head';
}
/** GPU selection geometry; native touch pan remains available until a long press/handle drag. */
export class PointerController {
  private gesture: Gesture | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private frame: number | null = null;
  private lastPress = { time: -Infinity, x: 0, y: 0, count: 0 };
  private stop: () => void;
  private listeners: (() => void)[] = [];
  constructor(private surface: CodeSurface, private element: HTMLElement, private options: PointerOptions = {}) {
    this.listen(element, 'pointerdown', (event) => this.down(event as PointerEvent));
    this.listen(element, 'pointermove', (event) => this.move(event as PointerEvent));
    this.listen(element, 'pointerup', (event) => this.up(event as PointerEvent));
    this.listen(element, 'pointercancel', () => this.cancel());
    this.listen(element, 'lostpointercapture', () => this.cancel());
    this.listen(window, 'blur', () => this.cancel());
    this.listen(window, 'resize', () => this.handles());
    this.listen(window, 'orientationchange', () => this.handles());
    this.stop = surface.subscribe(() => this.handles());
    this.handles();
  }
  private listen(target: EventTarget, name: string, listener: EventListener): void {
    target.addEventListener(name, listener); this.listeners.push(() => target.removeEventListener(name, listener));
  }
  private handles(): void {
    const s = this.surface.state.selection.main;
    this.options.onHandles?.(s.empty ? [] : [{ pos: s.anchor, end: false }, { pos: s.head, end: true }]);
  }
  private hitHandle(x: number, y: number): 'anchor' | 'head' | undefined {
    const s = this.surface.state.selection.main;
    if (s.empty) return;
    for (const end of ['head', 'anchor'] as const) {
      const r = this.surface.coordsAtPos(s[end]);
      if (r && Math.abs(x - r.left) <= 14 && Math.abs(y - (end === 'head' ? r.bottom : r.top)) <= 14) return end;
    }
  }
  private capture(id: number): void { try { this.element.setPointerCapture(id); } catch { /* unavailable after platform cancellation */ } }
  private down(event: PointerEvent): void {
    if (event.button !== 0 || event.isPrimary === false || this.surface.compositionRange || this.options.composing?.()) return;
    this.cancel();
    const position = this.surface.posAtCoords({ x: event.clientX, y: event.clientY });
    if (position == null) return;
    const touch = event.pointerType === 'touch', s = this.surface.state.selection.main;
    const g: Gesture = { id: event.pointerId, touch, startX: event.clientX, startY: event.clientY, x: event.clientX, y: event.clientY,
      position, anchor: event.shiftKey ? s.anchor : position, selecting: !touch, scrolling: false };
    this.gesture = g;
    let clicks = 1;
    if (event.pointerType === 'mouse') {
      const last = this.lastPress, now = Date.now();
      clicks = now - last.time <= 400 && Math.hypot(g.x - last.x, g.y - last.y) <= 5 ? last.count % 3 + 1 : 1;
      // PointerEvent.detail is usually zero; MouseEvent-derived test/platform values remain compatible.
      if (event.detail > 0) clicks = event.detail;
      this.lastPress = { time: now, x: g.x, y: g.y, count: clicks };
    }
    if (!touch && event.pointerType === 'mouse' && !event.shiftKey && !event.ctrlKey && !event.metaKey && !event.altKey && clicks === 1) {
      g.numeric = this.options.numericDrag?.(event, position) ?? undefined;
      if (g.numeric) { event.preventDefault(); this.capture(g.id); return; }
    }
    if (touch) {
      g.handle = this.hitHandle(event.clientX, event.clientY);
      if (g.handle) { g.selecting = true; this.capture(g.id); event.preventDefault(); return; }
      this.timer = setTimeout(() => {
        this.timer = null;
        if (this.gesture !== g || g.scrolling) return;
        const range = wordRange(this.surface.state.doc.toString(), g.position);
        g.anchor = range.from; g.selecting = true;
        this.surface.dispatch({ selection: { anchor: range.from, head: range.to } }); this.capture(g.id); this.options.focus?.();
      }, 500);
      return;
    }
    event.preventDefault(); this.capture(g.id); this.options.focus?.();
    if (clicks >= 3) {
      const line = this.surface.state.doc.lineAt(position);
      this.surface.dispatch({ selection: { anchor: line.from, head: Math.min(this.surface.state.doc.length, line.to + 1) } }); g.anchor = line.from;
    } else if (clicks === 2) {
      const range = wordRange(this.surface.state.doc.toString(), position);
      this.surface.dispatch({ selection: { anchor: range.from, head: range.to } }); g.anchor = range.from;
    } else this.surface.dispatch({ selection: { anchor: g.anchor, head: position } });
  }
  private move(event: PointerEvent): void {
    const g = this.gesture; if (!g || g.id !== event.pointerId) return;
    g.x = event.clientX; g.y = event.clientY;
    if (g.numeric) { event.preventDefault(); g.numeric.move(event); return; }
    if (g.touch && !g.selecting) {
      if (Math.hypot(g.x - g.startX, g.y - g.startY) > 8) { g.scrolling = true; this.clearTimer(); }
      return;
    }
    event.preventDefault(); this.select(g); this.autoscroll();
  }
  private select(g: Gesture): void {
    const pos = this.surface.posAtCoords({ x: g.x, y: g.y }); if (pos == null) return;
    const head = boundary(this.surface.state.doc.toString(), pos);
    const s = this.surface.state.selection.main;
    this.surface.dispatch({ selection: g.handle === 'anchor' ? { anchor: head, head: s.head }
      : { anchor: g.handle === 'head' ? s.anchor : g.anchor, head } });
  }
  private overflow(g: Gesture, r: CodeRect): { x: number; y: number } {
    return { x: g.x < r.left ? -12 : g.x > r.right ? 12 : 0, y: g.y < r.top ? -12 : g.y > r.bottom ? 12 : 0 };
  }
  private autoscroll(): void {
    if (this.frame != null || !this.options.scrollBy) return;
    const g = this.gesture; if (!g || !g.selecting || g.numeric) return;
    const delta = this.overflow(g, this.element.getBoundingClientRect());
    if (!delta.x && !delta.y) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = null;
      if (this.gesture !== g) return;
      const d = this.overflow(g, this.element.getBoundingClientRect());
      this.options.scrollBy?.(d.x, d.y); this.select(g); this.autoscroll();
    });
  }
  private up(event: PointerEvent): void {
    const g = this.gesture; if (!g || g.id !== event.pointerId) return;
    if (g.touch && !g.selecting && !g.scrolling) {
      this.surface.dispatch({ selection: { anchor: g.position } }); this.options.focus?.();
    }
    this.release(false);
  }
  private clearTimer(): void { if (this.timer != null) clearTimeout(this.timer); this.timer = null; }
  private release(cancelled: boolean): void {
    const g = this.gesture; this.gesture = null; this.clearTimer();
    if (this.frame != null) cancelAnimationFrame(this.frame); this.frame = null;
    if (g) { g.numeric?.end(cancelled); try { if (this.element.hasPointerCapture(g.id)) this.element.releasePointerCapture(g.id); } catch { /* platform already released */ } }
  }
  cancel(): void { this.release(true); }
  dispose(): void { this.cancel(); this.stop(); for (const remove of this.listeners) remove(); this.listeners = []; this.options.onHandles?.([]); }
}
