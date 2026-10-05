// The visual frame loop (design 9.3, 15.1.8; browser tier only). Once per
// animation frame: `core.frame(now)` (`session_frame`) resolves the active
// uniform plans, whose `uniforms` records reach the host synchronously,
// then `host.draw(now)` renders, then `onFrame` presents the panes. Both
// calls read the same audio-clock time. The loop pauses while the page or
// pane is hidden and stops for good on dispose or on a thrown frame.

import { uncorrelated, type AudibleClock, type Clock } from '../app/clock';

export interface FrameScheduler {
  request(cb: (frameMs: number) => void): number;
  cancel(id: number): void;
}

export interface FrameLoopOptions {
  core: { frame(now: number): void };
  host: { draw(timeSec: number): void };
  clock: Clock;
  audible?: AudibleClock;
  scheduler: FrameScheduler;
  /** Called after each draw (pane presentation). */
  onFrame?: (timeSec: number) => void;
  /** A thrown frame stops the loop and is reported here. */
  onError?: (e: unknown) => void;
}

export interface FrameLoop {
  /** Pauses (hidden) or resumes (visible) the loop. */
  setVisible(visible: boolean): void;
  readonly running: boolean;
  dispose(): void;
}

/** `requestAnimationFrame` of `win`, or a 16 ms timer where there is none. */
export function windowScheduler(win: Window | null): FrameScheduler {
  if (win && typeof win.requestAnimationFrame === 'function') {
    return {
      request: (cb) => win.requestAnimationFrame((frameMs) => cb(frameMs)),
      cancel: (id) => win.cancelAnimationFrame(id),
    };
  }
  return {
    request: (cb) => setTimeout(() => cb(performance.now()), 16) as unknown as number,
    cancel: (id) => clearTimeout(id),
  };
}

export function startFrameLoop(opts: FrameLoopOptions): FrameLoop {
  const audible = opts.audible ?? uncorrelated(opts.clock);
  let pending: number | null = null;
  let visible = true;
  let stopped = false;

  const schedule = (): void => {
    if (stopped || !visible || pending !== null) return;
    pending = opts.scheduler.request(tick);
  };

  const tick = (frameMs: number): void => {
    pending = null;
    if (stopped || !visible) return;
    try {
      const sample = audible.sample(frameMs);
      const t = sample.valid ? sample.time : opts.clock.now();
      opts.core.frame(t);
      opts.host.draw(t);
      opts.onFrame?.(t);
    } catch (e) {
      stopped = true;
      opts.onError?.(e);
      return;
    }
    schedule();
  };

  const cancel = (): void => {
    if (pending !== null) opts.scheduler.cancel(pending);
    pending = null;
  };

  schedule();
  return {
    setVisible(v: boolean): void {
      visible = v;
      if (v) schedule();
      else cancel();
    },
    get running(): boolean {
      return pending !== null;
    },
    dispose(): void {
      stopped = true;
      cancel();
    },
  };
}
