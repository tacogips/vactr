import { describe, expect, it, vi } from 'vitest';
import { FrameScheduler, PerfRecorder, PhaseTimer, type FrameHost } from '../../src/code/frame';

class TestHost {
  ratio = 1; innerHeight = 800; next = 1;
  get devicePixelRatio(): number { return this.ratio; }
  callbacks = new Map<number, FrameRequestCallback>();
  media: MediaQueryList[] = [];
  resizeCallback: ResizeObserverCallback | null = null;
  resizeInstance: { disconnect: ReturnType<typeof vi.fn> } | null = null;
  box = { width: 320, height: 200 };
  viewport: (EventTarget & { height: number; offsetTop: number }) | null = null;
  requestAnimationFrame(cb: FrameRequestCallback): number { const id = this.next++; this.callbacks.set(id, cb); return id; }
  cancelAnimationFrame(id: number): void { this.callbacks.delete(id); }
  matchMedia(query: string): MediaQueryList {
    const target = new EventTarget(); const add = EventTarget.prototype.addEventListener; const remove = EventTarget.prototype.removeEventListener;
    const media = Object.assign(target, { media: query, matches: true, onchange: null,
      addEventListener: (n: string, cb: EventListener) => add.call(target, n, cb),
      removeEventListener: (n: string, cb: EventListener) => remove.call(target, n, cb),
      addListener: () => {}, removeListener: () => {}, dispatch: () => target.dispatchEvent(new Event('change')),
    }) as unknown as MediaQueryList & { dispatch(): void };
    this.media.push(media); return media;
  }
  ResizeObserver = class {
    constructor(cb: ResizeObserverCallback) { thisHost.resizeCallback = cb; thisHost.resizeInstance = this; }
    observe() {}
    disconnect = vi.fn();
  };
  flush(timestamp = 16): void { const entry = this.callbacks.entries().next().value as [number, FrameRequestCallback] | undefined; if (!entry) return; this.callbacks.delete(entry[0]); entry[1](timestamp); }
  private static current: TestHost;
  static setCurrent(host: TestHost): void { TestHost.current = host; }
  static get currentHost(): TestHost { return TestHost.current; }
}
let thisHost: TestHost;
function setup(hidden = false, onFrame = vi.fn(), onViewport = vi.fn(), perf = false, box = { width: 320, height: 200 }) {
  const host = new TestHost(); thisHost = host; TestHost.setCurrent(host);
  host.box = { ...box };
  const doc = document.implementation.createHTMLDocument('frame-test');
  Object.defineProperty(doc, 'visibilityState', { configurable: true, get: () => hidden ? 'hidden' : 'visible' });
  const element = doc.createElement('div');
  element.getBoundingClientRect = () => ({ ...host.box, x: 0, y: 0, left: 0, top: 0, right: host.box.width, bottom: host.box.height, toJSON: () => ({}) });
  if (host.viewport) Object.defineProperty(host, 'visualViewport', { value: host.viewport });
  const scheduler = new FrameScheduler(host as unknown as FrameHost, doc, element, { onFrame, onViewport, perf });
  return { host, doc, element, scheduler, onFrame, onViewport };
}

