// Criterion 3 (meters): master rms and bands plus the analyzer entries
// render from scripted `levels`; a later `levels` repaints only the
// displays whose data changed (design 12.5, 15.1.8, G4).

import { afterEach, describe, expect, it } from 'vitest';

import { Store } from '../../src/protocol/store';
import type { LevelsBody } from '../../src/protocol/types';
import { AnalyzerArea, analyzerKey, LevelsTimeline, pitchReadout, stereoReadout } from '../../src/visual/meters';
import { installCanvasFakes, type CanvasFakes } from '../support/canvas';

let fakes: CanvasFakes | null = null;

afterEach(() => {
  fakes?.restore();
  fakes = null;
});

const bands8 = [0.5, 0.4, 0.3, 0.2, 0.1, 0.05, 0.02, 0.01];
const spectrum32 = (scale: number): number[] => Array.from({ length: 32 }, (_, i) => scale / (i + 1));

const LEVEL = { bus: ':master', kind: 'level', id: 0, cells: [0.3, 0.6] };
const SPECTRUM = { bus: 'drums', kind: 'spectrum', id: 2, cells: spectrum32(1) };

function levels(body: LevelsBody): { kind: 'levels'; body: LevelsBody } {
  return { kind: 'levels', body };
}

function canvasOf(el: HTMLElement): HTMLCanvasElement {
  return el.querySelector('canvas') as HTMLCanvasElement;
}

describe('AnalyzerArea', () => {
  it('renders master rms, bands and analyzer entries, then repaints only what changed', () => {
    fakes = installCanvasFakes();
    const store = new Store();
    const parent = document.createElement('div');
    const area = new AnalyzerArea(parent, store);
    // Nothing published yet: nothing painted.
    expect(area.master.meter.paints).toBe(0);

    store.apply(levels({ levels: [{ source: ':master', rms: 0.5, bands: bands8 }], analyzers: [LEVEL, SPECTRUM] }));
    expect(area.master.meter.paints).toBe(1);
    expect(area.master.bands.paints).toBe(1);
    const displays = area.displays();
    expect([...displays.keys()]).toEqual([analyzerKey(LEVEL), analyzerKey(SPECTRUM)]);
    const level = displays.get(analyzerKey(LEVEL));
    const spectrum = displays.get(analyzerKey(SPECTRUM));
    expect(level?.paints).toBe(1);
    expect(spectrum?.paints).toBe(1);

    // The rms bar is on the dBFS scale: 0.5 is about -6 dBFS of a -60 floor.
    const meterCtx = fakes.ctx(canvasOf(area.master.meter.el));
    const bar = meterCtx.named('fillRect')[0]?.args as number[];
    expect(bar[2]).toBeCloseTo(160 * (1 - 6.0206 / 60), 1);
    expect(fakes.ctx(canvasOf(area.master.bands.el)).named('fillRect')).toHaveLength(8);
    expect(fakes.ctx(canvasOf((spectrum as { el: HTMLElement }).el)).named('fillRect')).toHaveLength(32);
    // The level entry draws rms and a peak tick.
    expect(fakes.ctx(canvasOf((level as { el: HTMLElement }).el)).named('fillRect')).toHaveLength(2);

    // Second batch: only the spectrum entry changed.
    const quieter = { ...SPECTRUM, cells: spectrum32(0.5) };
    store.apply(levels({ levels: [{ source: ':master', rms: 0.5, bands: bands8 }], analyzers: [LEVEL, quieter] }));
    expect(area.master.meter.paints).toBe(1);
    expect(area.master.bands.paints).toBe(1);
    expect(level?.paints).toBe(1);
    expect(spectrum?.paints).toBe(2);

    // Third batch: master rms moves; the level entry is no longer published.
    store.apply(levels({ levels: [{ source: ':master', rms: 0.25, bands: bands8 }], analyzers: [quieter] }));
    expect(area.master.meter.paints).toBe(2);
    expect(area.master.bands.paints).toBe(1);
    expect(spectrum?.paints).toBe(2);
    expect(area.displays().has(analyzerKey(LEVEL))).toBe(false);
    expect(parent.querySelectorAll('.visual-analyzer-list > .visual-display')).toHaveLength(1);

    area.dispose();
    expect(parent.children).toHaveLength(0);
    store.apply(levels({ levels: [{ source: ':master', rms: 0.9 }] }));
    expect(area.master.meter.paints).toBe(2);
  });

  it('renders pitch, stereo and unknown kinds as readouts', () => {
    fakes = installCanvasFakes();
    const store = new Store();
    const parent = document.createElement('div');
    const area = new AnalyzerArea(parent, store);
    store.apply(
      levels({
        levels: [{ source: ':master', rms: 0 }],
        analyzers: [
          { bus: 'lead', kind: 'pitch-meter', id: 10, cells: [440, 0.9] },
          { bus: 'lead', kind: 'stereo-meter', id: 12, cells: [1, 0, -0.25] },
          { bus: 'lead', kind: 'future-kind', id: 20, cells: [1, 2.5] },
        ],
      }),
    );
    const text = [...parent.querySelectorAll('.visual-readout')].map((e) => e.textContent);
    expect(text).toEqual(['440.0 Hz A4 (conf 0.90)', 'corr 1.00 width 0.00 bal -0.25', '1.000 2.500']);
    area.dispose();
  });

  it('formats readouts defensively', () => {
    expect(pitchReadout([0, 0])).toBe('- (conf 0.00)');
    expect(pitchReadout([261.63, 1])).toBe('261.6 Hz C4 (conf 1.00)');
    expect(stereoReadout([])).toBe('corr 0.00 width 0.00 bal 0.00');
  });

  it('bounds timestamp history and marks stale and untimestamped presentations', () => {
    fakes = installCanvasFakes();
    const timeline = new LevelsTimeline();
    for (let i = 0; i < 10; i++) timeline.push({ time: i, levels: [], analyzers: [] }, i);
    expect(timeline.size).toBe(8);
    expect(timeline.at(9)?.time).toBe(9);
    expect(timeline.at(12)).toBeNull();

    const store = new Store();
    const area = new AnalyzerArea(document.createElement('div'), store, () => 4);
    area.receive({ time: 1, levels: [{ source: ':master', rms: 0.1 }] });
    area.present(4);
    expect(area.el.dataset.sync).toBe('hidden');
    expect(area.el.hidden).toBe(true);
    area.receive({ levels: [{ source: ':master', rms: 0.2 }] });
    expect(area.el.dataset.sync).toBe('unsynced');
    expect(area.el.hidden).toBe(false);
    area.dispose();
  });
});
