// The transport bar (design 15.1.5 "Transport bar", 11.7 clock status).
//
// - Tempo and cycle/beat from `tempo`, extrapolated on the tier clock
//   between messages (held while an external MIDI clock is lost).
// - MIDI clock status from `tempo.clock`: `internal`, `midi locked`,
//   `midi lost`.
// - hush/panic sends `hush` (the protocol has no separate panic; the
//   session's hush already releases with panic).
// - The per-slot list is built from `playing` events and the slots named
//   by `eval-result` and `diag` diagnostics (v1 carries no slot table):
//   an activity light lit for 150 ms from each event's time, and a mute
//   button that sends `stop {slot}`, the only per-slot message.
// - The master level readout is `levels[0].rms` in dBFS. Per-slot levels
//   are not available (E3).

import type { Clock } from '../app/clock';
import type { Client } from '../protocol/client';
import type { Diagnostic, LevelsBody, TempoBody, WirePlaying } from '../protocol/types';
import { DEFAULT_BEATS_PER_CYCLE, DEFAULT_BPM, TimeAnchor } from './highlight';

export const ACTIVITY_S = 0.15;

/** Event times kept per slot for the activity light. */
const MAX_SLOT_TIMES = 64;

export type ClockStatus = 'internal' | 'midi locked' | 'midi lost';

export function clockStatus(tempo: TempoBody | null): ClockStatus {
  const c = tempo?.clock;
  if (!c || c.source === 'internal') return 'internal';
  return c.locked ? 'midi locked' : 'midi lost';
}

/** `rms` as a dBFS readout. */
export function formatLevel(rms: number | undefined): string {
  if (rms === undefined || !Number.isFinite(rms)) return 'master --';
  if (rms <= 0) return 'master -inf dB';
  return `master ${(20 * Math.log10(rms)).toFixed(1)} dB`;
}

export interface TransportOptions {
  client: Client;
  clock: Clock;
  anchor?: TimeAnchor;
  /** Called after `hush` is sent. */
  onHush?: () => void;
}

interface SlotRow {
  li: HTMLElement;
  light: HTMLElement;
  times: number[];
}

export class TransportBar {
  readonly el: HTMLElement;
  private readonly opts: TransportOptions;
  private readonly anchor: TimeAnchor;
  private readonly tempoEl: HTMLElement;
  private readonly posEl: HTMLElement;
  private readonly clockEl: HTMLElement;
  private readonly levelEl: HTMLElement;
  private readonly slotsEl: HTMLElement;
  private readonly slots = new Map<string, SlotRow>();
  private tempo: TempoBody | null = null;
  private tempoAt = 0;

  constructor(parent: HTMLElement, opts: TransportOptions) {
    this.opts = opts;
    this.anchor = opts.anchor ?? new TimeAnchor(opts.clock, 'audio');
    const doc = parent.ownerDocument;
    const make = (tag: string, cls: string, text = ''): HTMLElement => {
      const e = doc.createElement(tag);
      e.className = cls;
      e.textContent = text;
      return e;
    };
    this.el = make('div', 'vact-transport');
    this.tempoEl = make('span', 'vact-tempo', `${DEFAULT_BPM.toFixed(1)} bpm`);
    this.posEl = make('span', 'vact-position', 'cycle - beat -');
    this.clockEl = make('span', 'vact-clock', 'internal');
    this.clockEl.dataset.status = 'internal';
    const hush = make('button', 'vact-hush', 'hush');
    hush.setAttribute('type', 'button');
    hush.title = 'hush / panic';
    hush.addEventListener('click', () => this.hush());
    this.levelEl = make('span', 'vact-level', formatLevel(undefined));
    this.slotsEl = make('ul', 'vact-slots');
    this.el.append(this.tempoEl, this.posEl, this.clockEl, hush, this.levelEl, this.slotsEl);
    parent.appendChild(this.el);
  }

  hush(): void {
    this.opts.client.hush();
    this.opts.onHush?.();
  }

