// Spectrum bars and the shared dB helpers of the analyzer displays (design
// 12.5, 15.1.8 "meters and scopes come from `levels`", G4). The data is
// the published analyzer cells and the master `bands`; nothing here taps
// audio. `mountSpectrum` backs `VisualApi.mountSpectrum` (the ED-PARAMS EQ
// editor's live spectrum): it prefers the `spectrum` analyzer of the
// requested bus and falls back to the master bands.

import type { Store } from '../protocol/store';
import type { LevelsBody, WireLevel } from '../protocol/types';

/** The level meter's floor (dBFS). */
export const METER_FLOOR_DB = -60;
/** The spectrum bars' floor (dB of linear magnitude). */
export const SPECTRUM_FLOOR_DB = -80;

export const SPECTRUM_WIDTH = 160;
export const SPECTRUM_HEIGHT = 48;

/** A finite number, or 0 (a non-finite f32 arrives as JSON null). */
export function num(v: unknown): number {
  return typeof v === 'number' && Number.isFinite(v) ? v : 0;
}

export function toDb(linear: number): number {
  return 20 * Math.log10(Math.max(Math.abs(num(linear)), 1e-9));
}

/** `linear` on a `floor`..0 dB scale, as 0..1. */
export function dbFraction(linear: number, floor: number): number {
  const f = (toDb(linear) - floor) / -floor;
  return Math.min(1, Math.max(0, f));
}

export function sameCells(a: readonly unknown[] | undefined, b: readonly unknown[] | undefined): boolean {
  if (a === b) return true;
  if (!a || !b || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

/** Bus names compare without the leading `:` (`:master` = `master`). */
export function sameBus(a: string, b: string): boolean {
  const strip = (s: string): string => (s.startsWith(':') ? s.slice(1) : s);
  return strip(a) === strip(b);
}

/** The `:master` entry of `levels`, else the first. */
export function masterLevel(body: LevelsBody | null): WireLevel | undefined {
  if (!body) return undefined;
  return body.levels.find((l) => sameBus(l.source, 'master')) ?? body.levels[0];
}

/** One bar per magnitude on a dB scale. */
export function drawBars(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  mags: readonly number[],
  floor = SPECTRUM_FLOOR_DB,
): void {
  if (mags.length === 0) return;
  const bw = w / mags.length;
  ctx.fillStyle = '#4fb3ff';
  mags.forEach((m, i) => {
    const bh = dbFraction(m, floor) * h;
    ctx.fillRect(i * bw + 0.5, h - bh, Math.max(1, bw - 1), bh);
  });
}

export interface SpectrumSource {
  from: 'analyzer' | 'master' | 'none';
  mags: number[];
}

/** The `spectrum` analyzer of `bus` when one is published, else the master bands. */
export function spectrumSource(body: LevelsBody | null, bus?: string): SpectrumSource {
  if (bus !== undefined) {
    const a = body?.analyzers?.find((x) => x.kind === 'spectrum' && sameBus(x.bus, bus));
    if (a) return { from: 'analyzer', mags: a.cells.map(num) };
  }
  const bands = masterLevel(body)?.bands;
  if (bands) return { from: 'master', mags: bands.map(num) };
  return { from: 'none', mags: [] };
}

/** A live spectrum in `el` from the store's `levels`; repaints only on change. */
export function mountSpectrum(el: HTMLElement, store: Store, source: { bus?: string }): { dispose(): void } {
  const doc = el.ownerDocument;
  const canvas = doc.createElement('canvas');
  canvas.className = 'visual-spectrum';
  canvas.width = SPECTRUM_WIDTH;
  canvas.height = SPECTRUM_HEIGHT;
  el.appendChild(canvas);
  const ctx = canvas.getContext('2d');
  let last: SpectrumSource | null = null;
  const paint = (): void => {
    const next = spectrumSource(store.levels, source.bus);
    if (last && last.from === next.from && sameCells(last.mags, next.mags)) return;
    last = next;
    canvas.dataset.source = next.from;
    if (!ctx) return;
    ctx.clearRect(0, 0, SPECTRUM_WIDTH, SPECTRUM_HEIGHT);
    drawBars(ctx, SPECTRUM_WIDTH, SPECTRUM_HEIGHT, next.mags);
  };
  const off = store.subscribe(['levels'], paint);
  paint();
  return {
    dispose(): void {
      off();
      canvas.remove();
    },
  };
}
