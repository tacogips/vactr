// The transport bar (design 15.1.5 "Transport bar", 11.7 clock status,
// 15.2 icons, audio state and eval outcome).
//
// - Position advances only while scheduled music is active. The session clock
//   can run in an empty session; its `running` flag alone is not playback.
// - Pause freezes existing bindings; Stop revokes them and lets tails finish.
// - MIDI clock status from `tempo.clock`: `internal`, `midi locked`,
//   `midi lost`, shown as a glyph with the word as its accessible name.
// - Hush sends `hush` for an immediate cut; stop-all sends one session-level
//   `stop-all` message for every slot, including slots not seen by the editor.
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
import type { AudibleClock, Clock } from '../app/clock';
import type { Client } from '../protocol/client';
import { defaultTimers, type Timers } from '../protocol/document';
import type { Diagnostic, LevelsBody, ServerEnvelope, TempoBody, TransportSample, WirePlaying } from '../protocol/types';
import { audioModel, type AudioContextLike, type AudioModel } from '../ui/audio';
import { isApplePlatform, stopShortcut } from '../ui/stop-keys';
import { TransportView, type ClockStatus, type EvalStatus, type Position, type PlaybackState, type SlotView } from '../ui/transport-view';
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
  audible?: AudibleClock;
  sample?: () => TransportSample | null;
  /** Install the app-level stop shortcuts unless explicitly disabled. */
  shortcuts?: boolean;
  /** Platform override for shortcut tests. */
  platform?: string;
  /** Called after stop-all or hush to clear the highlight scheduler. */
  onHush?: () => void;
  /** The browser tier's AudioContext; omitted on the native tier. */
  audio?: AudioContextLike | null;
  /** Evaluates the whole document from Play or Update. */
  onRun?: () => void;
  /** Explain why the current playback cannot pause, or return an empty string. */
  pauseReason?: () => string;
  /** Optional per-viewer Vim mode toggle. */
  vim?: { enabled: Accessor<boolean>; toggle: () => void };
  inline?: { enabled: Accessor<boolean>; toggle: () => void };
  unapplied?: { color: Accessor<string>; setColor: (color: string) => void };
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
  readonly state = { cycle: null as number | null, beatFlash: false, hidden: true };
  private readonly opts: TransportOptions;
  private readonly anchor: TimeAnchor;
  private readonly timers: Timers;
  private readonly slots = new Map<string, SlotRow>();
  private readonly setBpm: Setter<number>;
  private readonly setPosition: Setter<Position | null>;
  private readonly setClock: Setter<ClockStatus>;
  private readonly setLevel: Setter<number | null>;
  private readonly setOutput: Setter<'running' | 'draining' | 'cutting' | 'idle'>;
  private readonly setSlotViews: Setter<SlotView[]>;
  private readonly output: Accessor<'running' | 'draining' | 'cutting' | 'idle'>;
  private readonly evalStatus: Accessor<EvalStatus>;
  private readonly setEvalStatus: Setter<EvalStatus>;
  private readonly disposeRoot: () => void;
  private readonly disposeRender: () => void;
  private readonly stopKeyWindow: Window | null;
  private readonly stopKeyHandler: ((event: KeyboardEvent) => void) | null;
  private readonly outputStates: string[] = [];
  private evalSeq = 0;
  private evalTimer: unknown = null;
  private tempo: TempoBody | null = null;
  private tempoAt = 0;
  private playback!: Accessor<PlaybackState>;
  private setPlayback!: Setter<PlaybackState>;
  private legacyActive = false;
  private stopHeld = false;
  private pauseRequested: boolean | null = null;
  private pausedCycle: number | null = null;
  private pauseReason!: Accessor<string>;
  private setPauseReason!: Setter<string>;

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
      output: Setter<'running' | 'draining' | 'cutting' | 'idle'>;
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
      output: Accessor<'running' | 'draining' | 'cutting' | 'idle'>;
      slots: Accessor<SlotView[]>;
    };
    this.disposeRoot = createRoot((dispose) => {
      const [bpm, setBpm] = createSignal(DEFAULT_BPM);
      const [position, setPosition] = createSignal<Position | null>(null);
      const [clock, setClock] = createSignal<ClockStatus>('internal');
      const [level, setLevel] = createSignal<number | null>(null);
      const [output, setOutput] = createSignal<'running' | 'draining' | 'cutting' | 'idle'>('idle');
      [this.playback, this.setPlayback] = createSignal<PlaybackState>('stopped');
      [this.pauseReason, this.setPauseReason] = createSignal(opts.pauseReason?.() ?? '');
      const [slots, setSlots] = createSignal<SlotView[]>([]);
      const [status, setStatus] = createSignal<EvalStatus>({ kind: 'idle' });
      setters = { bpm: setBpm, position: setPosition, clock: setClock, level: setLevel, output: setOutput, slots: setSlots, evalStatus: setStatus };
      accessors = { bpm, position, clock, level, output, slots };
      evalStatus = status;
      audio = audioModel(opts.audio ?? null);
      return dispose;
    });
    this.setBpm = setters.bpm;
    this.setPosition = setters.position;
    this.setClock = setters.clock;
    this.setLevel = setters.level;
    this.setOutput = setters.output;
    this.setSlotViews = setters.slots;
    this.output = accessors.output;
    this.setEvalStatus = setters.evalStatus;
    this.evalStatus = evalStatus;
    this.audio = audio;

    this.disposeRender = render(
      () =>
        createComponent(TransportView, {
          playback: this.playback,
          pauseReason: this.pauseReason,
          onToggle: () => this.playback() === 'stopped' ? this.play() : this.stopAll(),
          onPause: () => this.togglePause(),
          bpm: accessors.bpm,
          position: accessors.position,
          clock: accessors.clock,
          level: accessors.level,
          output: accessors.output,
          slots: accessors.slots,
          audio: audio.state,
          audioReason: audio.reason,
          evalStatus,
          onAudio: () => void audio.start(),
          ...(opts.onRun ? { onRun: () => opts.onRun?.() } : {}),
          ...(opts.vim ? { vim: opts.vim } : {}),
          ...(opts.inline ? { inline: opts.inline } : {}),
          ...(opts.unapplied ? { unapplied: opts.unapplied } : {}),
          onHush: () => this.hush(),
          onStop: (slot: string) => this.opts.client.stop(slot),
        }),
      this.el,
    );

    const win = doc.defaultView;
    if (opts.shortcuts !== false && win) {
      this.stopKeyWindow = win;
      this.stopKeyHandler = (event) => {
        if (event.repeat) return;
        const action = stopShortcut(event, isApplePlatform(opts.platform ?? win.navigator.platform));
        if (!action) return;
        event.preventDefault();
        event.stopPropagation();
        if (action === 'stop-all') this.stopAll();
        else this.hush();
      };
      win.addEventListener('keydown', this.stopKeyHandler, true);
    } else {
      this.stopKeyWindow = null;
      this.stopKeyHandler = null;
    }
  }

  hush(): void {
    this.resetPlayback('cutting');
    this.opts.client.hush();
    this.opts.onHush?.();
  }

  /** Stops every slot, including slots that have not appeared in the editor. */
  stopAll(): void {
    this.resetPlayback('draining');
    this.opts.client.stopAll();
    this.opts.onHush?.();
  }

  private play(): void {
    // A per-slot stop can leave an empty scheduler paused. An explicit Play
    // should resume it before evaluating new bindings.
    const sample = this.opts.sample?.() ?? this.tempo?.transport;
    if (sample && !sample.running && clockStatus(this.tempo) !== 'midi lost') this.opts.client.continueTransport();
    this.pauseRequested = null;
    this.pausedCycle = null;
    this.opts.onRun?.();
  }

  private resetPlayback(output: 'draining' | 'cutting'): void {
    this.stopHeld = true;
    this.legacyActive = false;
    this.pauseRequested = null;
    this.pausedCycle = null;
    this.setPlayback('stopped');
    this.setOutput(output);
    this.setPosition(null);
    this.state.cycle = null;
    this.state.beatFlash = false;
    this.state.hidden = true;
    for (const row of this.slots.values()) { row.times = []; row.setLit(false); }
  }

  private togglePause(): void {
    if (this.playback() === 'stopped' || this.opts.pauseReason?.()) return;
    const pause = this.playback() !== 'paused';
    this.pauseRequested = pause;
    if (pause) {
      this.pausedCycle = this.state.cycle ?? this.cycles();
      this.setPlayback('paused');
      this.opts.client.pauseTransport();
      this.opts.onHush?.();
    } else {
      this.opts.client.continueTransport();
      void this.audio.start();
    }
    this.tick();
  }

  /** The distinct transport output states seen, retaining at most 16 transitions. */
  outputHistory(): readonly string[] {
    return [...this.outputStates];
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
        // A successful request permits fresh engine activity after a local stop.
        // Evaluation alone (constants, definitions) is not proof of playback.
        if (env.kind === 'eval-result' && env.body.forms.some((form) => !form.failure)) this.stopHeld = false;
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
    this.tick();
  }

  onLevels(levels: LevelsBody): void {
    const rms = levels.levels[0]?.rms;
    if (rms === undefined || !Number.isFinite(rms)) this.setLevel(null);
    else this.setLevel(rms <= 0 ? -Infinity : 20 * Math.log10(rms));
  }

  onPlaying(events: readonly WirePlaying[]): void {
    if (events.length && !this.stopHeld) {
      this.legacyActive = true;
      if (this.playback() === 'stopped') this.setPlayback('playing');
    }
    this.anchor.observe(events.map((e) => e.time));
    for (const ev of events) {
      const row = this.row(ev.slot);
      if (this.stopHeld) continue;
      row.times.push(this.anchor.local(ev.time));
      if (row.times.length > MAX_SLOT_TIMES) row.times.splice(0, row.times.length - MAX_SLOT_TIMES);
    }
  }

  /** Adds the slots named by diagnostics (`eval-result`, `diag`). */
  onDiagnostics(diags: readonly Diagnostic[]): void {
    for (const d of diags) if (d.slot) this.row(d.slot);
  }

  /** Whether the transport is actively playing. */
  isRunning(): boolean { return this.playback() === 'playing'; }

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
  tick(frameMs?: number): void {
    this.setPauseReason(this.opts.pauseReason?.() ?? '');
    const sample = this.opts.sample?.() ?? this.tempo?.transport ?? null;
    if (sample?.active === false) { this.legacyActive = false; this.stopHeld = false; }
    else if (sample?.active === undefined && sample?.output && sample.output !== 'running') this.legacyActive = false;
    const active = !this.stopHeld && (sample?.active ?? this.legacyActive);
    if (!active) this.setPlayback('stopped');
    else if (this.pauseRequested !== null) {
      if (sample && sample.running === !this.pauseRequested) {
        this.pauseRequested = null;
        this.setPlayback(sample.running ? 'playing' : 'paused');
      }
    } else this.setPlayback(sample?.running === false ? 'paused' : 'playing');
    const output = sample?.output ?? (active ? 'running' : 'idle');
    if (this.output() !== output) this.setOutput(output);
    if (this.outputStates[this.outputStates.length - 1] !== output) {
      this.outputStates.push(output);
      if (this.outputStates.length > 16) this.outputStates.shift();
    }
    this.renderPosition(frameMs, sample);
    const now = this.opts.clock.now();
    for (const row of this.slots.values()) {
      row.times = row.times.filter((t) => t + ACTIVITY_S > now);
      const on = row.times.some((t) => t <= now);
      if (row.lit() !== on) row.setLit(on);
    }
  }

  dispose(): void {
    if (this.evalTimer !== null) this.timers.clear(this.evalTimer);
    this.stopKeyWindow?.removeEventListener('keydown', this.stopKeyHandler as EventListener, true);
    this.disposeRender();
    this.audio.dispose();
    this.disposeRoot();
    this.el.remove();
  }

  private renderPosition(frameMs?: number, transportSample?: TransportSample | null): void {
    if (this.playback() === 'stopped') {
      this.setPosition(null);
      this.state.cycle = null;
      this.state.beatFlash = false;
      this.state.hidden = true;
      this.setBeatFlash(false);
      const position = this.el.querySelector<HTMLElement>('.vact-position');
      if (position) position.dataset.sync = 'hidden';
      return;
    }
    if (this.playback() === 'paused' && this.pauseRequested !== null) {
      this.state.beatFlash = false;
      this.setBeatFlash(false);
      return;
    }
    if (this.opts.audible && this.opts.sample) {
      const audible = this.opts.audible.sample(frameMs ?? performance.now());
      const s = transportSample === undefined ? this.opts.sample() : transportSample;
      let cycle: number | null = null;
      let beatFlash = false;
      if (audible.valid && s && (!audible.epoch || s.epoch === audible.epoch)) {
        const age = audible.time - s.sample_time;
        if (age >= 0 ? age <= 2 : age >= -1) {
          const bpc = s.beats_per_cycle > 0 ? s.beats_per_cycle : DEFAULT_BEATS_PER_CYCLE;
          const sampleCycle = s.cycle[1] === 0 ? 0 : s.cycle[0] / s.cycle[1];
          const bpm = s.bpm > 0 ? s.bpm : DEFAULT_BPM;
          const beatDuration = 60 / bpm;
          cycle = sampleCycle;
          if (s.running && this.playback() === 'playing') cycle += age * bpm / 60 / bpc;
          const sampleBeatPhase = (sampleCycle - Math.floor(sampleCycle)) * bpc;
          const beatElapsed = ((sampleBeatPhase % 1) * beatDuration) + (s.running ? age : 0);
          const beatAge = ((beatElapsed % beatDuration) + beatDuration) % beatDuration;
          beatFlash = this.playback() === 'playing' && s.running && beatAge < 0.08;
          const whole = Math.floor(cycle);
          const beat = Math.floor((cycle - whole) * bpc + 1e-9) + 1;
          this.setPosition({ cycle: whole, beat, beatsPerCycle: bpc });
        }
      }
      this.state.cycle = cycle;
      this.state.beatFlash = beatFlash && cycle !== null;
      this.state.hidden = cycle === null;
      const position = this.el.querySelector<HTMLElement>('.vact-position');
      if (position) {
        position.dataset.sync = this.state.hidden ? 'hidden' : 'visible';
        position.dataset.beatFlash = this.state.beatFlash ? 'on' : 'off';
      }
      if (cycle === null) this.setPosition(null);
      return;
    }
    const held = transportSample ? transportSample.cycle[0] / transportSample.cycle[1] : this.pausedCycle;
    const c = this.playback() === 'paused' ? held ?? this.cycles() : this.cycles();
    if (c === null) return;
    this.pausedCycle = c;
    this.state.cycle = c;
    this.state.hidden = false;
    this.state.beatFlash = false;
    const bpc = this.tempo && this.tempo.beats_per_cycle > 0 ? this.tempo.beats_per_cycle : DEFAULT_BEATS_PER_CYCLE;
    const cycle = Math.floor(c);
    const beat = Math.floor((c - cycle) * bpc + 1e-9) + 1;
    this.setPosition((prev) =>
      prev && prev.cycle === cycle && prev.beat === beat && prev.beatsPerCycle === bpc ? prev : { cycle, beat, beatsPerCycle: bpc },
    );
  }

  private setBeatFlash(on: boolean): void {
    const position = this.el.querySelector<HTMLElement>('.vact-position');
    if (position) position.dataset.beatFlash = on ? 'on' : 'off';
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