  onTempo(tempo: TempoBody): void {
    this.tempo = tempo;
    this.tempoAt = this.opts.clock.now();
    this.tempoEl.textContent = `${tempo.bpm.toFixed(1)} bpm`;
    const status = clockStatus(tempo);
    this.clockEl.textContent = status;
    this.clockEl.dataset.status = status.replace(' ', '-');
    this.renderPosition();
  }

  onLevels(levels: LevelsBody): void {
    this.levelEl.textContent = formatLevel(levels.levels[0]?.rms);
  }

  onPlaying(events: readonly WirePlaying[]): void {
    this.anchor.observe(events.map((e) => e.time));
    for (const ev of events) {
      const row = this.row(ev.slot);
      row.times.push(this.anchor.local(ev.time));
      if (row.times.length > MAX_SLOT_TIMES) row.times.splice(0, row.times.length - MAX_SLOT_TIMES);
    }
  }

  /** Adds the slots named by diagnostics (`eval-result`, `diag`). */
  onDiagnostics(diags: readonly Diagnostic[]): void {
    for (const d of diags) if (d.slot) this.row(d.slot);
  }

  /** The extrapolated position in cycles, or null before the first `tempo`. */
  cycles(): number | null {
    const t = this.tempo;
    if (!t) return null;
    const c0 = t.cycle[1] === 0 ? 0 : t.cycle[0] / t.cycle[1];
    if (clockStatus(t) === 'midi lost') return c0;
    const bpc = t.beats_per_cycle > 0 ? t.beats_per_cycle : DEFAULT_BEATS_PER_CYCLE;
    const dt = Math.max(0, this.opts.clock.now() - this.tempoAt);
    return c0 + (dt * t.bpm) / 60 / bpc;
  }

  /** Slot names in display order. */
  slotNames(): string[] {
    return [...this.slots.keys()].sort();
  }

  /** Whether `slot`'s activity light is lit at the clock's now. */
  lit(slot: string): boolean {
    return this.slots.get(slot)?.light.dataset.lit === 'true';
  }

  /** Per frame: the position and the activity lights. */
  tick(): void {
    this.renderPosition();
    const now = this.opts.clock.now();
    for (const row of this.slots.values()) {
      row.times = row.times.filter((t) => t + ACTIVITY_S > now);
      const on = row.times.some((t) => t <= now);
      row.light.dataset.lit = on ? 'true' : 'false';
    }
  }

  dispose(): void {
    this.el.remove();
  }

  private renderPosition(): void {
    const c = this.cycles();
    if (c === null) return;
    const bpc = this.tempo && this.tempo.beats_per_cycle > 0 ? this.tempo.beats_per_cycle : DEFAULT_BEATS_PER_CYCLE;
    const cycle = Math.floor(c);
    const beat = Math.floor((c - cycle) * bpc + 1e-9) + 1;
    this.posEl.textContent = `cycle ${cycle} beat ${beat}`;
  }

  private row(slot: string): SlotRow {
    const found = this.slots.get(slot);
    if (found) return found;
    const doc = this.el.ownerDocument;
    const li = doc.createElement('li');
    li.className = 'vact-slot';
    li.dataset.slot = slot;
    const light = doc.createElement('span');
    light.className = 'vact-light';
    light.dataset.lit = 'false';
    const name = doc.createElement('span');
    name.className = 'vact-slot-name';
    name.textContent = slot;
    const mute = doc.createElement('button');
    mute.className = 'vact-mute';
    mute.setAttribute('type', 'button');
    mute.textContent = 'mute';
    mute.title = `stop ${slot}`;
    mute.addEventListener('click', () => this.opts.client.stop(slot));
    li.append(light, name, mute);
    // Keep the list sorted by slot name.
    const next = [...this.slots.keys()].filter((s) => s > slot).sort()[0];
    const before = next === undefined ? null : (this.slots.get(next)?.li ?? null);
    this.slotsEl.insertBefore(li, before);
    const row = { li, light, times: [] };
    this.slots.set(slot, row);
    return row;
  }
}
