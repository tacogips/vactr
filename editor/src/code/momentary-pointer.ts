import type { CodeSurface, MomentaryGesture } from '../app/apis';
import { CONTEXT_WINDOW_MS, TWO_FINGER_WINDOW_MS } from '../bind/momentary';

interface PendingTouch { id: number; startX: number; startY: number; x: number; y: number; time: number; pos: number }
interface Pair { ids: [number, number]; gesture: MomentaryGesture; points: Map<number, { x: number; y: number }> }
export interface MomentaryPointerOptions { cancelSelection(): void; now?: () => number }

/** Independent right-mouse and touch-pair state; selection remains owned by PointerController. */
export class MomentaryPointer {
  private mouse = new Map<number, MomentaryGesture>();
  private pending = new Map<number, PendingTouch>();
  private pairs = new Map<number, Pair>();
  private pairByPointer = new Map<number, Pair>();
  private lastMouseEnd = -Infinity;
  private listeners: (() => void)[] = [];
  private readonly now: () => number;
  constructor(private readonly surface: CodeSurface, private readonly element: HTMLElement, private readonly options: MomentaryPointerOptions) {
    this.now = options.now ?? (() => Date.now());
    this.listen('pointerdown', (e) => this.down(e as PointerEvent));
    this.listen('pointermove', (e) => this.move(e as PointerEvent));
    this.listen('pointerup', (e) => this.up(e as PointerEvent));
    this.listen('pointercancel', (e) => this.cancelPointer((e as PointerEvent).pointerId));
    this.listen('lostpointercapture', (e) => this.cancelPointer((e as PointerEvent).pointerId));
    this.listen('contextmenu', (e) => this.context(e as MouseEvent));
    window.addEventListener('blur', this.blur); this.listeners.push(() => window.removeEventListener('blur', this.blur));
  }
  private listen(name: string, fn: (event: Event) => void): void { this.element.addEventListener(name, fn); this.listeners.push(() => this.element.removeEventListener(name, fn)); }
  private capture(id: number): void { try { this.element.setPointerCapture(id); } catch { /* platform cancellation */ } }
  private down(e: PointerEvent): void {
    if (e.pointerType === 'mouse' && e.button === 2) {
      if (e.buttons & 1) return; // D11: a right press chorded into a left drag starts nothing.
      const pos = this.surface.posAtCoords({ x: e.clientX, y: e.clientY }); if (pos == null) return;
      const g = this.surface.momentaryDrag({ pos, clientY: e.clientY, kind: 'mouse' }); if (!g) return;
      (e as PointerEvent & { vactrNumericGesture?: boolean }).vactrNumericGesture = true;
      this.mouse.set(e.pointerId, g); this.capture(e.pointerId); e.preventDefault(); return;
    }
    if (e.pointerType !== 'touch') return;
    const pos = this.surface.posAtCoords({ x: e.clientX, y: e.clientY }); if (pos == null) return;
    const pending: PendingTouch = { id: e.pointerId, startX: e.clientX, startY: e.clientY, x: e.clientX, y: e.clientY, time: this.now(), pos };
    const candidate = [...this.pending.values()].filter((p) => this.now() - p.time <= TWO_FINGER_WINDOW_MS && this.surface.momentaryHit(p.pos))
      .sort((a, b) => Math.hypot(a.x - e.clientX, a.y - e.clientY) - Math.hypot(b.x - e.clientX, b.y - e.clientY))[0];
    if (!candidate) { this.pending.set(e.pointerId, pending); return; }
    this.pending.delete(candidate.id);
    const gesture = this.surface.momentaryDrag({ pos: candidate.pos, clientY: (candidate.y + e.clientY) / 2, kind: 'touch' });
    if (!gesture) return;
    (e as PointerEvent & { vactrNumericGesture?: boolean }).vactrNumericGesture = true;
    this.options.cancelSelection();
    const pair: Pair = { ids: [candidate.id, e.pointerId], gesture, points: new Map([[candidate.id, { x: candidate.x, y: candidate.y }], [e.pointerId, { x: e.clientX, y: e.clientY }]]) };
    this.pairs.set(candidate.id, pair); this.pairByPointer.set(candidate.id, pair); this.pairByPointer.set(e.pointerId, pair);
    this.capture(candidate.id); this.capture(e.pointerId); e.preventDefault();
  }
  private move(e: PointerEvent): void {
    const mouse = this.mouse.get(e.pointerId); if (mouse) { mouse.move(e.clientY); e.preventDefault(); return; }
    const pair = this.pairByPointer.get(e.pointerId);
    if (pair) { pair.points.set(e.pointerId, { x: e.clientX, y: e.clientY }); pair.gesture.move([...pair.points.values()].reduce((sum, p) => sum + p.y, 0) / 2); e.preventDefault(); return; }
    const p = this.pending.get(e.pointerId);
    if (p) { if (Math.hypot(p.startX - e.clientX, p.startY - e.clientY) >= 8) this.pending.delete(e.pointerId); else { p.x = e.clientX; p.y = e.clientY; } }
  }
  private up(e: PointerEvent): void {
    const mouse = this.mouse.get(e.pointerId); if (mouse) { this.mouse.delete(e.pointerId); mouse.end(e.shiftKey); this.lastMouseEnd = this.now(); return; }
    const pair = this.pairByPointer.get(e.pointerId); if (pair) { this.endPair(pair, e.shiftKey); return; }
    this.pending.delete(e.pointerId);
  }
  private endPair(pair: Pair, snap: boolean): void {
    pair.gesture.end(snap); this.pairs.delete(pair.ids[0]); for (const id of pair.ids) this.pairByPointer.delete(id);
  }
  private cancelPointer(id: number): void {
    const mouse = this.mouse.get(id); if (mouse) { this.mouse.delete(id); mouse.end(false); this.lastMouseEnd = this.now(); return; }
    const pair = this.pairByPointer.get(id); if (pair) this.endPair(pair, false);
    this.pending.delete(id);
  }
  private context(e: MouseEvent): void {
    const pos = this.surface.posAtCoords({ x: e.clientX, y: e.clientY });
    if ((pos !== null && this.surface.momentaryHit(pos)) || this.mouse.size > 0 || this.now() - this.lastMouseEnd <= CONTEXT_WINDOW_MS) e.preventDefault();
  }
  private blur = (): void => { for (const id of [...this.mouse.keys()]) this.cancelPointer(id); for (const pair of [...this.pairs.values()]) this.endPair(pair, false); this.pending.clear(); };
  dispose(): void { this.blur(); for (const remove of this.listeners) remove(); this.listeners = []; }
}
