// Playing-step highlighting (design 14.4, 15.1.5 "Highlighting"; TASK-010
// criterion 1 and the highlight half of criterion 10).
//
// A `playing` event with `src` becomes an entry active over
// `[time, time + dur_seconds)` on the tier clock, where `dur_seconds` uses
// the latest `tempo`. `tick()` (per animation frame; per test step in
// tests) decorates the active entries' spans, each mapped from the event's
// OWN `doc_revision` to the current text through the revision history, and
// forgets expired ones. An event without `src`, of another file, or whose
// span no longer maps (edited, or older than the history) is dropped.

import type { AudibleClock, Clock } from '../app/clock';
import type { Ratio, Span, TempoBody, WirePlaying } from '../protocol/types';
import type { Range16 } from './history';

/** Used until the first `tempo` arrives (the runtime defaults). */
export const DEFAULT_BPM = 120;
export const DEFAULT_BEATS_PER_CYCLE = 4;

/** Entries kept at most; the oldest are dropped first. */
export const MAX_ENTRIES = 4096;

export const PLAYING_CLASS = 'vact-playing';

/**
 * Seconds of a `playing` duration under `tempo`. The wire publishes `dur`
 * in BEATS (`src/session/publish.rs` `dur_beats`, a 1/960 grid), not in
 * cycles, so `beats_per_cycle` plays no part here.
 */
export function durSeconds(dur: Ratio, tempo: Pick<TempoBody, 'bpm' | 'beats_per_cycle'> | null): number {
  const bpm = tempo && tempo.bpm > 0 ? tempo.bpm : DEFAULT_BPM;
  const beats = dur[1] === 0 ? 0 : dur[0] / dur[1];
  return (beats * 60) / bpm;
}

/**
 * Maps event times to the tier clock. The browser tier's event times ARE
 * `AudioContext.currentTime` (`audio`: identity). The native tier's are
 * host times; they are anchored at the receipt of the first batch
 * (`receipt`: offset = `clock.now()` minus the smallest time of that
 * batch). That anchoring is best effort and ignores transport latency.
 */
export class TimeAnchor {
  private readonly clock: Clock;
  private readonly mode: 'audio' | 'receipt';
  private offset: number | null;

  constructor(clock: Clock, mode: 'audio' | 'receipt') {
    this.clock = clock;
    this.mode = mode;
    this.offset = mode === 'audio' ? 0 : null;
  }

  /** Call once per received batch before `local`. */
  observe(times: readonly number[]): void {
    if (this.offset !== null || times.length === 0) return;
    this.offset = this.clock.now() - Math.min(...times);
  }

  /** A host time on the tier clock. */
  local(time: number): number {
    return time + (this.offset ?? 0);
  }

  get anchored(): boolean {
    return this.mode === 'audio' || this.offset !== null;
  }
}

interface Entry {
  id?: number;
  start: number;
  end: number;
  span: Span;
  rev: number;
  epoch: string | null;
  mapped?: { at: number; range: Range16 | null };
}

export interface HighlightOptions {
  clock: Clock;
  file: string;
  /** A wire span of `rev` to the current UTF-16 range (`CodeApi.mapWireSpan`). */
  map: (span: Span, rev: number) => Range16 | null;
  /** The latest `tempo` (read at event receipt). */
  tempo: () => TempoBody | null;
  anchor?: TimeAnchor;
  audible?: AudibleClock;
  epoch?: () => string | null;
  pin?: (owner: string, rev: number) => boolean;
  unpin?: (owner: string) => void;
  revision?: () => number;
  /** Receives the active ranges whenever they change. */
  apply?: (ranges: Range16[]) => void;
}

export class HighlightScheduler {
  readonly stats = { received: 0, accepted: 0, overflow: 0, horizonDrops: 0, epochDrops: 0, unmapped: 0,
    confirmedSkipped: 0, aheadAccepted: 0, retracted: 0 };
  private readonly opts: HighlightOptions;
  private readonly anchor: TimeAnchor;
  private entries: Entry[] = [];
  private readonly lastAccepted = new Map<number, number>();
  private readonly knownIds = new Set<number>();
  private activeRanges: Range16[] = [];
  private activeKey = '';
  private readonly acceptListeners = new Set<(e: { time: number; end: number; from: number; to: number; epoch: string | null; id?: number }) => void>();
  private readonly retractListeners = new Set<(id: number) => void>();

  constructor(opts: HighlightOptions) {
    this.opts = opts;
    this.anchor = opts.anchor ?? new TimeAnchor(opts.clock, 'audio');
  }

