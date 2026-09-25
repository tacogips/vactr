// @vitest-environment node
//
// TASK-010 criterion tests against the REAL host-wasm artifact
// (`$VACTROL_WASM`; design 15.1.12, vactrol-core.md TASK-010 criteria 1, 2,
// 3 (sites) and 11). Every test builds a fresh browser Session with
// `session_init` (G1) and drives it on a mock audio clock (`session_tick(now)`
// at chosen times). The worklet is played by answering its install, cell and
// slot-control records through `session_inbox`, so nothing is left
// unacknowledged. The editor-side consumers are the real ones:
// `HighlightScheduler` + `DocumentSync` (ED-CODE) for criterion 1 and
// `GlRenderHost` over `RecordingGL` (ED-VISUAL) for criterion 11.

import { Text } from '@codemirror/state';
import { describe, expect, it } from 'vitest';
import { HighlightScheduler } from '../../src/code/highlight';
import { DocumentSync } from '../../src/code/sync';
import { Client } from '../../src/protocol/client';
import type {
  BindingsBody,
  Diagnostic,
  EvalResultBody,
  RenderRecord,
  SessionCheckRecord,
  StaleBindingBody,
  TempoBody,
  WirePlaying,
  WireSite,
} from '../../src/protocol/types';
import { GlRenderHost, type HostDiagnostic } from '../../src/visual/render-host';
import { MockClock } from '../support/clock';
import { FakeContext2D } from '../support/canvas';
import { RecordingGL, type FakeProgram } from '../support/gl';
import { RecordingTransport } from '../support/recording';
import {
  ackSampleInstalls,
  jsonOf,
  loadVactrolWasm,
  putSample,
  TAG_RENDER,
  TAG_SESSION,
  type VactrolWasm,
  type WasmRecord,
} from '../support/wasm';

const FILE = 'main.vact';
const STEP = 0.02;
/** 120 bpm, 4 beats per cycle: one cycle is 2 s. */
const CYCLE = 2;
/** `SchedConfig::default().lookahead` (src/sched/runtime.rs). */
const LOOKAHEAD = 0.12;

const enc = new TextEncoder();

interface Env {
  v: number;
  seq: number;
  re?: number;
  kind: string;
  body: Record<string, unknown>;
}

interface Rig {
  w: VactrolWasm;
  seq: number;
  rev: number;
  epoch: number;
  now: number;
}

// Worklet-bound records the rig acknowledges (`host::wire`): slot control,
// cell init and cell batch, answered with their acks.
const TAG_SLOT_CONTROL = 0x10;
const TAG_CELL_INIT = 0x11;
const TAG_CELL_BATCH = 0x12;
const TAG_SLOT_CONTROL_ACK = 0x40;
const TAG_CELL_INIT_ACK = 0x41;
const TAG_CELL_BATCH_ACK = 0x42;

/** Answers the slot-control and cell records of `records` like the worklet. */
function ackControls(w: VactrolWasm, records: readonly WasmRecord[]): void {
  const replies: [number, number[]][] = [];
  for (const r of records) {
    const v = new DataView(r.bytes.buffer, r.bytes.byteOffset, r.bytes.byteLength);
    if (r.tag === TAG_SLOT_CONTROL) replies.push([TAG_SLOT_CONTROL_ACK, [v.getUint32(0, true), v.getUint32(4, true)]]);
    if (r.tag === TAG_CELL_INIT) replies.push([TAG_CELL_INIT_ACK, [v.getUint32(0, true), v.getUint32(4, true)]]);
    if (r.tag === TAG_CELL_BATCH) replies.push([TAG_CELL_BATCH_ACK, [v.getUint32(0, true)]]);
  }
  if (replies.length === 0) return;
  const size = replies.reduce((n, [, words]) => n + 5 + 4 * words.length, 0);
  const bytes = new Uint8Array(size);
  const out = new DataView(bytes.buffer);
  let at = 0;
  for (const [tag, words] of replies) {
    out.setUint32(at, 1 + 4 * words.length, true);
    out.setUint8(at + 4, tag);
    words.forEach((word, i) => out.setUint32(at + 5 + 4 * i, word, true));
    at += 5 + 4 * words.length;
  }
  w.withBytes(bytes, (p, n) => w.call('session_inbox', p, n));
}

