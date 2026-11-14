// The transport bar (design 15.1.5 "Transport bar", 11.7 clock status,
// 15.2 icons, audio state and eval outcome).
//
// - Tempo and cycle/beat from `tempo`, extrapolated on the tier clock
//   between messages (held while an external MIDI clock is lost).
// - MIDI clock status from `tempo.clock`: `internal`, `midi locked`,
//   `midi lost`, shown as a glyph with the word as its accessible name.
// - Hush sends `hush` (the session's hush already releases with panic);
//   stop-all sends `stop {slot}` for every known slot.
// - The per-slot list is built from `playing` events and the slots named
//   by `eval-result` and `diag` diagnostics (v1 carries no slot table):
//   an activity light lit for 150 ms from each event's time, and a stop
//   button that sends `stop {slot}`, the only per-slot message.
// - The master level readout is `levels[0].rms` in dBFS. Per-slot levels
//   are not available (E3).
// - The audio control shows the AudioContext state and starts it; the eval
//   status shows every eval's outcome, including "not delivered".
//
// The class keeps the imperative API the code area and tests use; the DOM
// is a Solid view over signals (design 15.2).

import { createComponent, createRoot, createSignal, type Accessor, type Setter } from 'solid-js';
import { render } from 'solid-js/web';
import type { Clock } from '../app/clock';
import type { Client } from '../protocol/client';
import { defaultTimers, type Timers } from '../protocol/document';
import type { Diagnostic, LevelsBody, ServerEnvelope, TempoBody, WirePlaying } from '../protocol/types';
import { audioModel, type AudioContextLike, type AudioModel } from '../ui/audio';
import { TransportView, type ClockStatus, type EvalStatus, type Position, type SlotView } from '../ui/transport-view';
import { DEFAULT_BEATS_PER_CYCLE, DEFAULT_BPM, TimeAnchor } from './highlight';

export type { ClockStatus, EvalStatus } from '../ui/transport-view';

export const ACTIVITY_S = 0.15;

/** How long an eval may stay unanswered before it shows "not delivered". */
export const EVAL_TIMEOUT_MS = 3000;

/** Event times kept per slot for the activity light. */
const MAX_SLOT_TIMES = 64;

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

/** The eval status an `eval-result` reply maps to. */
export function evalOutcome(env: ServerEnvelope): EvalStatus {
  if (env.kind !== 'eval-result') return { kind: 'not-delivered', reason: `unexpected reply ${env.kind}` };
  let errors = 0;
  let warnings = 0;
  for (const d of env.body.diagnostics) {
    if (d.severity === 'error') errors += 1;
    else if (d.severity === 'warning') warnings += 1;
  }
  for (const f of env.body.forms) if (f.failure) errors += 1;
  return errors + warnings === 0 ? { kind: 'ok' } : { kind: 'diagnostics', errors, warnings };
}

export interface TransportOptions {
  client: Client;
  clock: Clock;
  anchor?: TimeAnchor;
  /** Called after `hush` is sent. */
  onHush?: () => void;
  /** The browser tier's AudioContext; omitted on the native tier. */
  audio?: AudioContextLike | null;
  /** The Run button (evaluate the whole document); omitted hides it. */
  onRun?: () => void;
  timers?: Timers;
}

interface SlotRow {
  view: SlotView;
  lit: Accessor<boolean>;
  setLit: Setter<boolean>;
  times: number[];
}

export class TransportBar {
  readonly el: HTMLElement;
  readonly audio: AudioModel;
  private readonly opts: TransportOptions;
  private readonly anchor: TimeAnchor;
  private readonly timers: Timers;
  private readonly slots = new Map<string, SlotRow>();
  private readonly setBpm: Setter<number>;
  private readonly setPosition: Setter<Position | null>;
  private readonly setClock: Setter<ClockStatus>;
  private readonly setLevel: Setter<number | null>;
  private readonly setSlotViews: Setter<SlotView[]>;
  private readonly evalStatus: Accessor<EvalStatus>;
  private readonly setEvalStatus: Setter<EvalStatus>;
  private readonly disposeRoot: () => void;
  private readonly disposeRender: () => void;
  private evalSeq = 0;
  private evalTimer: unknown = null;
  private tempo: TempoBody | null = null;
  private tempoAt = 0;