describe('FrameScheduler', () => {
  it('runs one initial frame then stays idle without animation owners', () => {
    const f = setup(); expect(f.host.callbacks.size).toBe(1); f.host.flush(123);
    expect(f.onFrame).toHaveBeenCalledWith({ frameMs: 123, textDirty: false, reasons: new Set() }); expect(f.host.callbacks.size).toBe(0);
    f.scheduler.dispose();
  });
  it('delivers the initial unchanged viewport before the first frame, then only on changes', () => {
    const order: string[] = [];
    const f = setup(false, vi.fn(() => order.push('frame')), vi.fn(() => order.push('viewport')), false, { width: 767, height: 858 });
    f.host.flush(17);
    expect(f.onViewport).toHaveBeenCalledExactlyOnceWith({ width: 767, height: 858, dpr: 1, keyboardInset: 0 });
    expect(order.slice(0, 2)).toEqual(['viewport', 'frame']);
    f.scheduler.invalidateText(); f.host.flush(33);
    expect(f.onViewport).toHaveBeenCalledTimes(1);
    f.host.box = { width: 800, height: 600 }; f.host.resizeCallback?.([], {} as ResizeObserver); f.host.flush(49);
    expect(f.onViewport).toHaveBeenCalledTimes(2);
    expect(f.onViewport).toHaveBeenLastCalledWith({ width: 800, height: 600, dpr: 1, keyboardInset: 0 });
    f.scheduler.dispose();
  });
  it('coalesces viewport callback text invalidation into the first frame', () => {
    let f: ReturnType<typeof setup>;
    f = setup(false, vi.fn(), vi.fn(() => f.scheduler.invalidateText()));
    f.host.flush(17);
    expect(f.onFrame).toHaveBeenCalledExactlyOnceWith({ frameMs: 17, textDirty: true, reasons: new Set(['doc']) });
    expect(f.host.callbacks.size).toBe(0);
    f.scheduler.dispose();
  });
  it('keeps exactly one rAF while an owner is active and stops when cleared', () => {
    const f = setup(); f.host.flush(); f.scheduler.setActive('playing', true);
    for (let i = 0; i < 5; i++) f.host.flush(20 + i);
    f.scheduler.setActive('playing', false); expect(f.onFrame).toHaveBeenCalledTimes(6); expect(f.host.callbacks.size).toBe(0); f.scheduler.dispose();
  });
  it('coalesces text invalidations into one dirty frame', () => {
    const f = setup(); f.host.flush(); f.scheduler.invalidateText(); f.scheduler.invalidateText(); f.host.flush(33);
    expect(f.onFrame).toHaveBeenLastCalledWith({ frameMs: 33, textDirty: true, reasons: new Set(['doc']) }); expect(f.host.callbacks.size).toBe(0); f.scheduler.dispose();
  });
  it('defers hidden invalidation until visible and refreshes viewport once', () => {
    const f = setup(true); f.host.box.width = 400; f.scheduler.invalidateText(); expect(f.host.callbacks.size).toBe(0);
    Object.defineProperty(f.doc, 'visibilityState', { configurable: true, get: () => 'visible' }); f.doc.dispatchEvent(new Event('visibilitychange'));
    expect(f.host.callbacks.size).toBe(1); f.host.flush(); expect(f.onFrame).toHaveBeenLastCalledWith({ frameMs: 16, textDirty: true, reasons: new Set(['doc']) });
    expect(f.onViewport).toHaveBeenCalledTimes(1); f.scheduler.dispose();
  });
  it('delivers the unchanged constructor viewport on the first tick after becoming visible', () => {
    const f = setup(true); expect(f.onViewport).not.toHaveBeenCalled();
    Object.defineProperty(f.doc, 'visibilityState', { configurable: true, get: () => 'visible' }); f.doc.dispatchEvent(new Event('visibilitychange'));
    f.host.flush(17);
    expect(f.onViewport).toHaveBeenCalledExactlyOnceWith({ width: 320, height: 200, dpr: 1, keyboardInset: 0 });
    f.scheduler.dispose();
  });
  it('accumulates explicit dirty reasons and defaults to document changes', () => {
    const f = setup(); f.host.flush(); f.scheduler.invalidateText('selection'); f.host.flush(33);
    expect(f.onFrame).toHaveBeenLastCalledWith({ frameMs: 33, textDirty: true, reasons: new Set(['selection']) });
    f.scheduler.invalidateText(); f.host.flush(49);
    expect(f.onFrame).toHaveBeenLastCalledWith({ frameMs: 49, textDirty: true, reasons: new Set(['doc']) });
    f.scheduler.dispose();
  });
  it('re-registers resolution media query when DPR changes', () => {
    const f = setup(); f.host.flush(); f.host.ratio = 2; (f.host.media[0] as MediaQueryList & { dispatch(): void }).dispatch(); f.host.flush();
    expect(f.host.media[1]!.media).toBe('(resolution: 2dppx)'); expect(f.onViewport).toHaveBeenLastCalledWith({ width: 320, height: 200, dpr: 2, keyboardInset: 0 }); f.scheduler.dispose();
  });
  it('coalesces resize and visual viewport changes and calculates keyboard inset', () => {
    const f = setup(); f.host.flush(); f.host.viewport = Object.assign(new EventTarget(), { height: 500, offsetTop: 0 });
    Object.defineProperty(f.host, 'visualViewport', { value: f.host.viewport }); f.host.innerHeight = 800;
    f.host.resizeCallback?.([], {} as ResizeObserver); for (let i = 0; i < 10; i++) f.host.resizeCallback?.([], {} as ResizeObserver);
    f.host.viewport.dispatchEvent(new Event('resize')); f.host.flush();
    expect(f.onViewport).toHaveBeenCalledTimes(2); expect(f.onViewport).toHaveBeenLastCalledWith({ width: 320, height: 200, dpr: 1, keyboardInset: 300 }); f.scheduler.dispose();
  });
  it('keeps a pending viewport update when the last animation owner stops', () => {
    const f = setup(); f.host.flush(); f.scheduler.setActive('playing', true);
    f.host.box.width = 400; f.host.resizeCallback?.([], {} as ResizeObserver);
    f.scheduler.setActive('playing', false);
    expect(f.host.callbacks.size).toBe(1); f.host.flush();
    expect(f.onViewport).toHaveBeenCalledWith({ width: 400, height: 200, dpr: 1, keyboardInset: 0 });
    expect(f.host.callbacks.size).toBe(0); f.scheduler.dispose();
  });
  it('cancels on freeze, resumes without hidden rAF, and disposes observers/listeners', () => {
    const f = setup(); const host = thisHost;
    f.doc.dispatchEvent(new Event('freeze')); expect(f.scheduler.hidden).toBe(true); expect(f.host.callbacks.size).toBe(0);
    f.scheduler.request(); expect(f.host.callbacks.size).toBe(0); f.doc.dispatchEvent(new Event('resume')); expect(f.host.callbacks.size).toBe(1);
    f.scheduler.dispose(); f.scheduler.dispose(); expect(f.host.callbacks.size).toBe(0);
    const callbackCount = f.onFrame.mock.calls.length; f.doc.dispatchEvent(new Event('resume')); f.host.flush(); expect(f.onFrame).toHaveBeenCalledTimes(callbackCount);
    expect(host.resizeInstance?.disconnect).toHaveBeenCalledOnce();
  });
  it('runs afterPresent callbacks in a task after the next frame, with cancellation and hidden handling', () => {
    vi.useFakeTimers();
    try {
      const f = setup(); f.host.flush();
      const calls: string[] = [];
      f.scheduler.afterPresent(() => calls.push('presented'));
      vi.advanceTimersByTime(0); expect(calls).toEqual([]);
      f.host.flush(); expect(calls).toEqual([]);
      vi.advanceTimersByTime(0); expect(calls).toEqual(['presented']);

      const before = f.scheduler.afterPresent(() => calls.push('cancelled-before'));
      before(); f.host.flush(); vi.advanceTimersByTime(0);
      expect(calls).not.toContain('cancelled-before');

      const after = f.scheduler.afterPresent(() => calls.push('cancelled-after'));
      f.host.flush(); after(); vi.advanceTimersByTime(0);
      expect(calls).not.toContain('cancelled-after');

      Object.defineProperty(f.doc, 'visibilityState', { configurable: true, get: () => 'hidden' });
      f.doc.dispatchEvent(new Event('visibilitychange'));
      f.scheduler.afterPresent(() => calls.push('hidden'));
      vi.advanceTimersByTime(0); expect(calls).not.toContain('hidden');
      Object.defineProperty(f.doc, 'visibilityState', { configurable: true, get: () => 'visible' });
      f.doc.dispatchEvent(new Event('visibilitychange')); f.host.flush();
      expect(calls).not.toContain('hidden'); vi.advanceTimersByTime(0);
      expect(calls).toContain('hidden');
      f.scheduler.dispose();
    } finally { vi.useRealTimers(); }
  });
  it('requests an idle frame for afterPresent', () => {
    const f = setup(); f.host.flush(); expect(f.host.callbacks.size).toBe(0);
    f.scheduler.afterPresent(() => {}); expect(f.host.callbacks.size).toBe(1); f.scheduler.dispose();
  });
});