async function start(): Promise<Rig> {
  const w = await loadVactrolWasm();
  expect(w.call('session_init', 48000, 0)).toBe(1);
  const rig: Rig = { w, seq: 0, rev: 0, epoch: 0, now: 0 };
  drain(rig);
  return rig;
}

/** The outbox since the last drain; every worklet-bound install is acknowledged. */
function drain(rig: Rig): WasmRecord[] {
  const r = rig.w.drainRecords();
  ackSampleInstalls(rig.w, r);
  ackControls(rig.w, r);
  return r;
}

const envelopes = (records: readonly WasmRecord[]): Env[] => jsonOf<Env>(records, TAG_SESSION);
const kinds = (records: readonly WasmRecord[]): string[] => envelopes(records).map((e) => e.kind);
const bodiesOf = <T>(records: readonly WasmRecord[], kind: string): T[] =>
  envelopes(records)
    .filter((e) => e.kind === kind)
    .map((e) => e.body as unknown as T);

function send(rig: Rig, kind: string, body: unknown): WasmRecord[] {
  rig.seq += 1;
  rig.w.callStr('session_apply', JSON.stringify({ v: 1, seq: rig.seq, kind, body }));
  return drain(rig);
}

function subscribe(rig: Rig): void {
  expect(send(rig, 'subscribe', { telemetry: true, levels: false, diagnostics: true })).toEqual([]);
}

/** `eval` of the whole document at the next revision and edit epoch. */
function evalDoc(rig: Rig, code: string): { result: EvalResultBody; records: WasmRecord[] } {
  rig.rev += 1;
  rig.epoch += 1;
  const records = send(rig, 'eval', { file: FILE, code, doc_revision: rig.rev, edit_epoch: rig.epoch });
  const result = envelopes(records).find((e) => e.kind === 'eval-result' && e.re === rig.seq);
  expect(result, `an eval-result answering seq ${rig.seq}`).toBeDefined();
  return { result: (result as Env).body as unknown as EvalResultBody, records };
}

function setTweak(rig: Rig, site: WireSite, value: number): WasmRecord[] {
  return send(rig, 'set-tweak', { file: FILE, id: site.id, form_gen: site.form_gen, value, edit_epoch: rig.epoch });
}

/** One `session_tick` STEP later; its records. */
function tick(rig: Rig): WasmRecord[] {
  rig.now = Math.round((rig.now + STEP) * 1e6) / 1e6;
  rig.w.call('session_tick', rig.now);
  return drain(rig);
}

/** Ticks up to `until`; `each(now, records)` sees every tick. */
function tickTo(rig: Rig, until: number, each?: (now: number, records: WasmRecord[]) => void): WasmRecord[] {
  const out: WasmRecord[] = [];
  while (rig.now < until) {
    const r = tick(rig);
    each?.(rig.now, r);
    out.push(...r);
  }
  return out;
}

function putSamples(rig: Rig, ...banks: string[]): void {
  for (const b of banks) expect(putSample(rig.w, `${b}:0`, new Float32Array(64), 48000, 1)).toBe(1);
}

/** The byte span of the `occurrence`-th `needle` in `text`. */
function byteSpan(text: string, needle: string, occurrence = 0): { start: number; end: number } {
  let at = -1;
  for (let i = 0; i <= occurrence; i += 1) {
    at = text.indexOf(needle, at + 1);
    expect(at, `occurrence ${i} of ${needle}`).toBeGreaterThanOrEqual(0);
  }
  const start = enc.encode(text.slice(0, at)).length;
  return { start, end: start + enc.encode(needle).length };
}

function siteAt(sites: readonly WireSite[], text: string, needle: string, occurrence = 0): WireSite {
  const span = byteSpan(text, needle, occurrence);
  const site = sites.find((s) => s.span.start === span.start && s.span.end === span.end);
  expect(site, `a site at ${needle}`).toBeDefined();
  return site as WireSite;
}

/** `(cell, value)` of every worklet-bound cell batch entry. */
function batchValues(records: readonly WasmRecord[]): number[] {
  const out: number[] = [];
  for (const r of records) {
    if (r.tag !== TAG_CELL_BATCH) continue;
    const v = new DataView(r.bytes.buffer, r.bytes.byteOffset, r.bytes.byteLength);
    const count = v.getUint32(4, true);
    for (let i = 0; i < count; i += 1) out.push(v.getFloat32(8 + 12 * i + 8, true));
  }
  return out;
}

