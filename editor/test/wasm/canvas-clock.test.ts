// @vitest-environment node
// CE-TELEMETRY: the real raw host-wasm ABI, not fabricated clock messages.
import { describe, expect, it } from 'vitest';
import { decodeServer } from '../../src/protocol/envelope';
import type { PlayingBody, TempoBody, TransportSample, WirePlaying } from '../../src/protocol/types';
import {
  ackSampleInstalls,
  jsonOf,
  loadVactrWasm,
  putSample,
  TAG_SESSION,
  type VactrWasm,
  type WasmRecord,
} from '../support/wasm';

interface Envelope {
  v: number;
  seq: number;
  re?: number;
  kind: string;
  body: Record<string, unknown>;
}

interface Rig {
  w: VactrWasm;
  seq: number;
}

function drain(rig: Rig): WasmRecord[] {
  const records = rig.w.drainRecords();
  for (const record of records.filter((r) => r.tag === TAG_SESSION)) {
    const text = new TextDecoder().decode(record.bytes);
    const envelope = JSON.parse(text) as Envelope;
    if (envelope.kind === 'tempo' || envelope.kind === 'playing') {
      // Decode original ABI bytes with the accepted frontend wire contract.
      expect(decodeServer(text), text).toMatchObject({ ok: true });
    }
  }
  ackSampleInstalls(rig.w, records);
  return records;
}

function send(rig: Rig, kind: string, body: unknown): WasmRecord[] {
  rig.seq += 1;
  rig.w.callStr('session_apply', JSON.stringify({ v: 1, seq: rig.seq, kind, body }));
  return drain(rig);
}

function subscribe(rig: Rig, telemetry = true): void {
  send(rig, 'subscribe', { telemetry, levels: false, diagnostics: false });
}

async function start(telemetry = true): Promise<Rig> {
  const w = await loadVactrWasm();
  expect(w.call('session_init', 48000, 0)).toBe(1);
  const rig = { w, seq: 0 };
  drain(rig);
  subscribe(rig, telemetry);
  return rig;
}

function tick(rig: Rig, now: number): WasmRecord[] {
  rig.w.call('session_tick', now);
  return drain(rig);
}

function bodies<T>(records: readonly WasmRecord[], kind: string): T[] {
  const envelopes = jsonOf<Envelope>(records, TAG_SESSION).filter((e) => e.kind === kind);
  for (const e of envelopes) expect(e.v).toBe(1);
  return envelopes.map((e) => e.body as T);
}

function samples(records: readonly WasmRecord[]): TransportSample[] {
  return bodies<TempoBody>(records, 'tempo').flatMap((b) => b.transport ? [b.transport] : []);
}

function events(records: readonly WasmRecord[]): WirePlaying[] {
  return bodies<PlayingBody>(records, 'playing').flatMap((b) => b.events);
}

function evaluate(rig: Rig, file: string, code: string, revision: number): void {
  const records = send(rig, 'eval', { file, code, doc_revision: revision, edit_epoch: revision });
  const replies = jsonOf<Envelope>(records, TAG_SESSION).filter(
    (e) => e.kind === 'eval-result' && e.re === rig.seq,
  );
  expect(replies).toHaveLength(1);
  expect(replies[0]?.body.diagnostics).toEqual([]);
  expect(replies[0]?.body.doc_revision).toBe(revision);
}

function onlySample(records: readonly WasmRecord[]): TransportSample {
  const out = samples(records);
  expect(out).toHaveLength(1);
  return out[0] as TransportSample;
}

