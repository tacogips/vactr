// @vitest-environment node
import { describe, expect, it } from 'vitest';
import type { EvalResultBody, TempoBody, WirePlaying } from '../../src/protocol/types';
import { listExamples } from '../../src/examples/registry';
import { ackSampleInstalls, jsonOf, loadVactrWasm, TAG_RENDER, TAG_SESSION, type VactrWasm, type WasmRecord } from '../support/wasm';

const FILE = 'main.vact';
const STEP = 0.02;
interface Envelope { v: number; seq: number; re?: number; kind: string; body: Record<string, unknown> }
interface Rig { w: VactrWasm; seq: number; rev: number; epoch: number; now: number }
const TAG_SLOT_CONTROL = 0x10, TAG_CELL_INIT = 0x11, TAG_CELL_BATCH = 0x12;
const TAG_SLOT_CONTROL_ACK = 0x40, TAG_CELL_INIT_ACK = 0x41, TAG_CELL_BATCH_ACK = 0x42;

function ackControls(w: VactrWasm, records: readonly WasmRecord[]): void {
  const replies: [number, number[]][] = [];
  for (const record of records) {
    const view = new DataView(record.bytes.buffer, record.bytes.byteOffset, record.bytes.byteLength);
    if (record.tag === TAG_SLOT_CONTROL) replies.push([TAG_SLOT_CONTROL_ACK, [view.getUint32(0, true), view.getUint32(4, true)]]);
    if (record.tag === TAG_CELL_INIT) replies.push([TAG_CELL_INIT_ACK, [view.getUint32(0, true), view.getUint32(4, true)]]);
    if (record.tag === TAG_CELL_BATCH) replies.push([TAG_CELL_BATCH_ACK, [view.getUint32(0, true)]]);
  }
  if (!replies.length) return;
  const bytes = new Uint8Array(replies.reduce((size, [, words]) => size + 5 + 4 * words.length, 0));
  const view = new DataView(bytes.buffer);
  let at = 0;
  for (const [tag, words] of replies) {
    view.setUint32(at, 1 + 4 * words.length, true); view.setUint8(at + 4, tag);
    words.forEach((word, index) => view.setUint32(at + 5 + 4 * index, word, true));
    at += 5 + 4 * words.length;
  }
  w.withBytes(bytes, (ptr, len) => w.call('session_inbox', ptr, len));
}
function drain(rig: Rig): WasmRecord[] {
  const records = rig.w.drainRecords(); ackSampleInstalls(rig.w, records); ackControls(rig.w, records); return records;
}
function envelopes(records: readonly WasmRecord[]): Envelope[] { return jsonOf<Envelope>(records, TAG_SESSION); }
function bodies<T>(records: readonly WasmRecord[], kind: string): T[] {
  return envelopes(records).filter((record) => record.kind === kind).map((record) => record.body as T);
}
async function start(): Promise<Rig> {
  const w = await loadVactrWasm(); expect(w.call('session_init', 48000, 0)).toBe(1);
  const rig = { w, seq: 0, rev: 0, epoch: 0, now: 0 }; drain(rig); return rig;
}
function send(rig: Rig, kind: string, body: unknown): WasmRecord[] {
  rig.seq += 1; rig.w.callStr('session_apply', JSON.stringify({ v: 1, seq: rig.seq, kind, body })); return drain(rig);
}
function subscribe(rig: Rig): void { expect(send(rig, 'subscribe', { telemetry: true, levels: false, diagnostics: true })).toEqual([]); }
function evalDoc(rig: Rig, code: string): EvalResultBody {
  rig.rev += 1; rig.epoch += 1;
  const records = send(rig, 'eval', { file: FILE, code, doc_revision: rig.rev, edit_epoch: rig.epoch });
  const reply = envelopes(records).find((record) => record.kind === 'eval-result' && record.re === rig.seq);
  expect(reply, `eval-result for sequence ${rig.seq}`).toBeDefined(); return (reply as Envelope).body as unknown as EvalResultBody;
}
function tick(rig: Rig): WasmRecord[] { rig.now = Math.round((rig.now + STEP) * 1e6) / 1e6; rig.w.call('session_tick', rig.now); return drain(rig); }
function tickTo(rig: Rig, until: number, each?: (records: WasmRecord[]) => void): WasmRecord[] {
  const result: WasmRecord[] = []; while (rig.now < until) { const records = tick(rig); each?.(records); result.push(...records); } return result;
}
async function readText(path: string): Promise<string> {
  const spec: string = 'node:fs';
  const fs = (await import(/* @vite-ignore */ spec)) as { readFileSync(path: string, encoding: 'utf8'): string };
  return fs.readFileSync(path, 'utf8');
}

const examples = listExamples();
describe('registered examples in the browser wasm session', () => {
  for (const entry of examples) {
    it(`${entry.id} evaluates without errors`, async () => {
      const source = await readText(`../examples/${entry.id}.vact`);
      const rig = await start(); subscribe(rig); const result = evalDoc(rig, source);
      expect(result.diagnostics.filter(({ severity }) => severity === 'error'), entry.id).toEqual([]);
      expect(result.forms.filter(({ failure }) => failure), entry.id).toEqual([]);
      tickTo(rig, 1);
    }, 60_000);
  }

  it('showcase stays within activity and level bounds and emits a render program', async () => {
    const source = await readText('../examples/showcase.vact');
    const rig = await start(); subscribe(rig); const result = evalDoc(rig, source);
    expect(result.diagnostics.filter(({ severity }) => severity === 'error')).toEqual([]);
    expect(result.forms.filter(({ failure }) => failure)).toEqual([]);
    const tempos: TempoBody[] = []; const playing: WirePlaying[] = []; const renders: unknown[] = [];
    tickTo(rig, 4, (records) => {
      tempos.push(...bodies<TempoBody>(records, 'tempo'));
      for (const body of bodies<{ events: WirePlaying[] }>(records, 'playing')) playing.push(...body.events);
      renders.push(...jsonOf(records.filter(({ tag }) => tag === TAG_RENDER), TAG_RENDER));
    });
    expect(tempos.at(-1)?.bpm).toBeGreaterThanOrEqual(126);
    expect(tempos.at(-1)?.bpm).toBeLessThanOrEqual(136);
    expect(new Set(playing.map(({ slot }) => slot)).size).toBeGreaterThanOrEqual(4);
    expect(renders.some((record) => (record as { op?: string }).op === 'program')).toBe(true);
    expect(source.trim().split('\n').length).toBeLessThanOrEqual(40);
    const code = source.split('\n').filter((line) => !line.trim().startsWith('#')).join('\n');
    expect([...code.matchAll(/^\s*s\b.*>\s*d[1-8]\b/gm)]).toHaveLength(6);
    expect([...code.matchAll(/>\s*out\s+o0\b/g)]).toHaveLength(1);
    expect(/\b(sample|host-in|midi)\b/i.test(code)).toBe(false);
    const gains = [...source.matchAll(/gain\s+([0-9.]+)/g)].map((match) => Number(match[1]));
    expect(gains.length).toBeGreaterThanOrEqual(5);
    expect(gains.every((gain) => gain <= 0.32)).toBe(true);
    expect(gains.reduce((sum, gain) => sum + gain, 0)).toBeLessThanOrEqual(1.2);
  }, 60_000);
});
