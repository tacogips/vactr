// Parameter curves and the curve math of the parameter editors (design
// 15.1.7, 13.5 ParamMeta). A handle position is a 0..1 unit on the
// parameter's range; `log` maps it geometrically, `stepped` rounds to an
// integer. The response/transfer/shape functions below are display
// approximations only: the audio is whatever the session computes.

import type { ParamCurve } from '../protocol/types';

export interface CurveSpec {
  range: [number, number];
  curve: ParamCurve;
}

export const clamp = (v: number, lo: number, hi: number): number => Math.min(hi, Math.max(lo, v));

/** Rounds to at most 6 decimals, never producing `-0`. */
export const round6 = (v: number): number => Number(v.toFixed(6)) + 0;

const isLog = (s: CurveSpec): boolean => s.curve === 'log' && s.range[0] > 0 && s.range[1] > 0;

/** A value -> its 0..1 position on the range and curve. */
export function toUnit(value: number, s: CurveSpec): number {
  const [lo, hi] = s.range;
  if (hi === lo) return 0;
  const v = clamp(value, Math.min(lo, hi), Math.max(lo, hi));
  if (isLog(s)) return Math.log(v / lo) / Math.log(hi / lo);
  return (v - lo) / (hi - lo);
}

/** A 0..1 position -> a value on the range and curve (quantized). */
export function fromUnit(t: number, s: CurveSpec): number {
  const [lo, hi] = s.range;
  const u = clamp(t, 0, 1);
  const v = isLog(s) ? lo * (hi / lo) ** u : lo + u * (hi - lo);
  return quantize(v, s);
}

/** The value a handle writes: an integer on a stepped curve, else 6 decimals. */
export function quantize(v: number, s: Pick<CurveSpec, 'curve'>): number {
  return s.curve === 'stepped' ? Math.round(v) + 0 : round6(v);
}

/** `v` wrapped into `0..n-1` (euclid rotation). */
export function wrap(v: number, n: number): number {
  if (n <= 0) return 0;
  return (((Math.round(v) % n) + n) % n) + 0;
}

// ------------------------------------------------------ frequency axis

export const F_MIN = 20;
export const F_MAX = 20000;

/** A frequency -> 0..1 on the log axis. */
export const freqX = (f: number): number => Math.log(clamp(f, F_MIN, F_MAX) / F_MIN) / Math.log(F_MAX / F_MIN);

/** 0..1 on the log axis -> a frequency. */
export const xFreq = (x: number): number => F_MIN * (F_MAX / F_MIN) ** clamp(x, 0, 1);

/** A peaking band's gain (dB) at `f`. */
export function bellDb(f: number, f0: number, gainDb: number, q: number): number {
  const oct = Math.log2(Math.max(f, 1e-6) / Math.max(f0, 1e-6));
  const w = oct * Math.max(q, 0.05) * 2;
  return gainDb / (1 + w * w);
}

/** A shelf's gain (dB) at `f` (`high` = high shelf). */
export function shelfDb(f: number, f0: number, gainDb: number, high: boolean): number {
  const oct = Math.log2(Math.max(f, 1e-6) / Math.max(f0, 1e-6));
  const s = 1 / (1 + Math.exp(-oct * 3));
  return gainDb * (high ? s : 1 - s);
}

export type FilterShape = 'lowpass' | 'highpass' | 'bandpass' | 'notch';

/** The filter shape named by a builtin. */
export function filterShape(name: string): FilterShape {
  if (/hpf|highpass/.test(name)) return 'highpass';
  if (/bpf|bandpass|narrow/.test(name)) return 'bandpass';
  if (/notch|comb/.test(name)) return 'notch';
  return 'lowpass';
}

