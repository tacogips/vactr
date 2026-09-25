// A fake canvas 2D context recording its drawing calls, and
// `installCanvasFakes`, which makes `HTMLCanvasElement.getContext` (not
// implemented by jsdom) return one fake 2D context per canvas and, when
// given, a WebGL2 fake. `restore()` puts the jsdom method back.

import { vi } from 'vitest';

export interface Ctx2DCall {
  fn: string;
  args: unknown[];
}

export class FakeContext2D {
  readonly calls: Ctx2DCall[] = [];
  fillStyle: unknown = '#000';
  strokeStyle: unknown = '#000';
  font = '10px sans-serif';
  textAlign = 'start';
  textBaseline = 'alphabetic';
  lineWidth = 1;

  constructor(readonly canvas: HTMLCanvasElement | null = null) {}

  named(fn: string): Ctx2DCall[] {
    return this.calls.filter((c) => c.fn === fn);
  }

  reset(): void {
    this.calls.length = 0;
  }

  as(): CanvasRenderingContext2D {
    return this as unknown as CanvasRenderingContext2D;
  }

  private rec(fn: string, args: unknown[]): void {
    this.calls.push({ fn, args });
  }

  clearRect(...args: number[]): void {
    this.rec('clearRect', args);
  }

  fillRect(...args: number[]): void {
    this.rec('fillRect', [...args, this.fillStyle]);
  }

  fillText(...args: unknown[]): void {
    this.rec('fillText', [...args, this.font, this.fillStyle]);
  }

  beginPath(): void {
    this.rec('beginPath', []);
  }

  moveTo(x: number, y: number): void {
    this.rec('moveTo', [x, y]);
  }

  lineTo(x: number, y: number): void {
    this.rec('lineTo', [x, y]);
  }

  stroke(): void {
    this.rec('stroke', []);
  }

  drawImage(...args: unknown[]): void {
    this.rec('drawImage', args);
  }
}

export interface CanvasFakes {
  /** The fake 2D context of `canvas` (created on first use). */
  ctx(canvas: HTMLCanvasElement): FakeContext2D;
  /** The canvases that asked for a 2D context, in order. */
  canvases2d: HTMLCanvasElement[];
  restore(): void;
}

export function installCanvasFakes(
  opts: { webgl2?: () => unknown; no2d?: boolean } = {},
): CanvasFakes {
  const ctxs = new Map<HTMLCanvasElement, FakeContext2D>();
  const canvases2d: HTMLCanvasElement[] = [];
  const ctx = (canvas: HTMLCanvasElement): FakeContext2D => {
    let c = ctxs.get(canvas);
    if (!c) {
      c = new FakeContext2D(canvas);
      ctxs.set(canvas, c);
    }
    return c;
  };
  const impl = function (this: HTMLCanvasElement, kind: string): unknown {
    if (kind === '2d') {
      if (opts.no2d) return null;
      if (!ctxs.has(this)) canvases2d.push(this);
      return ctx(this);
    }
    if (kind === 'webgl2') return opts.webgl2 ? opts.webgl2() : null;
    return null;
  };
  const spy = vi
    .spyOn(HTMLCanvasElement.prototype, 'getContext')
    .mockImplementation(impl as unknown as HTMLCanvasElement['getContext']);
  return { ctx, canvases2d, restore: () => spy.mockRestore() };
}