describe('criterion 1: playing events and highlighting on the audio clock', () => {
  // A non-ASCII comment line puts the pattern at different UTF-8 and UTF-16 offsets.
  const CODE = '# café ♪\ns [:bd :sd :hh :sd] > d1\n';
  const STEPS: [string, number][] = [
    [':bd', 0],
    [':sd', 0],
    [':hh', 0],
    [':sd', 1],
  ];

  it('stamps every event with the eval revision, within one lookahead of the tick that emitted it', async () => {
    const rig = await start();
    subscribe(rig);
    putSamples(rig, 'bd', 'sd', 'hh');
    const { result } = evalDoc(rig, CODE);
    expect(result.diagnostics).toEqual([]);
    const emitted: [number, WirePlaying][] = [];
    tickTo(rig, 2 * CYCLE, (now, r) => {
      for (const b of bodiesOf<{ events: WirePlaying[] }>(r, 'playing')) {
        for (const e of b.events) if (e.time < 2 * CYCLE) emitted.push([now, e]);
      }
    });
    expect(emitted.map(([, e]) => e.time)).toEqual([0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5]);
    emitted.forEach(([now, e], i) => {
      expect(e.slot).toBe('d1');
      expect(e.src?.file).toBe(FILE);
      expect(e.src?.doc_revision).toBe(1);
      // Emitted when the scheduler commits it: at most one lookahead ahead of the tick.
      expect(e.time - now).toBeLessThanOrEqual(LOOKAHEAD);
      expect(e.time - now).toBeGreaterThanOrEqual(-STEP);
      const [needle, occurrence] = STEPS[i % 4] as [string, number];
      expect(e.src?.span).toEqual(byteSpan(CODE, needle, occurrence));
    });
  });

  it('HighlightScheduler on the mock clock activates exactly the playing step', async () => {
    const rig = await start();
    subscribe(rig);
    putSamples(rig, 'bd', 'sd', 'hh');
    const clock = new MockClock();
    const client = new Client(new RecordingTransport());
    const sync = new DocumentSync(client.document(FILE), Text.of(CODE.split('\n')));
    let tempo: TempoBody | null = null;
    const sched = new HighlightScheduler({
      clock,
      file: FILE,
      map: (span, rev) => sync.mapWireSpan(span, rev),
      tempo: () => tempo,
    });
    evalDoc(rig, CODE);
    // Each step is active from its event time: sample a quarter step in.
    const probes: [number, string][] = [];
    tickTo(rig, 2 * CYCLE, (now, r) => {
      clock.set(now);
      for (const t of bodiesOf<TempoBody>(r, 'tempo')) tempo = t;
      for (const b of bodiesOf<{ events: WirePlaying[] }>(r, 'playing')) sched.onPlaying(b.events);
      const phase = Math.round(((now % 0.5) / 0.5) * 1e6) / 1e6;
      if (Math.abs(phase - 0.24) < 1e-6 || Math.abs(phase - 0.28) < 1e-6) {
        const active = sched.tick().map((x) => CODE.slice(x.from, x.to));
        probes.push([now, active.join(',')]);
      }
    });
    expect(tempo).not.toBeNull();
    expect(probes.length).toBeGreaterThanOrEqual(8);
    for (const [now, active] of probes) {
      const step = Math.floor(now / 0.5) % 4;
      const [needle] = STEPS[step] as [string, number];
      expect([now, active]).toEqual([now, needle]);
    }
  });
});

