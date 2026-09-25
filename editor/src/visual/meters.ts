// The analyzer area (design 12.5, 15.1.8, G4): a master level meter
// (`rms`, dBFS), the master 8-band spectrum (`bands`) and one display per
// published analyzer entry, chosen by `kind`. Everything renders from the
// store's `levels` message; a display repaints only when its data changed,
// and displays of entries no longer published are removed. There is no
// audio input and no `getUserMedia` anywhere (E2).

import type { Store } from '../protocol/store';
import type { LevelsBody, WireAnalyzer } from '../protocol/types';
import {
  decodeRing,
  drawScope,
  drawSpectrogram,
  NOTE_SPECTROGRAM_RING,
  scopeSamples,
  SPECTROGRAM_RING,
} from './scopes';
import { dbFraction, drawBars, masterLevel, METER_FLOOR_DB, num, sameCells } from './spectrum';

export const DISPLAY_WIDTH = 160;
export const METER_HEIGHT = 14;
export const PLOT_HEIGHT = 48;

type Draw = (ctx: CanvasRenderingContext2D, w: number, h: number, cells: readonly number[]) => void;

export interface AnalyzerDisplay {
  readonly el: HTMLElement;
  /** Repaints so far. */
  readonly paints: number;
  update(cells: readonly number[]): void;
}

function frame(doc: Document, title: string, kind: string): { el: HTMLElement; body: HTMLElement } {
  const el = doc.createElement('div');
  el.className = 'visual-display';
  el.dataset.kind = kind;
  const label = doc.createElement('span');
  label.className = 'visual-display-title';
  label.textContent = title;
  el.appendChild(label);
  return { el, body: el };
}

class CanvasDisplay implements AnalyzerDisplay {
  readonly el: HTMLElement;
  paints = 0;
  private readonly ctx: CanvasRenderingContext2D | null;
  private readonly w: number;
  private readonly h: number;
  private readonly draw: Draw;

  constructor(doc: Document, title: string, kind: string, h: number, draw: Draw) {
    const f = frame(doc, title, kind);
    this.el = f.el;
    const canvas = doc.createElement('canvas');
    canvas.width = DISPLAY_WIDTH;
    canvas.height = h;
    f.body.appendChild(canvas);
    this.ctx = canvas.getContext('2d');
    this.w = DISPLAY_WIDTH;
    this.h = h;
    this.draw = draw;
  }

  update(cells: readonly number[]): void {
    this.paints++;
    if (!this.ctx) return;
    this.ctx.clearRect(0, 0, this.w, this.h);
    this.draw(this.ctx, this.w, this.h, cells);
  }
}

class ReadoutDisplay implements AnalyzerDisplay {
  readonly el: HTMLElement;
  paints = 0;
  private readonly text: HTMLElement;
  private readonly format: (cells: readonly number[]) => string;

  constructor(doc: Document, title: string, kind: string, format: (cells: readonly number[]) => string) {
    const f = frame(doc, title, kind);
    this.el = f.el;
    this.text = doc.createElement('span');
    this.text.className = 'visual-readout';
    f.body.appendChild(this.text);
    this.format = format;
  }

  update(cells: readonly number[]): void {
    this.paints++;
    this.text.textContent = this.format(cells);
  }
}

/** An rms bar on the dBFS scale plus a peak tick when the cells carry one. */
export function drawMeter(ctx: CanvasRenderingContext2D, w: number, h: number, cells: readonly number[]): void {
  ctx.fillStyle = '#5ec46e';
  ctx.fillRect(0, 0, dbFraction(num(cells[0]), METER_FLOOR_DB) * w, h);
  if (cells.length > 1) {
    ctx.fillStyle = '#f0c040';
    ctx.fillRect(Math.max(0, dbFraction(num(cells[1]), METER_FLOOR_DB) * w - 1), 0, 2, h);
  }
}

const NOTES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];

/** `pitch-meter`: `[frequency, confidence]`. */
export function pitchReadout(cells: readonly number[]): string {
  const f = num(cells[0]);
  const conf = num(cells[1]).toFixed(2);
  if (f <= 0) return `- (conf ${conf})`;
  const midi = Math.round(69 + 12 * Math.log2(f / 440));
  const name = `${NOTES[((midi % 12) + 12) % 12]}${Math.floor(midi / 12) - 1}`;
  return `${f.toFixed(1)} Hz ${name} (conf ${conf})`;
}

/** `stereo-meter`: `[correlation, width, balance]`. */
export function stereoReadout(cells: readonly number[]): string {
  const [c, w, b] = [0, 1, 2].map((i) => num(cells[i]).toFixed(2));
  return `corr ${c} width ${w} bal ${b}`;
}

