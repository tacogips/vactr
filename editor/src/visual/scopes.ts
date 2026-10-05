// Ring-buffered analyzer displays (design 12.5, G4): `spectrogram`,
// `note-spectrogram` and `oscilloscope` keep their frames in their cells
// with an index in the LAST cell (src/dsp/effects/analyzer.rs):
//
// - the spectrogram kinds (`ring_slot`) store the index of the frame
//   written most recently, so the oldest frame is the one after it;
// - `oscilloscope` (`scope`) stores the next write position, which is the
//   oldest sample.
//
// Decoding returns frames oldest first; an out-of-range index reads as 0.

import { dbFraction, num, SPECTRUM_FLOOR_DB } from './spectrum';
import type { LevelsBody } from '../protocol/types';

export interface TimedLevels { body: LevelsBody; receivedAt: number }

/** Select the newest timestamped level body that is not ahead of audible time. */
export function pickFrame(frames: readonly TimedLevels[], t: number): LevelsBody | null {
  let picked: TimedLevels | null = null;
  for (const frame of frames) {
    const time = frame.body.time;
    if (typeof time === 'number' && Number.isFinite(time) && time <= t && (!picked || time >= (picked.body.time as number))) picked = frame;
  }
  if (!picked || t - (picked.body.time as number) > 2) return null;
  return picked.body;
}

/** What the last cell of a ring holds. */
export type RingIndex = 'newest' | 'next';

export interface RingLayout {
  width: number;
  index: RingIndex;
}

export const SPECTROGRAM_RING: RingLayout = { width: 32, index: 'newest' };
export const NOTE_SPECTROGRAM_RING: RingLayout = { width: 12, index: 'newest' };
export const SCOPE_RING: RingLayout = { width: 1, index: 'next' };

/** The ring's frames, oldest first; the frame count comes from the cell count. */
export function decodeRing(cells: readonly number[], layout: RingLayout): number[][] {
  const frames = Math.floor(Math.max(0, cells.length - 1) / layout.width);
  if (frames === 0) return [];
  let idx = Math.trunc(num(cells[cells.length - 1]));
  if (idx < 0 || idx >= frames) idx = 0;
  const start = layout.index === 'newest' ? (idx + 1) % frames : idx;
  const out: number[][] = [];
  for (let j = 0; j < frames; j++) {
    const f = (start + j) % frames;
    out.push(cells.slice(f * layout.width, (f + 1) * layout.width).map(num));
  }
  return out;
}

/** The oscilloscope's samples, oldest first. */
export function scopeSamples(cells: readonly number[]): number[] {
  return decodeRing(cells, SCOPE_RING).map((f) => f[0] ?? 0);
}

/** A rolling image: time left to right, bins bottom to top. */
export function drawSpectrogram(ctx: CanvasRenderingContext2D, w: number, h: number, frames: number[][]): void {
  const cols = frames.length;
  const rows = frames[0]?.length ?? 0;
  if (cols === 0 || rows === 0) return;
  const cw = w / cols;
  const ch = h / rows;
  frames.forEach((frame, x) => {
    frame.forEach((m, y) => {
      const v = Math.round(dbFraction(m, SPECTRUM_FLOOR_DB) * 255);
      ctx.fillStyle = `rgb(${v}, ${Math.round(v * 0.7)}, ${255 - v})`;
      ctx.fillRect(x * cw, h - (y + 1) * ch, Math.ceil(cw), Math.ceil(ch));
    });
  });
}

/** A polyline over the samples in ring order, -1..1 full height. */
export function drawScope(ctx: CanvasRenderingContext2D, w: number, h: number, samples: readonly number[]): void {
  if (samples.length === 0) return;
  const step = samples.length > 1 ? w / (samples.length - 1) : 0;
  const y = (s: number): number => h / 2 - (Math.max(-1, Math.min(1, s)) * h) / 2;
  ctx.strokeStyle = '#7fe0a0';
  ctx.lineWidth = 1;
  ctx.beginPath();
  samples.forEach((s, i) => {
    if (i === 0) ctx.moveTo(0, y(s));
    else ctx.lineTo(i * step, y(s));
  });
  ctx.stroke();
}