describe('criterion 2: tweak write tiers and write authority', () => {
  const CODE = 's [:bd :bd :bd :bd] > gain 0.5 > lpf {* 400 2} > d1\n';

  async function playing(): Promise<{ rig: Rig; sites: WireSite[] }> {
    const rig = await start();
    subscribe(rig);
    putSamples(rig, 'bd');
    const { result } = evalDoc(rig, CODE);
    expect(result.diagnostics).toEqual([]);
    tickTo(rig, 0.5);
    return { rig, sites: result.sites };
  }

  it('a direct write needs no eval-result; the next site table carries its value', async () => {
    const { rig, sites } = await playing();
    const gain = siteAt(sites, CODE, '0.5');
    expect(gain.tier).toBe('direct');
    expect(setTweak(rig, gain, 0.9)).toEqual([]);
    const after = tickTo(rig, 1.0);
    // The write goes worklet-bound as a cell batch; no pass, no eval-result.
    expect(batchValues(after).some((v) => Math.abs(v - 0.9) < 1e-6)).toBe(true);
    expect(kinds(after)).not.toContain('eval-result');
    expect(kinds(after)).not.toContain('bindings');
    expect(kinds(after)).not.toContain('stale-binding');
    // The next site table (the rebuild of the reeval site's form) shows the written value.
    const lpf = siteAt(sites, CODE, '400');
    expect(setTweak(rig, lpf, 500)).toEqual([]);
    const next = tick(rig);
    const batches = bodiesOf<BindingsBody>(next, 'bindings');
    expect(batches).toHaveLength(1);
    expect(siteAt((batches[0] as BindingsBody).sites, CODE, '0.5').value).toBeCloseTo(0.9, 6);
    expect(kinds([...after, ...next])).not.toContain('eval-result');
  });

  it('a reeval write rebuilds its form in one bindings batch; the old form_gen is then stale', async () => {
    const { rig, sites } = await playing();
    const lpf = siteAt(sites, CODE, '400');
    expect(lpf.tier).toBe('reeval');
    expect(lpf.call).toMatchObject({ name: 'lpf', param: 'cutoff', ordinal: 1, arg: 0 });
    expect(setTweak(rig, lpf, 500)).toEqual([]);
    const out = tick(rig);
    expect(kinds(out)).not.toContain('eval-result');
    const batches = bodiesOf<BindingsBody>(out, 'bindings');
    expect(batches).toHaveLength(1);
    const fresh = siteAt((batches[0] as BindingsBody).sites, CODE, '400');
    expect(fresh.form_gen).toBeGreaterThan(lpf.form_gen);
    expect(fresh.value).toBe(500);
    // The pre-rebuild generation is rejected.
    const stale = bodiesOf<StaleBindingBody>(setTweak(rig, lpf, 600), 'stale-binding');
    expect(stale).toHaveLength(1);
    expect(stale[0]).toMatchObject({ target: lpf.id, reason: 'stale-form-gen' });
    // The rebuilt site is writable.
    expect(setTweak(rig, fresh, 600)).toEqual([]);
  });

  it('a doc-changed touching a site makes a later write edit-invalidated', async () => {
    const { rig, sites } = await playing();
    const gain = siteAt(sites, CODE, '0.5');
    // `0.5` retyped as `0.55` (revision 2, epoch 2).
    const at = byteSpan(CODE, '0.5');
    rig.rev += 1;
    rig.epoch += 1;
    const changed = send(rig, 'doc-changed', {
      file: FILE,
      doc_revision: rig.rev,
      base_revision: rig.rev - 1,
      changes: [{ from: at.start, to: at.end, insert_len: 4 }],
      dirty: [{ start: at.start, end: at.start + 4 }],
      edit_epoch: rig.epoch,
    });
    expect(changed).toEqual([]);
    const stale = bodiesOf<StaleBindingBody>(setTweak(rig, gain, 0.7), 'stale-binding');
    expect(stale).toHaveLength(1);
    expect(stale[0]).toMatchObject({ target: gain.id, reason: 'edit-invalidated' });
  });
});

describe('criterion 3: site origins, call fields and an inst-default direct write', () => {
  const CODE =
    'let base 0.4\n' +
    'inst pad freq: float = 440 amp: float = 0.5:\n' +
    '\tsaw freq\n' +
    '\t\t> * amp\n' +
    's :pad > note [60 64] > lpf 800 > gain {* base 2} > d1\n';

  it('reports pattern-literal, binding and inst-default sites; lpf carries its call', async () => {
    const rig = await start();
    subscribe(rig);
    const { result } = evalDoc(rig, CODE);
    expect(result.diagnostics).toEqual([]);
    const origins = new Set(result.sites.map((s) => s.origin));
    expect(origins).toEqual(new Set(['pattern-literal', 'binding', 'inst-default']));
    expect(siteAt(result.sites, CODE, '0.4')).toMatchObject({ origin: 'binding', tier: 'reeval' });
    expect(siteAt(result.sites, CODE, '0.5')).toMatchObject({ origin: 'inst-default', tier: 'direct' });
    expect(siteAt(result.sites, CODE, '440')).toMatchObject({ origin: 'inst-default', tier: 'direct' });
    const lpf = siteAt(result.sites, CODE, '800');
    expect(lpf.origin).toBe('pattern-literal');
    expect(lpf.call).toEqual({ name: 'lpf', head: byteSpan(CODE, 'lpf'), ordinal: 1, arg: 0, param: 'cutoff' });
  });

  it('an inst-default write is accepted with no eval-result and no rebuild of the inst form', async () => {
    const rig = await start();
    subscribe(rig);
    const { result } = evalDoc(rig, CODE);
    const amp = siteAt(result.sites, CODE, '0.5');
    const instGen = amp.form_gen;
    tickTo(rig, 0.5);
    const reply = setTweak(rig, amp, 0.8);
    expect(reply).toEqual([]);
    const after = tickTo(rig, CYCLE + 0.5);
    expect(kinds(after)).not.toContain('eval-result');
    expect(kinds(after)).not.toContain('stale-binding');
    for (const b of bodiesOf<BindingsBody>(after, 'bindings')) {
      expect(b.sites.filter((s) => s.origin === 'inst-default' || s.form_gen === instGen)).toEqual([]);
    }
  });
});

