// The step grid (design 15.1.7, 13.5 "Sequences are code-only"): a
// DISPLAY of `playing` events, one row per slot, each step placed by its
// beat within the current cycle. This module has no write-back path: it
// imports nothing that can send a message or change the document, and it
// registers no input handler.

import type { WirePlaying } from '../protocol/types';

export interface GridStep {
  /** 0..1 within the cycle. */
  pos: number;
  /** Cycle fraction. */
  len: number;
}

const beatOf = (r: readonly [number, number]): number => (r[1] === 0 ? 0 : r[0] / r[1]);

/** Per-slot steps of the latest cycle seen, fed from `playing` batches. */
export class CycleSteps {
  private readonly slots = new Map<string, { cycle: number; steps: GridStep[] }>();

  add(events: readonly WirePlaying[], beatsPerCycle: number): void {
    const bpc = beatsPerCycle > 0 ? beatsPerCycle : 4;
    for (const e of events) {
      const beat = beatOf(e.beat);
      const cycle = Math.floor(beat / bpc);
      let s = this.slots.get(e.slot);
      if (!s || cycle > s.cycle) {
        s = { cycle, steps: [] };
        this.slots.set(e.slot, s);
      } else if (cycle < s.cycle) {
        continue;
      }
      const pos = (beat - cycle * bpc) / bpc;
      if (s.steps.some((x) => Math.abs(x.pos - pos) < 1e-9)) continue;
      s.steps.push({ pos, len: beatOf(e.dur) / bpc });
      s.steps.sort((a, b) => a.pos - b.pos);
    }
  }

  slotNames(): string[] {
    return [...this.slots.keys()].sort();
  }

  steps(slot: string): GridStep[] {
    return this.slots.get(slot)?.steps ?? [];
  }

  clear(): void {
    this.slots.clear();
  }
}

export class StepGrid {
  readonly el: HTMLElement;
  private readonly data = new CycleSteps();

  constructor(parent: HTMLElement) {
    this.el = parent.ownerDocument.createElement('div');
    this.el.className = 'params-grid';
    parent.appendChild(this.el);
  }

  /** Adds one `playing` batch and redraws. */
  update(events: readonly WirePlaying[], beatsPerCycle: number): void {
    this.data.add(events, beatsPerCycle);
    this.draw();
  }

  steps(slot: string): GridStep[] {
    return this.data.steps(slot);
  }

  clear(): void {
    this.data.clear();
    this.draw();
  }

  dispose(): void {
    this.el.remove();
  }

  private draw(): void {
    const doc = this.el.ownerDocument;
    this.el.textContent = '';
    for (const slot of this.data.slotNames()) {
      const row = doc.createElement('div');
      row.className = 'params-grid-row';
      row.dataset.slot = slot;
      const label = doc.createElement('span');
      label.className = 'params-grid-slot';
      label.textContent = slot;
      row.appendChild(label);
      for (const s of this.data.steps(slot)) {
        const cell = doc.createElement('span');
        cell.className = 'params-grid-step';
        cell.dataset.pos = String(Number(s.pos.toFixed(6)));
        cell.style.left = `${s.pos * 100}%`;
        cell.style.width = `${Math.max(0.5, s.len * 100)}%`;
        row.appendChild(cell);
      }
      this.el.appendChild(row);
    }
  }
}
