export interface FrameHost {
  requestAnimationFrame(cb: FrameRequestCallback): number;
  cancelAnimationFrame(id: number): void;
  readonly devicePixelRatio: number;
  readonly innerHeight: number;
  matchMedia?(query: string): MediaQueryList;
  readonly visualViewport?: VisualViewport | null;
  ResizeObserver?: typeof ResizeObserver;
}
export interface ViewportInfo { width: number; height: number; dpr: number; keyboardInset: number }
export type TextDirtyReason = 'doc' | 'selection' | 'view' | 'syntax' | 'annotations' | 'gpu';
export interface FrameContext { frameMs: number; textDirty: boolean; reasons: ReadonlySet<TextDirtyReason> }
export interface PerfSnapshot { frames: number[][]; keys: number[][] }

export type PerfPhase = 'input' | 'caret' | 'shaping' | 'syntax' | 'upload' | 'frame' | 'tick';
export const PERF_PHASES: readonly PerfPhase[] = ['input', 'caret', 'shaping', 'syntax', 'upload', 'frame', 'tick'];
interface PhaseSpan { phase: PerfPhase; startMs: number; childMs: number }

/** Bounded exclusive phase spans. It is created only when performance recording is enabled. */
export class PhaseTimer {
  private readonly ring = Array.from({ length: 4096 }, () => Array<number>(2 + PERF_PHASES.length).fill(0));
  private readonly stack: PhaseSpan[] = [];
  private next = 0;
  private count = 0;
  private startMs = 0;
  private exclusive = PERF_PHASES.map(() => 0);
  constructor(private readonly now: () => number = () => performance.now()) {}
  begin(phase: PerfPhase): void {
    const startMs = this.now();
    if (!this.stack.length) { this.startMs = startMs; this.exclusive = PERF_PHASES.map(() => 0); }
    this.stack.push({ phase, startMs, childMs: 0 });
  }
  end(phase: PerfPhase): void {
    const span = this.stack[this.stack.length - 1];
    if (!span || span.phase !== phase) throw new Error(`Mismatched performance phase end: ${phase}`);
    const endMs = this.now(); const duration = Math.max(0, endMs - span.startMs);
    this.exclusive[PERF_PHASES.indexOf(phase)]! += Math.max(0, duration - span.childMs);
    this.stack.pop();
    const parent = this.stack[this.stack.length - 1];
    if (parent) parent.childMs += duration;
    else {
      const row = this.ring[this.next]!; row[0] = PERF_PHASES.indexOf(phase); row[1] = this.startMs;
      for (let i = 0; i < this.exclusive.length; i++) row[i + 2] = this.exclusive[i]!;
      this.next = (this.next + 1) % this.ring.length; this.count = Math.min(this.ring.length, this.count + 1);
    }
  }
  rows(): number[][] {
    const start = (this.next - this.count + this.ring.length) % this.ring.length;
    return Array.from({ length: this.count }, (_, i) => [...this.ring[(start + i) % this.ring.length]!]);
  }
}

/** Fixed-size telemetry rings. Writes reuse preallocated rows; copies are made only on snapshot. */
export class PerfRecorder {
  private readonly frameRows = Array.from({ length: 4096 }, () => [0, 0, 0, 0]);
  private readonly keyRows = Array.from({ length: 4096 }, () => [0, 0]);
  private frameNext = 0; private frameCount = 0; private keyNext = 0; private keyCount = 0;
  readonly phases = new PhaseTimer();
  recordFrame(frameMs: number, workMs: number, text: boolean, revision: number): void {
    const row = this.frameRows[this.frameNext]!; row[0] = frameMs; row[1] = workMs; row[2] = text ? 1 : 0; row[3] = revision;
    this.frameNext = (this.frameNext + 1) % this.frameRows.length; this.frameCount = Math.min(this.frameRows.length, this.frameCount + 1);
  }
  recordKey(timeStampMs: number, revision: number): void {
    const row = this.keyRows[this.keyNext]!; row[0] = timeStampMs; row[1] = revision;
    this.keyNext = (this.keyNext + 1) % this.keyRows.length; this.keyCount = Math.min(this.keyRows.length, this.keyCount + 1);
  }
  snapshot(): PerfSnapshot { return { frames: this.ordered(this.frameRows, this.frameNext, this.frameCount), keys: this.ordered(this.keyRows, this.keyNext, this.keyCount) }; }
  private ordered(rows: number[][], next: number, count: number): number[][] {
    const start = (next - count + rows.length) % rows.length;
    return Array.from({ length: count }, (_, i) => [...rows[(start + i) % rows.length]!]);
  }
}

