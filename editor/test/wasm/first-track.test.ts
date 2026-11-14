// TASK-011 criterion (design 15.2): evaluating examples/first-track.vact in
// the REAL browser session (host-wasm artifact, `session_init`) reaches the
// session, which answers with no diagnostics, publishes `tempo` 124 bpm and
// schedules events; fed to the editor's code area, the transport shows it.
// jsdom environment: the code area mounts real CodeMirror and Solid views.
import { describe, expect, it } from 'vitest';
import type { EditorDeps } from '../../src/app/deps';
import { buildLayout } from '../../src/app/layout';
import { mount } from '../../src/code/mount';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import type { EvalResultBody, TempoBody, WirePlaying } from '../../src/protocol/types';
import { MockClock } from '../support/clock';
import { RecordingTransport } from '../support/recording';
import { ackSampleInstalls, jsonOf, loadVactrWasm, TAG_SESSION, type VactrWasm, type WasmRecord } from '../support/wasm';

const FILE = 'main.vact';

// No @types/node in the pinned set (as in ../support/wasm.ts): node:fs is
// loaded through a non-literal dynamic import.
async function readText(path: string): Promise<string> {
  const spec: string = 'node:fs';
  const fs = (await import(/* @vite-ignore */ spec)) as { readFileSync(p: string, enc: 'utf8'): string };
  return fs.readFileSync(path, 'utf8');
}
const STEP = 0.02;

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
function ackControls(w: VactrWasm, records: readonly WasmRecord[]): void {
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
  const w = await loadVactrWasm();
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


describe('examples/first-track.vact through the real browser session', () => {
  it('evaluates cleanly, publishes 124 bpm and plays; the transport shows 124', async () => {
    const code = await readText('../examples/first-track.vact');
    const rig = await start();
    subscribe(rig);
    const { result } = evalDoc(rig, code);
    expect(result.diagnostics.filter((d) => d.severity === 'error')).toEqual([]);
    expect(result.forms.filter((f) => f.failure)).toEqual([]);
    const tempos: TempoBody[] = [];
    const playing: WirePlaying[] = [];
    tickTo(rig, 4.5, (_now, records) => {
      tempos.push(...bodiesOf<TempoBody>(records, 'tempo'));
      for (const p of bodiesOf<{ events: WirePlaying[] }>(records, 'playing')) playing.push(...p.events);
    });
    expect(tempos.length).toBeGreaterThan(0);
    expect(tempos[tempos.length - 1]?.bpm).toBe(124);
    expect(new Set(playing.map((e) => e.slot))).toEqual(new Set(['d1', 'd2', 'd3', 'd4', 'd5', 'd6']));

    // The editor side: the same messages drive the Solid transport view.
    const root = document.createElement('div');
    document.body.appendChild(root);
    const layout = buildLayout(root);
    const store = new Store();
    const transport = new RecordingTransport();
    const deps: EditorDeps = { client: new Client(transport, { store }), store, clock: new MockClock(0), tier: 'native', files: new MemoryFiles() };
    const area = mount(root, deps);
    transport.emit({ kind: 'tempo', body: tempos[tempos.length - 1] as TempoBody });
    expect(layout.transport.querySelector('.vact-tempo')?.getAttribute('aria-label')).toBe('tempo 124.0 bpm');
    area.dispose();
    root.remove();
  }, 60_000);
});
