// Ring decoding of the `spectrogram` and `oscilloscope` cells honors the
// index cell (src/dsp/effects/analyzer.rs), and `mountSpectrum` prefers the
// bus analyzer over the master bands (design 12.5, 15.1.8).

import { afterEach, describe, expect, it } from 'vitest';

import { Store } from '../../src/protocol/store';
import type { LevelsBody } from '../../src/protocol/types';
import { AnalyzerArea } from '../../src/visual/meters';
import { decodeRing, scopeSamples, SPECTROGRAM_RING } from '../../src/visual/scopes';
import { mountSpectrum } from '../../src/visual/spectrum';
import { installCanvasFakes, type CanvasFakes } from '../support/canvas';

let fakes: CanvasFakes | null = null;

afterEach(() => {
  fakes?.restore();
  fakes = null;
});

/** 16 frames x 32 bands, every cell of frame f = (f + 1) * scale; last cell = `newest`. */
function spectrogramCells(newest: number, scale = 1): number[] {
  const cells: number[] = [];
  for (let f = 0; f < 16; f++) for (let b = 0; b < 32; b++) cells.push((f + 1) * scale);
  cells.push(newest);
  return cells;
}

/** 256 samples, sample i = i / 256; last cell = the next write position. */
function scopeCells(next: number): number[] {
  return [...Array.from({ length: 256 }, (_, i) => i / 256), next];
}

describe('ring decoding', () => {
  it('orders spectrogram frames oldest first after the most recently written frame', () => {
    const frames = decodeRing(spectrogramCells(5), SPECTROGRAM_RING);
    expect(frames).toHaveLength(16);
    expect(frames.every((f) => f.length === 32)).toBe(true);
    // Frame 5 was written last, so frame 6 is the oldest.
    expect(frames.map((f) => f[0])).toEqual([7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 1, 2, 3, 4, 5, 6]);
  });

  it('orders oscilloscope samples from the next write position', () => {
    const samples = scopeSamples(scopeCells(100));
    expect(samples).toHaveLength(256);
    expect(samples[0]).toBe(100 / 256);
    expect(samples[155]).toBe(255 / 256);
    expect(samples[156]).toBe(0);
    expect(samples[255]).toBe(99 / 256);
  });

  it('reads an out-of-range or missing index as 0', () => {
    expect(scopeSamples(scopeCells(999))[0]).toBe(0);
    expect(decodeRing([], SPECTROGRAM_RING)).toEqual([]);
  });

  it('draws the scope polyline and the spectrogram image in ring order', () => {
    fakes = installCanvasFakes();
    const store = new Store();
    const parent = document.createElement('div');
    const area = new AnalyzerArea(parent, store);
    store.apply({
      kind: 'levels',
      body: {
        levels: [{ source: ':master', rms: 0 }],
        analyzers: [
          { bus: 'lead', kind: 'oscilloscope', id: 0, cells: scopeCells(64) },
          { bus: 'lead', kind: 'spectrogram', id: 300, cells: spectrogramCells(15, 1e-4) },
        ],
      },
    });
    const [scopeEl, specEl] = [...parent.querySelectorAll<HTMLElement>('.visual-analyzer-list > .visual-display')];
    const scope = fakes.ctx(scopeEl?.querySelector('canvas') as HTMLCanvasElement);
    const start = scope.named('moveTo')[0]?.args as number[];
    // Sample 64/256 = 0.25 first: y = 24 - 0.25 * 24.
    expect(start).toEqual([0, 18]);
    expect(scope.named('lineTo')).toHaveLength(255);
    const spec = fakes.ctx(specEl?.querySelector('canvas') as HTMLCanvasElement);
    const rects = spec.named('fillRect');
    expect(rects).toHaveLength(16 * 32);
    // Newest = 15, so frame 0 (value 1, the dimmest) is the leftmost column.
    const colors = rects.filter((r) => r.args[0] === 0).map((r) => r.args[4]);
    const brightest = rects.filter((r) => (r.args[0] as number) > 149).map((r) => r.args[4]);
    expect(new Set(colors).size).toBe(1);
    expect(colors[0]).not.toBe(brightest[0]);
    area.dispose();
  });
});

describe('mountSpectrum', () => {
  const body: LevelsBody = {
    levels: [{ source: ':master', rms: 0.1, bands: [0.5, 0.4, 0.3, 0.2, 0.1, 0.05, 0.02, 0.01] }],
    analyzers: [{ bus: 'drums', kind: 'spectrum', id: 4, cells: Array.from({ length: 32 }, () => 0.2) }],
  };

  it('prefers the bus spectrum analyzer over the master bands', () => {
    fakes = installCanvasFakes();
    const store = new Store();
    store.apply({ kind: 'levels', body });
    const el = document.createElement('div');
    const drums = mountSpectrum(el, store, { bus: 'drums' });
    const canvas = el.querySelector('canvas') as HTMLCanvasElement;
    expect(canvas.dataset.source).toBe('analyzer');
    expect(fakes.ctx(canvas).named('fillRect')).toHaveLength(32);
    drums.dispose();
    expect(el.querySelector('canvas')).toBeNull();

    const bass = mountSpectrum(el, store, { bus: 'bass' });
    const c2 = el.querySelector('canvas') as HTMLCanvasElement;
    expect(c2.dataset.source).toBe('master');
    expect(fakes.ctx(c2).named('fillRect')).toHaveLength(8);
    bass.dispose();
  });

  it('repaints only when its chosen data changes', () => {
    fakes = installCanvasFakes();
    const store = new Store();
    const el = document.createElement('div');
    const m = mountSpectrum(el, store, {});
    const canvas = el.querySelector('canvas') as HTMLCanvasElement;
    expect(canvas.dataset.source).toBe('none');
    store.apply({ kind: 'levels', body });
    const ctx = fakes.ctx(canvas);
    expect(canvas.dataset.source).toBe('master');
    expect(ctx.named('clearRect')).toHaveLength(2);
    // Only the (unrelated) analyzer changes.
    store.apply({ kind: 'levels', body: { ...body, analyzers: [] } });
    expect(ctx.named('clearRect')).toHaveLength(2);
    m.dispose();
    store.apply({ kind: 'levels', body: { levels: [{ source: ':master', rms: 0, bands: [1] }] } });
    expect(ctx.named('clearRect')).toHaveLength(2);
  });
});