  constructor(parent: HTMLElement, opts: TransportOptions) {
    this.opts = opts;
    this.anchor = opts.anchor ?? new TimeAnchor(opts.clock, 'audio');
    this.timers = opts.timers ?? defaultTimers;
    const doc = parent.ownerDocument;
    this.el = doc.createElement('div');
    this.el.className = 'vact-transport-host';
    parent.appendChild(this.el);

    let setters!: {
      bpm: Setter<number>;
      position: Setter<Position | null>;
      clock: Setter<ClockStatus>;
      level: Setter<number | null>;
      slots: Setter<SlotView[]>;
      evalStatus: Setter<EvalStatus>;
    };
    let evalStatus!: Accessor<EvalStatus>;
    let audio!: AudioModel;
    let accessors!: {
      bpm: Accessor<number>;
      position: Accessor<Position | null>;
      clock: Accessor<ClockStatus>;
      level: Accessor<number | null>;
      slots: Accessor<SlotView[]>;
    };
    this.disposeRoot = createRoot((dispose) => {
      const [bpm, setBpm] = createSignal(DEFAULT_BPM);
      const [position, setPosition] = createSignal<Position | null>(null);
      const [clock, setClock] = createSignal<ClockStatus>('internal');
      const [level, setLevel] = createSignal<number | null>(null);
      const [slots, setSlots] = createSignal<SlotView[]>([]);
      const [status, setStatus] = createSignal<EvalStatus>({ kind: 'idle' });
      setters = { bpm: setBpm, position: setPosition, clock: setClock, level: setLevel, slots: setSlots, evalStatus: setStatus };
      accessors = { bpm, position, clock, level, slots };
      evalStatus = status;
      audio = audioModel(opts.audio ?? null);
      return dispose;
    });
    this.setBpm = setters.bpm;
    this.setPosition = setters.position;
    this.setClock = setters.clock;
    this.setLevel = setters.level;
    this.setSlotViews = setters.slots;
    this.setEvalStatus = setters.evalStatus;
    this.evalStatus = evalStatus;
    this.audio = audio;

    this.disposeRender = render(
      () =>
        createComponent(TransportView, {
          bpm: accessors.bpm,
          position: accessors.position,
          clock: accessors.clock,
          level: accessors.level,
          slots: accessors.slots,
          audio: audio.state,
          audioReason: audio.reason,
          evalStatus,
          onAudio: () => void audio.start(),
          ...(opts.onRun ? { onRun: () => opts.onRun?.() } : {}),
          onHush: () => this.hush(),
          onStopAll: () => this.stopAll(),
          onStop: (slot: string) => this.opts.client.stop(slot),
        }),
      this.el,
    );
  }

  hush(): void {
    this.opts.client.hush();
    this.opts.onHush?.();
  }

  /** `stop {slot}` for every known slot. */
  stopAll(): void {
    for (const slot of this.slotNames()) this.opts.client.stop(slot);
  }

  /** Tracks one eval reply: pending, then its outcome or "not delivered". */
  evalStarted(reply: Promise<ServerEnvelope>): void {
    this.evalSeq += 1;
    const seq = this.evalSeq;
    this.setEvalStatus({ kind: 'pending' });
    if (this.evalTimer !== null) this.timers.clear(this.evalTimer);
    this.evalTimer = this.timers.set(() => {
      this.evalTimer = null;
      if (seq === this.evalSeq && this.evalStatus().kind === 'pending') {
        this.setEvalStatus({ kind: 'not-delivered', reason: 'no reply from the session' });
      }
    }, EVAL_TIMEOUT_MS);
    reply.then(
      (env) => {
        if (seq !== this.evalSeq) return;
        this.setEvalStatus(evalOutcome(env));
      },
      (e: unknown) => {
        if (seq !== this.evalSeq) return;
        this.setEvalStatus({ kind: 'not-delivered', reason: e instanceof Error ? e.message : String(e) });
      },
    );
  }

  /** The current eval status (tests, the status line). */
  currentEvalStatus(): EvalStatus {
    return this.evalStatus();
  }

  onTempo(tempo: TempoBody): void {
    this.tempo = tempo;
    this.tempoAt = this.opts.clock.now();
    this.setBpm(tempo.bpm);
    this.setClock(clockStatus(tempo));
    this.renderPosition();
  }

  onLevels(levels: LevelsBody): void {
    const rms = levels.levels[0]?.rms;
    if (rms === undefined || !Number.isFinite(rms)) this.setLevel(null);
    else this.setLevel(rms <= 0 ? -Infinity : 20 * Math.log10(rms));
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
    return this.slots.get(slot)?.lit() ?? false;
  }

  /** Per frame: the position and the activity lights. */
  tick(): void {
    this.renderPosition();
    const now = this.opts.clock.now();
    for (const row of this.slots.values()) {
      row.times = row.times.filter((t) => t + ACTIVITY_S > now);
      const on = row.times.some((t) => t <= now);
      if (row.lit() !== on) row.setLit(on);
    }
  }

  dispose(): void {
    if (this.evalTimer !== null) this.timers.clear(this.evalTimer);
    this.disposeRender();
    this.audio.dispose();
    this.disposeRoot();
    this.el.remove();
  }

  private renderPosition(): void {
    const c = this.cycles();
    if (c === null) return;
    const bpc = this.tempo && this.tempo.beats_per_cycle > 0 ? this.tempo.beats_per_cycle : DEFAULT_BEATS_PER_CYCLE;
    const cycle = Math.floor(c);
    const beat = Math.floor((c - cycle) * bpc + 1e-9) + 1;
    this.setPosition((prev) =>
      prev && prev.cycle === cycle && prev.beat === beat && prev.beatsPerCycle === bpc ? prev : { cycle, beat, beatsPerCycle: bpc },
    );
  }

  private row(slot: string): SlotRow {
    const found = this.slots.get(slot);
    if (found) return found;
    const [lit, setLit] = createRoot(() => createSignal(false));
    const row: SlotRow = { view: { name: slot, lit }, lit, setLit, times: [] };
    this.slots.set(slot, row);
    // Stable view objects: `For` keeps existing rows and inserts the new one.
    this.setSlotViews(this.slotNames().map((name) => (this.slots.get(name) as SlotRow).view));
    return row;
  }
}