describe('criterion 11: visual programs through the render host', () => {
  type Program = Extract<RenderRecord, { op: 'program' }>;
  const programsOf = (records: readonly WasmRecord[]): Program[] =>
    jsonOf<RenderRecord>(records, TAG_RENDER).filter((r): r is Program => r.op === 'program');

  interface Pane {
    gl: RecordingGL;
    host: GlRenderHost;
    diags: HostDiagnostic[];
    canvases: FakeContext2D[];
    /** The linked program each output draws with (tracked per accepted record). */
    drawing: Map<number, FakeProgram>;
    accept(records: readonly Program[]): void;
  }

  function pane(): Pane {
    const gl = new RecordingGL();
    const canvases: FakeContext2D[] = [];
    // Node has no canvas: a plain object whose 2D context records its calls.
    const createCanvas = (): HTMLCanvasElement => {
      const fake: { width: number; height: number; getContext(kind: string): unknown } = {
        width: 0,
        height: 0,
        getContext: (kind: string) => (kind === '2d' ? ctx.as() : null),
      };
      const canvas = fake as unknown as HTMLCanvasElement;
      const ctx = new FakeContext2D(canvas);
      canvases.push(ctx);
      return canvas;
    };
    const host = new GlRenderHost(gl.as(), { width: 640, height: 360 }, { createCanvas });
    const diags: HostDiagnostic[] = [];
    host.onDiagnostic((d) => diags.push(d));
    const drawing = new Map<number, FakeProgram>();
    return {
      gl,
      host,
      diags,
      canvases,
      drawing,
      accept(records) {
        for (const r of records) {
          const mark = gl.named('linkProgram').length;
          host.onRecord(r);
          const linked = gl.named('linkProgram').slice(mark);
          if (linked.length === 1) drawing.set(r.out, linked[0]?.args[0] as FakeProgram);
        }
      },
    };
  }

  /** Ticks until the first program records arrive (at most two cycles); them and the tick time. */
  function awaitPrograms(rig: Rig): { programs: Program[]; at: number } {
    const until = rig.now + 2 * CYCLE;
    while (rig.now < until) {
      const programs = programsOf(tick(rig));
      if (programs.length > 0) return { programs, at: rig.now };
    }
    throw new Error('no program record');
  }

  /** The programs `draw` used, in draw order. */
  function drawn(p: Pane, time: number): FakeProgram[] {
    const mark = p.gl.calls.length;
    p.host.draw(time);
    return p.gl
      .since(mark)
      .filter((c) => c.fn === 'drawArrays')
      .map((c) => c.args[4] as FakeProgram);
  }

  const OSC = 'osc 20 > rotate 0.5 > out o0\n';
  const TEXT = 'text "hello" > out o1\n';
  // A visual-chain parameter that is not a number: the form fails with a type error.
  const BROKEN = 'osc 20 > rotate "a" > out o0\n';

  it('osc > rotate renders from the cycle boundary; text renders its asset; a broken chain keeps the previous program', async () => {
    const rig = await start();
    subscribe(rig);
    const p = pane();
    tickTo(rig, 0.1);

    // The program leaves only at the cycle boundary, then compiles and draws.
    const first = evalDoc(rig, OSC);
    expect(first.result.diagnostics).toEqual([]);
    expect(jsonOf<RenderRecord>(first.records, TAG_RENDER)).toEqual([]);
    const boundary = awaitPrograms(rig);
    expect(boundary.at).toBeGreaterThan(CYCLE - 0.25);
    expect(boundary.at).toBeLessThanOrEqual(CYCLE + STEP);
    expect(boundary.programs.map((x) => x.out)).toEqual([0]);
    p.accept(boundary.programs);
    expect(p.diags).toEqual([]);
    expect(p.host.hasProgram(0)).toBe(true);
    const osc = p.drawing.get(0) as FakeProgram;
    expect(osc.ok).toBe(true);
    expect(drawn(p, rig.now)).toEqual([osc]);

    // `text "hello"` carries a TextAsset; the host rasterizes it and draws o1.
    const second = evalDoc(rig, OSC + TEXT);
    expect(second.result.diagnostics).toEqual([]);
    const next = awaitPrograms(rig).programs;
    const text = next.find((x) => x.out === 1) as Program;
    expect(text).toBeDefined();
    expect(text.assets).toHaveLength(1);
    expect(text.assets[0]?.text).toBe('hello');
    expect(text.source).toContain(`u_text${text.assets[0]?.id ?? -1}`);
    p.accept(next);
    expect(p.diags).toEqual([]);
    expect(p.host.hasProgram(1)).toBe(true);
    expect(p.canvases.flatMap((c) => c.named('fillText')).map((c) => c.args[0])).toEqual(['hello']);
    const before = [p.drawing.get(0), p.drawing.get(1)];
    expect(drawn(p, rig.now)).toEqual(before);

    // The broken chain: its form fails with a diagnostic and no o0 program is sent;
    // the previous frame keeps rendering.
    const links = p.gl.named('linkProgram').length;
    const third = evalDoc(rig, BROKEN + TEXT);
    const failed = third.result.forms.map((f) => f.failure).filter((d): d is Diagnostic => d !== undefined);
    expect(failed).toHaveLength(1);
    expect(failed[0]).toMatchObject({ code: 'type', severity: 'error', span: byteSpan(BROKEN, 'rotate "a"') });
    const later = [...third.records, ...tickTo(rig, rig.now + 2 * CYCLE)];
    const sent = programsOf(later);
    expect(sent.filter((x) => x.out === 0)).toEqual([]);
    p.accept(sent);
    expect(p.diags).toEqual([]);
    expect(p.host.hasProgram(0)).toBe(true);
    expect(p.drawing.get(0)).toBe(before[0]);
    expect(p.gl.named('linkProgram').length - links).toBe(sent.length);
    expect(drawn(p, rig.now)).toContain(before[0]);
  });
});