/** Unknown kinds: the numeric cells. */
export function numericReadout(cells: readonly number[]): string {
  const shown = cells.slice(0, 32).map((v) => num(v).toFixed(3));
  return cells.length > 32 ? `${shown.join(' ')} ...` : shown.join(' ');
}

/** The display of one analyzer entry, by kind. */
export function createAnalyzerDisplay(doc: Document, a: WireAnalyzer): AnalyzerDisplay {
  const title = `${a.kind} ${a.bus} #${a.id}`;
  switch (a.kind) {
    case 'level':
      return new CanvasDisplay(doc, title, a.kind, METER_HEIGHT, drawMeter);
    case 'spectrum':
      return new CanvasDisplay(doc, title, a.kind, PLOT_HEIGHT, (ctx, w, h, c) => drawBars(ctx, w, h, c.map(num)));
    case 'spectrogram':
      return new CanvasDisplay(doc, title, a.kind, PLOT_HEIGHT, (ctx, w, h, c) =>
        drawSpectrogram(ctx, w, h, decodeRing(c, SPECTROGRAM_RING)),
      );
    case 'note-spectrogram':
      return new CanvasDisplay(doc, title, a.kind, PLOT_HEIGHT, (ctx, w, h, c) =>
        drawSpectrogram(ctx, w, h, decodeRing(c, NOTE_SPECTROGRAM_RING)),
      );
    case 'oscilloscope':
      return new CanvasDisplay(doc, title, a.kind, PLOT_HEIGHT, (ctx, w, h, c) =>
        drawScope(ctx, w, h, scopeSamples(c)),
      );
    case 'pitch-meter':
      return new ReadoutDisplay(doc, title, a.kind, pitchReadout);
    case 'stereo-meter':
      return new ReadoutDisplay(doc, title, a.kind, stereoReadout);
    default:
      return new ReadoutDisplay(doc, title, a.kind, numericReadout);
  }
}

export const analyzerKey = (a: WireAnalyzer): string => `${a.bus}|${a.kind}|${a.id}`;

export class AnalyzerArea {
  readonly el: HTMLElement;
  readonly master: { meter: AnalyzerDisplay; bands: AnalyzerDisplay };
  private readonly list: HTMLElement;
  private readonly entries = new Map<string, { display: AnalyzerDisplay; cells: readonly number[] }>();
  private lastRms: readonly number[] | undefined;
  private lastBands: readonly number[] | undefined;
  private readonly off: () => void;

  constructor(parent: HTMLElement, store: Store) {
    const doc = parent.ownerDocument;
    this.el = doc.createElement('section');
    this.el.className = 'visual-analyzers';
    this.el.dataset.area = 'analyzers';
    const master = doc.createElement('div');
    master.className = 'visual-master';
    this.master = {
      meter: new CanvasDisplay(doc, 'master rms', 'master-rms', METER_HEIGHT, drawMeter),
      bands: new CanvasDisplay(doc, 'master bands', 'master-bands', PLOT_HEIGHT, (ctx, w, h, c) =>
        drawBars(ctx, w, h, c),
      ),
    };
    master.append(this.master.meter.el, this.master.bands.el);
    this.list = doc.createElement('div');
    this.list.className = 'visual-analyzer-list';
    this.el.append(master, this.list);
    parent.appendChild(this.el);
    this.off = store.subscribe(['levels'], () => this.update(store.levels));
    this.update(store.levels);
  }

  displays(): ReadonlyMap<string, AnalyzerDisplay> {
    return new Map([...this.entries].map(([k, v]) => [k, v.display]));
  }

  update(body: LevelsBody | null): void {
    if (!body) return;
    const m = masterLevel(body);
    if (m) {
      const rms = [num(m.rms)];
      if (!sameCells(this.lastRms, rms)) {
        this.lastRms = rms;
        this.master.meter.update(rms);
      }
      const bands = (m.bands ?? []).map(num);
      if (!sameCells(this.lastBands, bands)) {
        this.lastBands = bands;
        this.master.bands.update(bands);
      }
    }
    const seen = new Set<string>();
    for (const a of body.analyzers ?? []) {
      const key = analyzerKey(a);
      if (seen.has(key)) continue;
      seen.add(key);
      const prev = this.entries.get(key);
      if (prev && sameCells(prev.cells, a.cells)) continue;
      const display = prev?.display ?? createAnalyzerDisplay(this.el.ownerDocument, a);
      if (!prev) this.list.appendChild(display.el);
      this.entries.set(key, { display, cells: a.cells.slice() });
      display.update(a.cells);
    }
    for (const [key, e] of this.entries) {
      if (seen.has(key)) continue;
      e.display.el.remove();
      this.entries.delete(key);
    }
  }

  dispose(): void {
    this.off();
    this.el.remove();
    this.entries.clear();
  }
}
