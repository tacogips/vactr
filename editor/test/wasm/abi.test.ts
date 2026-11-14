// @vitest-environment node
//
// The browser Session over the raw wasm ABI (ED-WASM; design 15.1.2 G1,
// G5, G6; command.md "Browser transport (raw wasm ABI, TASK-010)"),
// against the REAL host-wasm artifact (`$VACTR_WASM`). Every test builds
// a fresh instance with `session_init` and drives it on a mock audio clock
// (`session_tick(now)` at chosen times), playing the worklet's install acks
// through `session_inbox`, and reads the framed outbox.

import { describe, expect, it } from 'vitest';
import type { PkgReply, RenderRecord, SessionCheckRecord } from '../../src/protocol/types';
import { FakeProxy } from '../support/fetch';
import {
  ackSampleInstalls,
  consoleLines,
  jsonOf,
  loadVactrWasm,
  putSample,
  recordJson,
  TAG_CONSOLE,
  TAG_PKG,
  TAG_RENDER,
  TAG_SESSION,
  type VactrWasm,
  type WasmRecord,
} from '../support/wasm';
import { buildZip } from '../support/zip';

const FILE = 'main.vact';
const STEP = 0.02;
/** 120 bpm, 4 beats per cycle: one cycle is 2 s. */
const CYCLE = 2;

interface Env {
  v: number;
  seq: number;
  re?: number;
  kind: string;
  body: Record<string, unknown>;
}

interface Rig {
  w: VactrWasm;
  seq: number;
  rev: number;
  now: number;
  /** Every record since `session_init`, in order. */
  all: WasmRecord[];
}

async function start(): Promise<Rig> {
  const w = await loadVactrWasm();
  expect(w.call('session_init', 48000, 0)).toBe(1);
  const rig: Rig = { w, seq: 0, rev: 0, now: 0, all: [] };
  drain(rig);
  return rig;
}

/** The outbox records since the last drain; sample installs are acknowledged. */
function drain(rig: Rig): WasmRecord[] {
  const r = rig.w.drainRecords();
  rig.all.push(...r);
  ackSampleInstalls(rig.w, r);
  return r;
}

const envelopes = (records: readonly WasmRecord[]): Env[] => jsonOf<Env>(records, TAG_SESSION);

/** Sends one client envelope; returns the records it produced. */
function send(rig: Rig, kind: string, body: unknown): WasmRecord[] {
  rig.seq += 1;
  rig.w.callStr('session_apply', JSON.stringify({ v: 1, seq: rig.seq, kind, body }));
  return drain(rig);
}

function subscribe(rig: Rig): void {
  expect(send(rig, 'subscribe', { telemetry: true, levels: false, diagnostics: true })).toEqual([]);
}

/** `eval` of the whole document at the next revision; its `eval-result` body and records. */
function evalDoc(rig: Rig, code: string): { result: Record<string, unknown>; records: WasmRecord[] } {
  rig.rev += 1;
  const records = send(rig, 'eval', { file: FILE, code, doc_revision: rig.rev, edit_epoch: rig.rev });
  const result = envelopes(records).find((e) => e.kind === 'eval-result' && e.re === rig.seq);
  expect(result, `an eval-result answering seq ${rig.seq}`).toBeDefined();
  return { result: (result as Env).body, records };
}

/** One `session_tick` STEP later; its records. */
function tick(rig: Rig): WasmRecord[] {
  rig.now = Math.round((rig.now + STEP) * 1e6) / 1e6;
  rig.w.call('session_tick', rig.now);
  return drain(rig);
}

/** Ticks up to `until` seconds; the records. */
function tickTo(rig: Rig, until: number): WasmRecord[] {
  const out: WasmRecord[] = [];
  while (rig.now < until) out.push(...tick(rig));
  return out;
}

interface Playing {
  slot: string;
  time: number;
  src: { file: string; doc_revision: number; form_gen: number };
}

const playing = (records: readonly WasmRecord[]): Playing[] =>
  envelopes(records)
    .filter((e) => e.kind === 'playing')
    .flatMap((e) => e.body.events as Playing[]);

/** A short silent sample under `bank:0` for each bank. */
function putSamples(rig: Rig, ...banks: string[]): void {
  for (const b of banks) expect(putSample(rig.w, `${b}:0`, new Float32Array(64), 48000, 1)).toBe(1);
}

