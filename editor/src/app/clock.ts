// The editor's time source (design 15.1.4 "highlight timing"). The browser
// tier reads `AudioContext.currentTime`, the session's own clock; the
// native tier anchors host times at batch receipt on the page clock.

export interface Clock {
  /** Seconds. */
  now(): number;
}

export class AudioClock implements Clock {
  private readonly ctx: { readonly currentTime: number };

  constructor(ctx: { readonly currentTime: number }) {
    this.ctx = ctx;
  }

  now(): number {
    return this.ctx.currentTime;
  }
}

/** `performance.now()` in seconds (the native tier). */
export class PageClock implements Clock {
  now(): number {
    return performance.now() / 1000;
  }
}