describe('PerfRecorder', () => {
  it('records exclusive nested phase times, recovers after a mismatch and wraps its ring', () => {
    const times = [0, 2, 7, 10]; let nextTime = 11;
    const timer = new PhaseTimer(() => times.shift() ?? nextTime++);
    timer.begin('input'); timer.begin('caret'); timer.end('caret'); timer.end('input');
    const row = timer.rows()[0]!;
    expect(row.slice(0, 4)).toEqual([0, 0, 5, 5]);
    expect(row.slice(4).every((value) => value === 0)).toBe(true);
    expect(() => timer.end('input')).toThrow('Mismatched performance phase end: input');
    for (let i = 0; i < 4097; i++) { timer.begin('frame'); timer.end('frame'); }
    const rows = timer.rows(); expect(rows).toHaveLength(4096);
    expect(rows[0]![1]).toBeGreaterThan(10); expect(rows.at(-1)![1]).toBeGreaterThan(rows[0]![1]!);
  });
  it('retains only the newest 4096 frame and key records', () => {
    const perf = new PerfRecorder(); for (let i = 0; i < 5000; i++) { perf.recordFrame(i, 1, i % 2 === 0, 7); perf.recordKey(i, 7); }
    const snapshot = perf.snapshot(); expect(snapshot.frames).toHaveLength(4096); expect(snapshot.keys).toHaveLength(4096);
    expect(snapshot.frames[0]).toEqual([904, 1, 1, 7]); expect(snapshot.frames.at(-1)).toEqual([4999, 1, 0, 7]);
  });
});
