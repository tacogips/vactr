// Processing and audible time sources. Processing time remains the scheduling
// clock; display consumers use AudibleClock to account for output latency.

import { defaultTimers, type Timers } from '../protocol/document';
import type { Client } from '../protocol/client';
import type { ClockProbeReply, ServerEnvelope } from '../protocol/types';

export interface Clock {
  /** Seconds. */
  now(): number;
}

export type Provenance = 'measured' | 'estimate' | 'unavailable';
export interface CorrelatedTime { time: number; uncertainty: number; provenance: Provenance }
export interface ClockCorrelation { at(pageMs: number): CorrelatedTime | null }
export interface AudibleSample {
  time: number;
  targetMs: number;
  epoch: string | null;
  provenance: Provenance;
  uncertainty: number;
  valid: boolean;
}

export interface OutputContextLike {
  currentTime: number;
  sampleRate: number;
  outputLatency?: number;
  state?: AudioContextState;
  getOutputTimestamp?(): { contextTime?: number; performanceTime?: number };
}

export class OutputTimestampCorrelation implements ClockCorrelation {
  constructor(private readonly ctx: OutputContextLike) {}

  at(pageMs: number): CorrelatedTime | null {
    const ctx = this.ctx;
    if (ctx.state === 'suspended' || ctx.state === 'closed') return null;
    const stamp = ctx.getOutputTimestamp?.();
    if (stamp && Number.isFinite(stamp.contextTime) && (stamp.contextTime as number) > 0 &&
      Number.isFinite(stamp.performanceTime) && (stamp.performanceTime as number) > 0 &&
      Math.abs(pageMs - (stamp.performanceTime as number)) <= 1000 &&
      (stamp.contextTime as number) <= ctx.currentTime) {
      return { time: (stamp.contextTime as number) + (pageMs - (stamp.performanceTime as number)) / 1000,
        uncertainty: 128 / ctx.sampleRate, provenance: 'measured' };
    }
    if (Number.isFinite(ctx.outputLatency) && (ctx.outputLatency as number) > 0)
      return { time: ctx.currentTime - (ctx.outputLatency as number), uncertainty: (ctx.outputLatency as number) / 2, provenance: 'estimate' };
    return { time: ctx.currentTime, uncertainty: 0, provenance: 'unavailable' };
  }
}

interface ProbeSample { pageReceive: number; reply: ClockProbeReply; rtt: number }
export interface ProbeCorrelationOptions { timers?: Timers; pageNow?: () => number }

export class ProbeCorrelation implements ClockCorrelation {
  readonly stats = { samples: 0, rejected: 0, failures: 0 };
  private readonly timers: Timers;
  private readonly pageNow: () => number;
  private samples: ProbeSample[] = [];
  private timer: unknown = null;
  private running = false;

  constructor(private readonly probe: (pageSend: number) => Promise<ServerEnvelope>, opts: ProbeCorrelationOptions = {}) {
    this.timers = opts.timers ?? defaultTimers;
    this.pageNow = opts.pageNow ?? (() => performance.now());
  }

  start(): void {
    if (this.running) return;
    this.running = true;
    void this.send();
    this.schedule();
  }

  at(pageMs: number): CorrelatedTime | null {
    const fresh = this.samples.filter((s) => pageMs - s.pageReceive < 3000 && pageMs >= s.pageReceive);
    if (!fresh.length) return null;
    let best = fresh[0] as ProbeSample;
    for (const sample of fresh) if (sample.rtt < best.rtt) best = sample;
    const b = best.reply;
    const offset = (b.engine_receive + b.engine_send) / 2 - (b.page_send + best.rtt / 2) / 1000;
    return { time: pageMs / 1000 + offset - (b.latency_seconds ?? 0),
      uncertainty: best.rtt / 2000 + (b.uncertainty_seconds ?? 0), provenance: b.latency_kind };
  }

  dispose(): void {
    this.running = false;
    if (this.timer !== null) this.timers.clear(this.timer);
    this.timer = null;
  }