/** A 2-pole response magnitude (dB) at `f` with resonance `res` (0..1). */
export function filterDb(f: number, cutoff: number, res: number, shape: FilterShape): number {
  const r = f / Math.max(cutoff, 1e-6);
  const peak = 1 + clamp(res, 0, 1) * 8;
  const mag = (x: number): number => 1 / Math.sqrt((1 - x * x) ** 2 + (x / peak) ** 2);
  switch (shape) {
    case 'lowpass':
      return 20 * Math.log10(mag(r));
    case 'highpass':
      return 20 * Math.log10(mag(1 / r));
    case 'bandpass':
      return 20 * Math.log10(mag(r) * (r / peak));
    case 'notch':
      return -Math.min(40, 20 * Math.log10(mag(r)) + 6) - 6;
  }
}

/** The static compressor transfer (dB in -> dB out) with a soft knee. */
export function transferDb(inDb: number, threshold: number, ratio: number, knee: number): number {
  const r = Math.max(ratio, 1);
  const k = Math.max(knee, 0);
  const over = inDb - threshold;
  if (k > 0 && Math.abs(over) <= k / 2) return inDb + ((1 / r - 1) * (over + k / 2) ** 2) / (2 * k);
  return over > 0 ? threshold + over / r : inDb;
}

export type LfoShape = 'sine' | 'saw' | 'tri' | 'square' | 'rand' | 'perlin';

/** The LFO shape a signal call draws (effects with an internal LFO draw a sine). */
export function lfoShape(name: string): LfoShape {
  if (name === 'saw' || name === 'tri' || name === 'square' || name === 'rand' || name === 'perlin') return name;
  return 'sine';
}

/** One period of `shape` at phase `p` (0..1), in -1..1; `rand`/`perlin` are fixed previews. */
export function lfoAt(shape: LfoShape, p: number): number {
  const x = p - Math.floor(p);
  switch (shape) {
    case 'sine':
      return Math.sin(2 * Math.PI * x);
    case 'saw':
      return 2 * x - 1;
    case 'tri':
      return 1 - 4 * Math.abs(x - 0.5);
    case 'square':
      return x < 0.5 ? 1 : -1;
    case 'rand':
      return Math.sin(Math.floor(x * 8) * 12.9898) % 1;
    case 'perlin':
      return 0.6 * Math.sin(2 * Math.PI * x) + 0.4 * Math.sin(6 * Math.PI * x + 1);
  }
}

/** The Bjorklund / euclidean hit pattern, rotated right by `rotation`. */
export function euclidPattern(hits: number, steps: number, rotation = 0): boolean[] {
  const n = Math.max(0, Math.round(steps));
  const k = clamp(Math.round(hits), 0, n);
  const out: boolean[] = [];
  for (let i = 0; i < n; i += 1) out.push(Math.floor(((i + 1) * k) / n) - Math.floor((i * k) / n) === 1);
  if (n === 0) return out;
  const r = wrap(rotation, n);
  return out.map((_, i) => out[(i - r + n) % n] as boolean);
}

/** Seconds per beat at `bpm`. */
export const beatSeconds = (bpm: number): number => 60 / Math.max(bpm, 1);

/** A delay time snapped to the nearest 1/`div` beat (`unit` s or ms). */
export function snapTime(value: number, unit: string, bpm: number, div = 4): number {
  const step = beatSeconds(bpm) / div * (unit === 'ms' ? 1000 : 1);
  if (step <= 0) return value;
  return round6(Math.max(step, Math.round(value / step) * step));
}

/** `data` (interleaved) reduced to `width` min/max pairs of the first channel. */
export function peaks(data: Float32Array, channels: number, width: number): [number, number][] {
  const ch = Math.max(1, channels);
  const frames = Math.floor(data.length / ch);
  const out: [number, number][] = [];
  if (frames === 0 || width <= 0) return out;
  for (let x = 0; x < width; x += 1) {
    const a = Math.floor((x * frames) / width);
    const b = Math.max(a + 1, Math.floor(((x + 1) * frames) / width));
    let lo = 0;
    let hi = 0;
    for (let i = a; i < b && i < frames; i += 1) {
      const v = data[i * ch] ?? 0;
      if (v < lo) lo = v;
      if (v > hi) hi = v;
    }
    out.push([lo, hi]);
  }
  return out;
}