/** `[time, gain]` of the worklet-bound audio events (`host::wire` `AudioEvent`). */
function sentGains(rig: Rig, records: readonly WasmRecord[]): [number, number][] {
  // `ctl_id` is a stateless control-table lookup (no half state).
  const gain = rig.w.callStr('ctl_id', 'gain');
  expect(gain).toBeGreaterThanOrEqual(0);
  const out: [number, number][] = [];
  for (const r of records) {
    if (r.tag !== 0x01) continue;
    // time f64, slot, gen, inst, voice_hint u32, n_ctl u8, then 7-byte controls.
    const v = new DataView(r.bytes.buffer, r.bytes.byteOffset, r.bytes.byteLength);
    const n = v.getUint8(24);
    for (let i = 0; i < n; i += 1) {
      const at = 25 + 7 * i;
      if (v.getUint16(at, true) === gain && v.getUint8(at + 2) === 0) {
        out.push([v.getFloat64(0, true), v.getFloat32(at + 3, true)]);
      }
    }
  }
  return out;
}

describe('session ABI (real host-wasm artifact)', () => {
  it('session_init returns 1; worklet records are never tagged 0x71-0x73', async () => {
    const rig = await start();
    expect(consoleLines(rig.all).some((l) => l.startsWith('session: ready'))).toBe(true);
    subscribe(rig);
    putSamples(rig, 'bd', 'sd');
    evalDoc(rig, 's [:bd :sd] > d1');
    tickTo(rig, CYCLE + 0.5);
    rig.w.call('session_frame', rig.now);
    drain(rig);
    const worklet = rig.all.filter((r) => r.tag < TAG_CONSOLE);
    expect(worklet.length).toBeGreaterThan(0);
    // Prelude templates, the sample install and audio events all went worklet-bound.
    expect(new Set(worklet.map((r) => r.tag))).toEqual(new Set([0x01, 0x16, 0x18, 0x1a]));
    // Every editor record is JSON of its kind; nothing else uses 0x71-0x73.
    for (const r of rig.all.filter((x) => x.tag >= TAG_SESSION && x.tag <= TAG_PKG)) {
      expect(() => recordJson(r)).not.toThrow();
    }
    expect(rig.all.every((r) => r.tag <= TAG_PKG)).toBe(true);
  }, 60_000);

  it('eval yields an eval-result with sites; playing carries the eval doc_revision', async () => {
    const rig = await start();
    subscribe(rig);
    putSamples(rig, 'bd', 'sd');
    const { result } = evalDoc(rig, 's [:bd :sd] > d1');
    expect(result.file).toBe(FILE);
    expect(result.diagnostics).toEqual([]);
    expect(result.sites).toEqual([]);
    expect(result.doc_revision).toBe(rig.rev);
    const events = playing(tickTo(rig, 2 * CYCLE + 0.1));
    expect(events.map((e) => e.time)).toEqual([0, 1, 2, 3, 4]);
    for (const e of events) {
      expect(e.slot).toBe('d1');
      expect(e.src.file).toBe(FILE);
      expect(e.src.doc_revision).toBe(1);
    }
    // A numeric literal is a tweak site; the next revision's events carry it.
    const next = evalDoc(rig, 's [:bd :sd] > gain 0.8 > d1').result;
    const sites = next.sites as { value: number }[];
    expect(sites).toHaveLength(1);
    expect(sites[0]?.value).toBeCloseTo(0.8);
    const later = playing(tickTo(rig, 4 * CYCLE + 0.1)).filter((e) => e.time >= 3 * CYCLE);
    expect(later.length).toBeGreaterThan(0);
    for (const e of later) expect(e.src.doc_revision).toBe(2);
  });

  it('garbage and non-UTF-8 frames yield protocol-error bad-json', async () => {
    const rig = await start();
    rig.w.callStr('session_apply', 'not json {');
    const errs = envelopes(drain(rig)).filter((e) => e.kind === 'protocol-error');
    expect(errs).toHaveLength(1);
    expect(errs[0]?.body.code).toBe('bad-json');
    rig.w.withBytes(new Uint8Array([0xff, 0xfe, 0x7b]), (p, n) => rig.w.call('session_apply', p, n));
    const bad = envelopes(drain(rig));
    expect(bad).toHaveLength(1);
    expect(bad[0]?.kind).toBe('protocol-error');
    expect(bad[0]?.body.code).toBe('bad-json');
    // A known kind with a bad body is `bad-body`, answered to its seq.
    const reply = envelopes(send(rig, 'eval', { file: FILE }));
    expect(reply).toHaveLength(1);
    expect(reply[0]?.kind).toBe('protocol-error');
  });

  it('session_check reports a type error and never executes the text', async () => {
    const rig = await start();
    subscribe(rig);
    putSamples(rig, 'bd', 'sd');
    const code = 'let n {+ 1 "a"}\ns [:bd :sd] > d1';
    rig.w.callStr('session_check', JSON.stringify({ file: FILE, code }));
    const checks = jsonOf<SessionCheckRecord & { file: string }>(drain(rig), TAG_SESSION);
    expect(checks).toHaveLength(1);
    const rec = checks[0] as SessionCheckRecord & { file: string };
    expect(rec.kind).toBe('check');
    expect(rec.file).toBe(FILE);
    expect(rec.diagnostics.map((d) => [d.code, d.severity])).toContainEqual(['type-mismatch', 'error']);
    // The raw document text (what the editor's WasmCore.check passes) checks identically.
    rig.w.callStr('session_check', code);
    expect(jsonOf<SessionCheckRecord>(drain(rig), TAG_SESSION).map((r) => r.diagnostics)).toEqual([rec.diagnostics]);
    rig.w.callStr('session_check', JSON.stringify({ file: FILE, code: 's [:bd :sd] > d1' }));
    expect(jsonOf<SessionCheckRecord>(drain(rig), TAG_SESSION).map((r) => r.diagnostics)).toEqual([[]]);
    // Nothing was evaluated: no slot plays and no sample was requested.
    const after = tickTo(rig, CYCLE + 0.5);
    expect(playing(after)).toEqual([]);
    expect(rig.all.some((r) => r.tag === 0x1a || r.tag === 0x01)).toBe(false);
  });

  it('the 0x72 program arrives only at the cycle boundary, then uniforms per frame', async () => {
    const rig = await start();
    subscribe(rig);
    tickTo(rig, 0.1);
    const { result, records } = evalDoc(rig, 'osc 20 > rotate 0.5 > out o0');
    expect(result.diagnostics).toEqual([]);
    expect(jsonOf<RenderRecord>(records, TAG_RENDER)).toEqual([]);
    type Program = Extract<RenderRecord, { op: 'program' }>;
    let program: Program | undefined;
    let at = 0;
    while (!program && rig.now < 2 * CYCLE) {
      program = jsonOf<RenderRecord>(tick(rig), TAG_RENDER).find((x): x is Program => x.op === 'program');
      at = rig.now;
    }
    expect(program, 'a program record').toBeDefined();
    const p = program as Program;
    expect(at).toBeGreaterThan(CYCLE - 0.25);
    expect(at).toBeLessThanOrEqual(CYCLE + STEP);
    expect(p.out).toBe(0);
    expect(p.source).toContain('#version 300 es');
    expect(p.assets).toEqual([]);
    const frame = (now: number): Extract<RenderRecord, { op: 'uniforms' }>[] => {
      rig.w.call('session_frame', now);
      return jsonOf<RenderRecord>(drain(rig), TAG_RENDER).filter(
        (x): x is Extract<RenderRecord, { op: 'uniforms' }> => x.op === 'uniforms',
      );
    };
    const u = frame(rig.now);
    expect(u).toHaveLength(1);
    expect(u[0]?.out).toBe(0);
    expect(u[0]?.values).toHaveLength(p.uniform_names.length);
    expect(u[0]?.values.length).toBeGreaterThan(0);
    expect(frame(rig.now + 0.1)).toHaveLength(1);
    // hush drops the plan: no uniforms afterwards.
    send(rig, 'hush', {});
    tick(rig);
    expect(frame(rig.now + 0.2)).toEqual([]);
  });

  describe('package driver', () => {
    const PROXY = 'https://proxy.test';
    const PATH = 'github.com/test/pads';
    const files = [
      { name: 'vactr.toml', data: '[deps]\n' },
      { name: 'a.vact', data: 'let a 1\n' },
    ];
    const proxy = new FakeProxy(PROXY).add({ path: PATH, version: 'v1.0.0', toml: '[deps]\n', files });
    const zipUrl = `${PROXY}/${PATH}/@v/v1.0.0.zip`;

    function supply(rig: Rig, url: string, status: number, body: Uint8Array): void {
      rig.w.withBytes(new TextEncoder().encode(url), (up, un) =>
        rig.w.withBytes(body, (bp, bn) => rig.w.call('pkg_supply', up, un, status, bp, bn)),
      );
      expect(drain(rig)).toEqual([]);
    }

    /** Drives `req` to a non-`need` reply, supplying each need from the proxy; the urls asked and the reply. */
    function drive(rig: Rig, req: unknown): [string[], PkgReply] {
      const needs: string[] = [];
      for (let i = 0; i < 16; i += 1) {
        rig.w.callStr('pkg_resolve', JSON.stringify(req));
        const replies = jsonOf<PkgReply>(drain(rig), TAG_PKG);
        expect(replies).toHaveLength(1);
        const r = replies[0] as PkgReply;
        if (r.status !== 'need') return [needs, r];
        needs.push(r.url);
        const body = proxy.body(r.url);
        supply(rig, r.url, body ? 200 : 404, body ?? new Uint8Array(0));
      }
      throw new Error('the driver never finished');
    }

    it('the need/supply loop ends in done with a lock line; a tampered zip is package-integrity', async () => {
      const rig = await start();
      const [needs, done] = drive(rig, { proxy: PROXY, requirements: { [PATH]: 'v1.0.0' } });
      expect(needs).toEqual([`${PROXY}/${PATH}/@v/list`, `${PROXY}/${PATH}/@v/v1.0.0.toml`, zipUrl]);
      expect(done.status).toBe('done');
      const d = done as Extract<PkgReply, { status: 'done' }>;
      const sha = d.resolved[0]?.sha256 ?? '';
      expect(d.resolved).toEqual([{ path: PATH, version: 'v1.0.0', sha256: sha }]);
      expect(sha).toMatch(/^[0-9a-f]{64}$/);
      const line = d.lock.split('\n').find((l) => l.includes(PATH));
      expect(line).toBeDefined();
      expect(line).toContain('v1.0.0');
      expect(line).toContain(sha);
      // Restore of that lock against a tampered (still valid) zip: only the digest catches it.
      supply(rig, zipUrl, 200, buildZip([{ name: 'vactr.toml', data: '[deps]\n# tampered\n' }]));
      const [again, bad] = drive(rig, { proxy: PROXY, lock: d.lock });
      expect(again).toEqual([]);
      expect(bad).toMatchObject({ status: 'error', code: 'package-integrity' });
    });

    it('a 404 is package-resolve; a malformed request is an error record, not a panic', async () => {
      const rig = await start();
      supply(rig, `${PROXY}/github.com/test/missing/@v/list`, 404, new Uint8Array(0));
      const [, missing] = drive(rig, { proxy: PROXY, requirements: { 'github.com/test/missing': '' } });
      expect(missing).toMatchObject({ status: 'error', code: 'package-resolve' });
      for (const text of ['{"proxy": 1}', 'nope', '{"proxy": "x", "lock": "", "requirements": {}}']) {
        rig.w.callStr('pkg_resolve', text);
        const r = jsonOf<PkgReply>(drain(rig), TAG_PKG);
        expect(r).toHaveLength(1);
        expect(r[0]).toMatchObject({ status: 'error', code: 'bad-body' });
      }
    });
  });

  it('MIDI input: CC 74 = 127 on channel 1 updates the cc cell the next events read', async () => {
    const rig = await start();
    subscribe(rig);
    putSamples(rig, 'bd');
    const { result } = evalDoc(rig, 's [:bd :bd :bd :bd] > gain {cc 74 channel: 1} > d1');
    expect(result.diagnostics).toEqual([]);
    // No input yet: the cell reads 0.
    const before = tickTo(rig, CYCLE - 0.1);
    expect(sentGains(rig, before).length).toBeGreaterThan(0);
    for (const [, g] of sentGains(rig, before)) expect(g).toBe(0);
    const input = rig.now;
    rig.w.withBytes(new Uint8Array([0xb0, 74, 127]), (p, n) => rig.w.call('session_midi_in', p, n, input));
    // Channel 2 and a non-CC status byte are ignored by channel 1's cell.
    rig.w.withBytes(new Uint8Array([0xb1, 74, 5]), (p, n) => rig.w.call('session_midi_in', p, n, rig.now));
    rig.w.withBytes(new Uint8Array([0xf0, 1, 2]), (p, n) => rig.w.call('session_midi_in', p, n, rig.now));
    const after = tickTo(rig, 2 * CYCLE);
    // Events committed before the input (the scheduler's lookahead) keep 0;
    // every event committed after it reads the cell at 127/127.
    const sent = sentGains(rig, after);
    const later = sent.filter(([t]) => t > input + 0.25);
    expect(later.length).toBeGreaterThanOrEqual(2);
    for (const [, g] of later) expect(g).toBeCloseTo(1, 6);
    for (const [t, g] of sent) expect([t, g === 0 || Math.abs(g - 1) < 1e-6]).toEqual([t, true]);
    expect(playing(after).length).toBeGreaterThan(0);
  });
});