  private schedule(): void {
    this.timer = this.timers.set(() => {
      this.timer = null;
      if (!this.running) return;
      void this.send();
      this.schedule();
    }, 1000);
  }

  private async send(): Promise<void> {
    const pageSendMs = this.pageNow();
    try {
      const env = await this.probe(pageSendMs);
      if (!this.running || env.kind !== 'clock-probe') return;
      const reply = env.body;
      const pageReceive = this.pageNow();
      const rtt = pageReceive - reply.page_send - (reply.engine_send - reply.engine_receive) * 1000;
      if (rtt < 0 || rtt > 100 || pageReceive - reply.page_send >= 3000) {
        this.stats.rejected += 1;
        return;
      }
      if (this.samples.length && this.samples[0]?.reply.epoch !== reply.epoch) this.samples = [];
      this.samples.push({ pageReceive, reply, rtt });
      if (this.samples.length > 8) this.samples.shift();
      this.stats.samples += 1;
    } catch {
      if (this.running) this.stats.failures += 1;
    }
  }
}

export class AudibleClock implements Clock {
  private lead = 16.7;
  private lastFrame: number | null = null;
  private cached: AudibleSample | null = null;

  constructor(private readonly correlation: ClockCorrelation, private readonly opts: {
    pageNow?: () => number; epoch?: () => string | null;
  } = {}) {}

  now(): number { return this.correlation.at((this.opts.pageNow ?? (() => performance.now()))())?.time ?? 0; }

  sample(frameMs: number): AudibleSample {
    if (this.cached?.targetMs === frameMs + this.lead && this.cachedFrame === frameMs) return this.cached;
    if (this.lastFrame !== null && frameMs !== this.lastFrame) {
      const delta = frameMs - this.lastFrame;
      if (delta > 0 && Number.isFinite(delta)) this.lead = Math.max(8.3, Math.min(33.4, this.lead * 0.9 + delta * 0.1));
    }
    this.lastFrame = frameMs;
    const targetMs = frameMs + this.lead;
    const result = this.correlation.at(targetMs);
    this.cachedFrame = frameMs;
    this.cached = { time: result?.time ?? 0, targetMs, epoch: this.opts.epoch?.() ?? null,
      provenance: result?.provenance ?? 'unavailable', uncertainty: result?.uncertainty ?? 0, valid: result !== null };
    return this.cached;
  }

  private cachedFrame: number | null = null;
}

export function uncorrelated(clock: Clock): AudibleClock {
  return new AudibleClock({ at: () => ({ time: clock.now(), uncertainty: 0, provenance: 'unavailable' }) });
}

export function audibleFor(tier: 'browser' | 'native', src: {
  ctx?: OutputContextLike; client?: Pick<Client, 'clockProbe'>; timers?: Timers; epoch?: () => string | null;
}): { audible: AudibleClock; start(): void; dispose(): void } {
  const correlation: ClockCorrelation = tier === 'browser'
    ? new OutputTimestampCorrelation(src.ctx as OutputContextLike)
    : new ProbeCorrelation((pageSend) => (src.client as Pick<Client, 'clockProbe'>).clockProbe(pageSend), { timers: src.timers });
  const audible = new AudibleClock(correlation, { ...(src.epoch ? { epoch: src.epoch } : {}) });
  return { audible, start: () => { if (correlation instanceof ProbeCorrelation) correlation.start(); },
    dispose: () => { if (correlation instanceof ProbeCorrelation) correlation.dispose(); } };
}

export class AudioClock implements Clock {
  private readonly ctx: { readonly currentTime: number };

  constructor(ctx: { readonly currentTime: number }) {
    this.ctx = ctx;
  }

  now(): number {
    return this.ctx.currentTime;
  }
}

/** `performance.now()` in seconds (the native tier). */
export class PageClock implements Clock {
  now(): number {
    return performance.now() / 1000;
  }
}