export class FrameScheduler {
  private pending: number | null = null;
  private hiddenValue: boolean;
  private disposed = false;
  private inFrame = false;
  private deliveringViewport = false;
  private requestedDuringFrame = false;
  private dirtyReasons = new Set<TextDirtyReason>();
  private viewportDirty = true;
  private viewportDelivered = false;
  private viewportValue: ViewportInfo;
  private owners = new Set<string>();
  private cleanups: (() => void)[] = [];
  private observer: ResizeObserver | null = null;
  private media: MediaQueryList | null = null;
  private afterPresentNext = 0;
  private afterPresentCallbacks = new Map<number, () => void>();
  private afterPresentTimer: ReturnType<typeof setTimeout> | null = null;
  private readonly tick = (timestamp: number): void => {
    this.pending = null;
    if (this.disposed || this.hiddenValue) return;
    this.inFrame = true; this.requestedDuringFrame = false;
    if (this.viewportDirty) {
      this.viewportDirty = false;
      const next = this.readViewport();
      if (!this.viewportDelivered || !this.sameViewport(next, this.viewportValue)) {
        this.viewportValue = next; this.viewportDelivered = true; this.deliveringViewport = true;
        try { this.opts.onViewport(next); } finally { this.deliveringViewport = false; }
      }
    }
    const reasons = this.dirtyReasons; this.dirtyReasons = new Set<TextDirtyReason>();
    const textDirty = reasons.size > 0;
    const start = this.perf ? performance.now() : 0;
    this.perf?.phases.begin('frame');
    try { this.opts.onFrame({ frameMs: timestamp, textDirty, reasons }); this.stats.frames++; if (textDirty) this.stats.textFrames++; }
    finally {
      this.perf?.phases.end('frame');
      const work = this.perf ? performance.now() - start : 0;
      this.perf?.recordFrame(timestamp, work, textDirty, this.opts.revision?.() ?? 0);
      this.inFrame = false;
      this.postAfterPresent();
    }
    if (this.requestedDuringFrame || this.owners.size > 0) this.schedule();
  };
  readonly perf: PerfRecorder | null;
  readonly stats = { frames: 0, textFrames: 0 };
  constructor(private host: FrameHost, private doc: Document, private element: HTMLElement,
    private opts: { onFrame(ctx: FrameContext): void; onViewport(v: ViewportInfo): void; perf?: boolean; revision?: () => number }) {
    this.perf = opts.perf ? new PerfRecorder() : null;
    this.viewportValue = this.readViewport(); this.hiddenValue = doc.visibilityState === 'hidden';
    this.listen(doc, 'visibilitychange', () => this.visibilityChanged());
    this.listen(doc, 'freeze', () => this.setHidden(true));
    this.listen(doc, 'resume', () => this.resume());
    const Resize = host.ResizeObserver;
    if (Resize) { this.observer = new Resize(() => this.markViewportDirty()); this.observer.observe(element); }
    const vv = host.visualViewport;
    if (vv) { this.listen(vv, 'resize', () => this.markViewportDirty()); this.listen(vv, 'scroll', () => this.markViewportDirty()); }
    this.watchDpr();
    if (!this.hiddenValue) this.request();
  }
  get hidden(): boolean { return this.hiddenValue; }
  invalidateText(reason: TextDirtyReason = 'doc'): void { this.dirtyReasons.add(reason); if (!this.deliveringViewport) this.request(); }
  setActive(owner: string, active: boolean): void {
    if (active) this.owners.add(owner); else this.owners.delete(owner);
    if (active) this.request();
    else if (this.owners.size === 0 && this.dirtyReasons.size === 0 && !this.viewportDirty && this.pending !== null) { this.host.cancelAnimationFrame(this.pending); this.pending = null; }
  }
  request(): void {
    if (this.disposed || this.hiddenValue) return;
    if (this.inFrame) { this.requestedDuringFrame = true; return; }
    this.schedule();
  }
  afterPresent(cb: () => void): () => void {
    if (this.disposed) return () => {};
    const id = this.afterPresentNext++;
    this.afterPresentCallbacks.set(id, cb);
    this.request();
    return () => { this.afterPresentCallbacks.delete(id); };
  }
  private postAfterPresent(): void {
    if (!this.afterPresentCallbacks.size || this.afterPresentTimer !== null) return;
    const callbackIds = [...this.afterPresentCallbacks.keys()];
    this.afterPresentTimer = setTimeout(() => {
      this.afterPresentTimer = null;
      for (const id of callbackIds) {
        const cb = this.afterPresentCallbacks.get(id);
        this.afterPresentCallbacks.delete(id);
        if (!this.disposed) cb?.();
      }
      if (this.afterPresentCallbacks.size) this.request();
    }, 0);
  }
  private schedule(): void {
    if (this.pending === null && !this.disposed && !this.hiddenValue) this.pending = this.host.requestAnimationFrame(this.tick);
  }
  private visibilityChanged(): void { if (this.doc.visibilityState === 'hidden') this.setHidden(true); else this.resume(); }
  private setHidden(hidden: boolean): void {
    this.hiddenValue = hidden;
    if (hidden && this.pending !== null) { this.host.cancelAnimationFrame(this.pending); this.pending = null; }
    if (!hidden) this.resume();
  }
  private resume(): void {
    if (this.disposed) return;
    this.hiddenValue = false; this.viewportDirty = true; this.invalidateText();
  }
  private markViewportDirty(): void { this.viewportDirty = true; this.request(); }
  private readViewport(): ViewportInfo {
    const rect = this.element.getBoundingClientRect(); const vv = this.host.visualViewport;
    return { width: rect.width, height: rect.height, dpr: this.host.devicePixelRatio,
      keyboardInset: vv ? Math.max(0, this.host.innerHeight - (vv.height + vv.offsetTop)) : 0 };
  }
  private sameViewport(a: ViewportInfo, b: ViewportInfo): boolean { return a.width === b.width && a.height === b.height && a.dpr === b.dpr && a.keyboardInset === b.keyboardInset; }
  private watchDpr(): void {
    if (!this.host.matchMedia) return;
    this.media?.removeEventListener('change', this.dprChanged);
    this.media = this.host.matchMedia(`(resolution: ${this.host.devicePixelRatio}dppx)`);
    const media = this.media; media.addEventListener('change', this.dprChanged);
    this.cleanups.push(() => media.removeEventListener('change', this.dprChanged));
  }
  private readonly dprChanged = (): void => { this.watchDpr(); this.markViewportDirty(); this.invalidateText('gpu'); };
  private listen(target: EventTarget, name: string, callback: EventListener): void {
    target.addEventListener(name, callback); this.cleanups.push(() => target.removeEventListener(name, callback));
  }
  dispose(): void {
    if (this.disposed) return; this.disposed = true;
    if (this.pending !== null) this.host.cancelAnimationFrame(this.pending); this.pending = null;
    if (this.afterPresentTimer !== null) clearTimeout(this.afterPresentTimer); this.afterPresentTimer = null;
    this.afterPresentCallbacks.clear();
    for (const cleanup of this.cleanups.splice(0)) cleanup(); this.observer?.disconnect(); this.observer = null; this.owners.clear();
  }
}
