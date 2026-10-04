import { describe, expect, it } from 'vitest';
import { AudibleClock, OutputTimestampCorrelation, ProbeCorrelation } from '../../src/app/clock';
import type { ServerEnvelope } from '../../src/protocol/types';
import { SimulatedTime } from '../support/clock';

const reply = (pageSend: number, engine: number, latency: number | null = 0.02): ServerEnvelope => ({
  v: 1, seq: 1, kind: 'clock-probe', body: {
    page_send: pageSend, engine_receive: engine, engine_send: engine,
    epoch: 'e1', latency_seconds: latency, latency_kind: latency === null ? 'unavailable' : 'measured',
    uncertainty_seconds: 0.001,
  },
});

describe('audible clock', () => {
  it('uses output timestamps without subtracting output latency twice', () => {
    const time = new SimulatedTime(); time.pageMs = 5016; time.contextSeconds = 10.016;
    const correlation = new OutputTimestampCorrelation({ currentTime: 10.02, sampleRate: 48000,
      outputLatency: 0.05, getOutputTimestamp: () => ({ contextTime: 10, performanceTime: 5000 }) });
    expect(correlation.at(5016)).toEqual({ time: 10.016, uncertainty: 128 / 48000, provenance: 'measured' });
  });

  it('falls back to latency estimate, then uncompensated time; suspended contexts invalidate', () => {
    expect(new OutputTimestampCorrelation({ currentTime: 3, sampleRate: 48000, outputLatency: 0.04 }).at(1))
      .toMatchObject({ time: 2.96, provenance: 'estimate' });
    const ctx = { currentTime: 3, sampleRate: 48000 };
    expect(new OutputTimestampCorrelation(ctx).at(1)).toMatchObject({ time: 3, provenance: 'unavailable' });
    ctx.currentTime = 3;
    expect(new OutputTimestampCorrelation({ ...ctx, state: 'suspended' }).at(1)).toBeNull();
  });

  it('caches same-frame samples and predicts presentation with a bounded lead', () => {
    const time = new SimulatedTime();
    const audible = new AudibleClock({ at: (ms) => ({ time: ms / 1000, uncertainty: 0, provenance: 'measured' }) },
      { pageNow: () => time.pageMs, epoch: () => 'e1' });
    const a = audible.sample(1000);
    expect(a).toBe(audible.sample(1000));
    expect(a.targetMs).toBe(1016.7);
    expect(a.valid).toBe(true);
    expect(a.epoch).toBe('e1');
  });

  it('selects the minimum RTT probe and expires stale correlation', async () => {
    const time = new SimulatedTime(); time.pageMs = 1000;
    const probes = new ProbeCorrelation(async (send) => { time.pageMs += 30; return reply(send, 5.01); },
      { pageNow: () => time.pageMs, timers: time.timers });
    probes.start();
    await Promise.resolve();
    expect(probes.at(1030)?.time).toBeCloseTo(5.01 + 0.015 - 0.02, 12);
    expect(probes.at(1030)?.provenance).toBe('measured');
    expect(probes.at(5000)).toBeNull();
    probes.dispose();
  });

  it('counts accepted, over-100ms rejected, and failed probes', async () => {
    const time = new SimulatedTime();
    const pending: { send: number; resolve: (env: ServerEnvelope) => void; reject: (error: Error) => void }[] = [];
    const probes = new ProbeCorrelation((send) => new Promise((resolve, reject) => pending.push({ send, resolve, reject })),
      { pageNow: () => time.pageMs, timers: time.timers });
    probes.start();
    time.advance(30); (pending[0] as typeof pending[number]).resolve(reply(0, 1)); await Promise.resolve();
    time.advance(970);
    time.pageMs = 1010; (pending[1] as typeof pending[number]).resolve(reply(1000, 2)); await Promise.resolve();
    expect(probes.at(1100)?.time).toBeCloseTo(2 + 0.095 - 0.02, 12);
    time.advance(990);
    time.pageMs = 2150; (pending[2] as typeof pending[number]).resolve(reply(2000, 3)); await Promise.resolve();
    time.advance(850); (pending[3] as typeof pending[number]).reject(new Error('busy')); await Promise.resolve();
    expect(probes.stats).toEqual({ samples: 2, rejected: 1, failures: 1 });
    probes.dispose();
  });

  it('starts probes immediately, repeats every second, and stops after dispose', async () => {
    const time = new SimulatedTime();
    let count = 0;
    const probe = new ProbeCorrelation(async (send) => { count += 1; return reply(send, 0.1); },
      { pageNow: () => time.pageMs, timers: time.timers });
    probe.start();
    await Promise.resolve();
    expect(count).toBe(1);
    time.advance(1000); await Promise.resolve();
    expect(count).toBe(2);
    probe.dispose();
    time.advance(5000); await Promise.resolve();
    expect(count).toBe(2);
  });
});