describe('canvas transport telemetry (real host-wasm artifact)', () => {
  it('refreshes transport without tempo changes, paired with host time, at no more than 20 Hz', async () => {
    const rig = await start();
    evaluate(rig, 'main.vact', 'use-bpm 120', 17);
    const out: TransportSample[] = [];
    const tickTimes: number[] = [];
    for (let ms = 0; ms < 1000; ms += 1) {
      const hostNow = ms / 1000;
      for (const sample of samples(tick(rig, hostNow))) {
        const gridPeriod = 1 / (960 * 0.5);
        expect(hostNow - sample.sample_time).toBeGreaterThanOrEqual(0);
        expect(hostNow - sample.sample_time).toBeLessThan(gridPeriod + 1e-9);
        out.push(sample);
        tickTimes.push(hostNow);
      }
    }
    expect(out.length).toBeGreaterThanOrEqual(18);
    expect(out.length).toBeLessThanOrEqual(20);
    expect(new Set(out.map((s) => s.epoch)).size).toBe(1);
    for (let i = 0; i < out.length; i += 1) {
      const sample = out[i] as TransportSample;
      expect(sample.epoch.length).toBeGreaterThan(0);
      expect(sample.bpm).toBe(120);
      expect(sample.beats_per_cycle).toBe(4);
      expect(sample.running).toBe(true);
      expect(sample.cycle[1]).toBeGreaterThan(0);
      // Runtime position is quantized to its cycle grid and paired with its host time.
      const cycleTime = (sample.cycle[0] / sample.cycle[1]) / 0.5;
      expect(Math.abs(cycleTime - sample.sample_time)).toBeLessThanOrEqual(1e-9);
      expect(sample.latency_seconds).toBeNull();
      expect(sample.latency_kind).toBe('unavailable');
      expect(sample.uncertainty_seconds).toBeNull();
      if (i > 0) {
        const spacing = (tickTimes[i] as number) - (tickTimes[i - 1] as number);
        expect(spacing).toBeGreaterThanOrEqual(0.05 - 1e-12);
      }
    }
  });

  it('retains scheduled end times and original source revisions when a separate file changes BPM', async () => {
    const rig = await start();
    expect(putSample(rig.w, 'bd:0', new Float32Array(64), 48000, 1)).toBe(1);
    const code = 'use-bpm 120\ns [:bd] > d1';
    evaluate(rig, 'main.vact', code, 41);
    const firstRecords = tick(rig, 0);
    const before = events(firstRecords);
    expect(before.length).toBeGreaterThan(0);
    const epoch = onlySample(firstRecords).epoch;
    for (const event of before) {
      expect(event.end_time).toBeCloseTo(event.time + 2, 6);
      expect(event.epoch).toBe(epoch);
    }
    // The existing pattern is not evaluated again or relabelled to this document.
    evaluate(rig, 'tempo.vact', 'use-bpm 240', 99);
    const after: WirePlaying[] = [];
    const refreshed: TransportSample[] = [];
    for (let ms = 50; ms <= 3100; ms += 50) {
      const records = tick(rig, ms / 1000);
      after.push(...events(records));
      refreshed.push(...samples(records));
    }
    expect(refreshed.some((s) => s.bpm === 240)).toBe(true);
    expect(after.length).toBeGreaterThan(0);
    for (const event of [...before, ...after]) {
      expect(event.src?.file).toBe('main.vact');
      expect(event.src?.doc_revision).toBe(41);
      expect(event.src?.span.start).toBeGreaterThanOrEqual(0);
      expect(event.src?.span.end).toBeLessThanOrEqual(new TextEncoder().encode(code).length);
      expect(event.end_time).toBeGreaterThanOrEqual(event.time);
    }
    for (const event of after) expect(event.end_time).toBeCloseTo(event.time + 1, 6);
    // Previously scheduled highlights keep their absolute end, despite the new BPM.
    for (const event of before) expect(event.end_time).toBeCloseTo(event.time + 2, 6);
  });

  it('invalid host samples are ignored without consuming cadence or contaminating the epoch', async () => {
    const rig = await start();
    const first = onlySample(tick(rig, 0));
    for (const invalid of [Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY, -1]) {
      expect(tick(rig, invalid)).toEqual([]);
    }
    expect(samples(tick(rig, 0.049))).toEqual([]);
    const next = onlySample(tick(rig, 0.05));
    expect(next.epoch).toBe(first.epoch);
    expect(next.sample_time).toBeCloseTo(0.05, 9);
    expect(next.cycle[0] / next.cycle[1]).toBeCloseTo(0.025, 2);
  });

  it('declares a new epoch on host clock rollback and on reinitializing the same Wasm instance', async () => {
    const rig = await start();
    const first = onlySample(tick(rig, 1));
    const rewind = onlySample(tick(rig, 0.25));
    expect(rewind.epoch).not.toBe(first.epoch);
    expect(rewind.sample_time).toBe(0.25);
    const resumed = onlySample(tick(rig, 0.301));
    expect(resumed.epoch).toBe(rewind.epoch);
    expect(resumed.sample_time).toBe(0.301);
    expect(rig.w.call('session_init', 48000, 0)).toBe(1);
    drain(rig);
    subscribe(rig);
    const restarted = onlySample(tick(rig, 0));
    expect(restarted.epoch).not.toBe(first.epoch);
    expect(restarted.epoch).not.toBe(rewind.epoch);
    expect(restarted.sample_time).toBe(0);
    expect(restarted.cycle).toEqual([0, 1]);
  });

  it('invalidates zero-position MIDI restarts before playing even inside snapshot cadence', async () => {
    const rig = await start();
    expect(putSample(rig.w, 'bd:0', new Float32Array(64), 48000, 1)).toBe(1);
    evaluate(rig, 'main.vact', 'use-clock :midi\ns [:bd] > d1', 41);
    const first = onlySample(tick(rig, 0));
    expect(first.cycle).toEqual([0, 1]);
    rig.w.withBytes(new Uint8Array([0xfa]), (p, n) => rig.w.call('session_midi_in', p, n, 0));
    const immediate = tick(rig, 0.01);
    expect(samples(immediate)).toEqual([]);
    const playing = events(immediate);
    expect(playing.length).toBeGreaterThan(0);
    const epoch = playing[0]?.epoch;
    expect(epoch).not.toBe(first.epoch);
    for (const event of playing) {
      expect(event.epoch).toBe(epoch);
      expect(event.src?.file).toBe('main.vact');
      expect(event.src?.doc_revision).toBe(41);
      expect(event.end_time).toBeCloseTo(event.time + 2, 6);
    }
    expect(onlySample(tick(rig, 0.05)).epoch).toBe(epoch);
  });

  it('invalidates between-sample MIDI restarts despite nondecreasing sampled position', async () => {
    const rig = await start();
    expect(putSample(rig.w, 'bd:0', new Float32Array(64), 48000, 1)).toBe(1);
    evaluate(rig, 'main.vact', 'use-clock :midi\ns [:bd] > d1', 42);
    const first = onlySample(tick(rig, 0));
    for (const time of [0.02, 0.04]) {
      rig.w.withBytes(new Uint8Array([0xfa]), (p, n) => rig.w.call('session_midi_in', p, n, time));
    }
    const out = tick(rig, 0.1);
    const next = onlySample(out);
    expect(next.cycle[0] / next.cycle[1]).toBeGreaterThanOrEqual(first.cycle[0] / first.cycle[1]);
    expect(next.epoch).not.toBe(first.epoch);
    const playing = events(out);
    expect(playing.length).toBeGreaterThan(0);
    for (const event of playing) {
      expect(event.epoch).toBe(next.epoch);
      expect(event.src?.doc_revision).toBe(42);
      expect(event.end_time).toBeCloseTo(event.time + 2, 6);
    }
  });

  it('routes transport snapshots only to telemetry subscribers', async () => {
    const rig = await start(false);
    expect(samples(tick(rig, 0))).toEqual([]);
    expect(samples(tick(rig, 0.1))).toEqual([]);
    subscribe(rig);
    expect(samples(tick(rig, 0.2))).toHaveLength(1);
    subscribe(rig, false);
    expect(samples(tick(rig, 0.3))).toEqual([]);
  });
});