describe('session_check: a type error is reported and nothing plays differently', () => {
  it('returns the diagnostic and leaves the playing revision untouched', async () => {
    const rig = await start();
    subscribe(rig);
    putSamples(rig, 'bd', 'sd');
    const code = 's [:bd :sd] > d1\n';
    evalDoc(rig, code);
    const before = tickTo(rig, CYCLE);
    const revs = (records: readonly WasmRecord[]): number[] =>
      bodiesOf<{ events: WirePlaying[] }>(records, 'playing').flatMap((b) => b.events.map((e) => e.src?.doc_revision ?? -1));
    expect(revs(before).length).toBeGreaterThan(0);
    const broken = 'let n {+ 1 "a"}\ns [:bd :sd :sd] > d1\n';
    rig.w.callStr('session_check', JSON.stringify({ file: FILE, code: broken }));
    const checks = jsonOf<SessionCheckRecord>(drain(rig), TAG_SESSION);
    expect(checks).toHaveLength(1);
    expect(checks[0]?.kind).toBe('check');
    expect(checks[0]?.diagnostics.map((d) => [d.code, d.severity])).toContainEqual(['type-mismatch', 'error']);
    // The pattern keeps its two steps per cycle at revision 1.
    const after = tickTo(rig, 3 * CYCLE);
    expect(new Set(revs(after))).toEqual(new Set([1]));
    const times = bodiesOf<{ events: WirePlaying[] }>(after, 'playing').flatMap((b) => b.events.map((e) => e.time));
    expect(times.filter((t) => t >= 2 * CYCLE && t < 3 * CYCLE)).toEqual([4, 5]);
    expect(kinds(after)).not.toContain('eval-result');
  });
});
