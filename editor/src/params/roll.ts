// The piano roll (design 15.1.7): a DISPLAY of `playing` events placed by
// beat within the cycle and by pitch. The pitch is read, for display only,
// from the source text at the event's mapped `src` span (`mapWireSpan`
// plus the document text): a note keyword (`:a4`, `:c#3`) or a number (a
// MIDI note). Any other text goes to the unpitched lane. Like the grid,
// this module has no write-back path and registers no input handler.

import type { CodeApi } from '../app/apis';
import type { WirePlaying } from '../protocol/types';
import { createComponent, createSignal, type Setter } from 'solid-js';
import { render } from 'solid-js/web';
import { RollView, type RollDisplayNote } from './telemetry-view';

export interface RollNote {
  slot: string;
  pos: number;
  len: number;
  /** A MIDI note, or null for the unpitched lane. */
  pitch: number | null;
  text: string;
}

const PC: Record<string, number> = { c: 0, d: 2, e: 4, f: 5, g: 7, a: 9, b: 11 };

/** A note keyword (`:a4`, `:c#3`, `:eb2`) or a number -> a MIDI note, else null. */
export function parsePitch(text: string): number | null {
  const t = text.trim();
  const kw = /^:([a-gA-G])(#|s|b)?(-?\d+)$/.exec(t);
  if (kw) {
    const base = PC[(kw[1] as string).toLowerCase()] as number;
    const acc = kw[2] === '#' || kw[2] === 's' ? 1 : kw[2] === 'b' ? -1 : 0;
    return (Number(kw[3]) + 1) * 12 + base + acc;
  }
  if (/^-?\d+(\.\d+)?$/.test(t)) {
    const n = Math.round(Number(t));
    return n >= 0 && n <= 127 ? n : null;
  }
  return null;
}

const beatOf = (r: readonly [number, number]): number => (r[1] === 0 ? 0 : r[0] / r[1]);

export class PianoRoll {
  readonly el: HTMLElement;
  private readonly code: () => CodeApi | undefined;
  private cycle = -1;
  private list: RollNote[] = [];
  private readonly setNotes: Setter<RollDisplayNote[]>;
  private readonly disposeView: () => void;

  constructor(parent: HTMLElement, code: () => CodeApi | undefined) {
    this.code = code;
    const holder = parent.ownerDocument.createElement('div');
    const [notes, setNotes] = createSignal<RollDisplayNote[]>([]);
    this.setNotes = setNotes;
    this.disposeView = render(() => createComponent(RollView, { notes }), holder);
    this.el = holder.firstElementChild as HTMLElement;
    parent.appendChild(this.el);
  }

  /** Adds one `playing` batch (only the latest cycle is kept) and redraws. */
  update(events: readonly WirePlaying[], beatsPerCycle: number): void {
    const bpc = beatsPerCycle > 0 ? beatsPerCycle : 4;
    for (const e of events) {
      const beat = beatOf(e.beat);
      const cycle = Math.floor(beat / bpc);
      if (cycle > this.cycle) {
        this.cycle = cycle;
        this.list = [];
      } else if (cycle < this.cycle) {
        continue;
      }
      const text = this.sourceText(e);
      this.list.push({ slot: e.slot, pos: (beat - cycle * bpc) / bpc, len: beatOf(e.dur) / bpc, pitch: parsePitch(text), text });
    }
    this.draw();
  }

  notes(): RollNote[] {
    return this.list;
  }

  dispose(): void {
    this.disposeView();
    this.el.remove();
  }

  /** The literal text under the event's source span (read only). */
  private sourceText(e: WirePlaying): string {
    const code = this.code();
    if (!e.src || !code) return '';
    const r = code.mapWireSpan(e.src.span, e.src.doc_revision);
    return r ? code.view.state.sliceDoc(r.from, r.to) : '';
  }

  private draw(): void {
    const pitches = this.list.map((n) => n.pitch).filter((p): p is number => p !== null);
    const hi = pitches.length > 0 ? Math.max(...pitches) : 0;
    const lo = pitches.length > 0 ? Math.min(...pitches) : 0;
    const lanes = hi - lo + 2;
    this.setNotes(this.list.map((n) => {
      const lane = n.pitch === null ? lanes - 1 : hi - n.pitch;
      return { ...n, lane: n.pitch === null ? 'unpitched' : undefined, top: (lane / lanes) * 100 };
    }));
  }
}