  /** Stores the highlightable events of one `playing` batch. */
  onPlaying(events: readonly WirePlaying[], extra: { ahead?: readonly WirePlaying[]; retract?: readonly number[] } = {}): void {
    const ahead = extra.ahead ?? [];
    const retract = extra.retract ?? [];
    this.stats.received += events.length + ahead.length;
    for (const id of retract) {
      const before = this.entries.length;
      this.entries = this.entries.filter((entry) => entry.id !== id);
      if (this.entries.length !== before) {
        this.stats.retracted += 1;
        for (const cb of this.retractListeners) cb(id);
      }
    }
    this.anchor.observe([...events, ...ahead].map((e) => e.time));
    const tempo = this.opts.tempo();
    const accept = (ev: WirePlaying, isAhead: boolean): void => {
      if (!isAhead && ev.id !== undefined && this.knownIds.has(ev.id)) {
        this.stats.confirmedSkipped += 1;
        return;
      }
      const src = ev.src;
      if (!src || src.file !== this.opts.file) return;
      const start = this.opts.audible ? ev.time : this.anchor.local(ev.time);
      const end = this.opts.audible ? (ev.end_time ?? start + durSeconds(ev.dur, tempo)) : start + durSeconds(ev.dur, tempo);
      const epoch = ev.epoch ?? null;
      if (this.opts.audible) {
        if (epoch !== null && epoch !== (this.opts.epoch?.() ?? null)) { this.stats.epochDrops += 1; return; }
        const t = this.opts.audible.now();
        if (start > t + 2) { this.stats.horizonDrops += 1; return; }
      }
      const mapped = this.opts.audible || this.acceptListeners.size > 0
        ? this.opts.map(src.span, src.doc_revision)
        : null;
      if (this.opts.audible && !mapped) { this.stats.unmapped += 1; return; }
      this.opts.pin?.(`playing:${src.doc_revision}`, src.doc_revision);
      this.lastAccepted.set(src.doc_revision, this.opts.audible?.now() ?? this.opts.clock.now());
      this.entries.push({ ...(ev.id !== undefined ? { id: ev.id } : {}), start, end, span: src.span, rev: src.doc_revision, epoch,
        ...(mapped ? { mapped: { at: this.opts.revision?.() ?? -1, range: mapped } } : {}) });
      if (ev.id !== undefined) {
        this.knownIds.add(ev.id);
        while (this.knownIds.size > MAX_ENTRIES) this.knownIds.delete(this.knownIds.values().next().value as number);
      }
      this.stats.accepted += 1;
      if (ev.id !== undefined && isAhead) this.stats.aheadAccepted += 1;
      if (mapped) for (const cb of this.acceptListeners) cb({ time: start, end, from: mapped.from, to: mapped.to, epoch,
        ...(ev.id !== undefined ? { id: ev.id } : {}) });
    };
    for (const ev of events) accept(ev, false);
    for (const ev of ahead) accept(ev, true);
    if (this.entries.length > MAX_ENTRIES) {
      const overflow = this.entries.length - MAX_ENTRIES;
      this.stats.overflow += overflow;
      this.entries.splice(0, overflow);
    }
  }

  /** Recomputes the active set at the frame's audible presentation time. */
  tick(frameMs?: number): Range16[] {
    const sample = this.opts.audible?.sample(frameMs ?? performance.now());
    if (sample && !sample.valid) {
      this.activeKey = '';
      this.activeRanges = [];
      this.opts.apply?.([]);
      return this.activeRanges;
    }
    const now = sample?.time ?? this.opts.clock.now();
    const keep: Entry[] = [];
    const ranges: Range16[] = [];
    for (const e of this.entries) {
      if (this.opts.audible && e.epoch !== null && e.epoch !== (this.opts.epoch?.() ?? null)) {
        this.stats.epochDrops += 1;
        continue;
      }
      if (this.opts.audible && e.start > now + 2) { this.stats.horizonDrops += 1; continue; }
      if (e.end <= now) continue;
      if (e.start > now) {
        keep.push(e);
        continue;
      }
      const revision = this.opts.revision?.();
      let r = e.mapped && revision !== undefined && e.mapped.at === revision ? e.mapped.range : undefined;
      if (r === undefined) {
        r = this.opts.map(e.span, e.rev);
        if (this.opts.revision) e.mapped = { at: this.opts.revision(), range: r };
      }
      if (!r) { this.stats.unmapped += 1; continue; }
      keep.push(e);
      ranges.push(r);
    }
    this.entries = keep;
    for (const [rev, at] of this.lastAccepted) {
      if (!keep.some((entry) => entry.rev === rev) && now - at > 2) {
        this.opts.unpin?.(`playing:${rev}`);
        this.lastAccepted.delete(rev);
      }
    }
    ranges.sort((a, b) => a.from - b.from || a.to - b.to);
    const unique = ranges.filter((r, i) => i === 0 || r.from !== ranges[i - 1]?.from || r.to !== ranges[i - 1]?.to);
    const key = unique.map((r) => `${r.from}:${r.to}`).join(',');
    if (key !== this.activeKey) {
      this.activeKey = key;
      this.activeRanges = unique;
      this.opts.apply?.(unique);
    }
    return this.activeRanges;
  }

  /** The ranges decorated by the last `tick`. */
  active(): Range16[] {
    return this.activeRanges;
  }

  onAccept(cb: (e: { time: number; end: number; from: number; to: number; epoch: string | null; id?: number }) => void): () => void {
    this.acceptListeners.add(cb);
    return () => { this.acceptListeners.delete(cb); };
  }

  onRetract(cb: (id: number) => void): () => void {
    this.retractListeners.add(cb);
    return () => { this.retractListeners.delete(cb); };
  }

  /** Pending plus active entries. */
  get size(): number {
    return this.entries.length;
  }

  /** Forgets every entry (hush); the next `tick` clears the decorations. */
  clear(): void {
    this.entries = [];
    this.knownIds.clear();
    for (const rev of this.lastAccepted.keys()) this.opts.unpin?.(`playing:${rev}`);
    this.lastAccepted.clear();
  }
}
