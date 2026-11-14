import { describe, expect, it } from 'vitest';
import { loadVactrWasm, TAG_SESSION, type VactrWasm, type WasmRecord } from '../support/wasm';

const encode = new TextEncoder();
const decode = new TextDecoder();
type Message = { kind: string; re?: number; body: { state?: string; epoch?: string; application_frame?: string; message?: string; muted?: boolean } };
const tone = 'inst tone freq: float = 440:\n\tsin-osc freq > * amp\nsong {part [tone: {s :tone > gain 0.1}] duration: 1/8} tail-seconds: 0 > play-song';
const sample = 'song {part [drums: {s :bd}] duration: 1/8} tail-seconds: 0 > play-song';

async function pair() {
  const session = await loadVactrWasm();
  const worklet = await loadVactrWasm();
  expect(worklet.call('worklet_init', 8000, 1024 * 1024, 16)).toBe(1);
  expect(session.call('session_init', 8000, 1024 * 1024)).toBe(1);
  session.callStr('session_apply', JSON.stringify({ v: 1, seq: 1, kind: 'subscribe',
    body: { telemetry: true, levels: false, diagnostics: true } }));
  return { session, worklet };
}
function upload(wasm: VactrWasm, value: number) {
  const pcm = new Float32Array(2048).fill(value);
  return wasm.withBytes(encode.encode('bd:0'), (kp, kn) =>
    wasm.withBytes(new Uint8Array(pcm.buffer), (dp) => wasm.call('session_sample_put', kp, kn, dp, pcm.length, 8000, 2)));
}
function catalog(wasm: VactrWasm, body: unknown) {
  return wasm.callStr('session_song_bank_catalog', JSON.stringify(body));
}
function apply(wasm: VactrWasm, code: string) {
  wasm.callStr('session_apply', JSON.stringify({ v: 1, seq: 11, kind: 'apply-song',
    body: { file: 'main.vact', code, doc_revision: 1, edit_epoch: 0 } }));
}
function frame(records: WasmRecord[]): Uint8Array {
  const out = new Uint8Array(records.reduce((total, record) => total + 5 + record.bytes.length, 0));
  let offset = 0;
  for (const record of records) {
    new DataView(out.buffer).setUint32(offset, 1 + record.bytes.length, true);
    out[offset + 4] = record.tag;
    out.set(record.bytes, offset + 5);
    offset += 5 + record.bytes.length;
  }
  return out;
}
function playback(session: VactrWasm, worklet: VactrWasm,
  observe?: (messages: readonly Message[], pcm: Float32Array, frame: number) => void) {
  const pending: WasmRecord[] = [];
  const messages: Message[] = [];
  const blocks: Float32Array[] = [];
  const songReceipts: number[][] = [];
  let ended = false;
  for (let turn = 0; turn < 512; turn++) {
    for (const record of session.drainRecords()) {
      if (record.tag === TAG_SESSION) {
        const message = JSON.parse(decode.decode(record.bytes)) as Message;
        expect(message.kind, JSON.stringify({ message, songReceipts: songReceipts.slice(-8) })).not.toBe('song-candidate-failed');
        messages.push(message);
        ended ||= message.kind === 'song-transport-state' && message.body.state === 'ended';
      } else if (record.tag < 0x40) pending.push(record);
    }
    for (let sent = 0; pending.length && sent < 4; sent++) {
      const record = pending[0];
      const packet = new Uint8Array(1 + record.bytes.length);
      packet[0] = record.tag;
      packet.set(record.bytes, 1);
      const ptr = worklet.call('staging_ptr');
      new Uint8Array((worklet.exports.memory as WebAssembly.Memory).buffer, ptr, packet.length).set(packet);
      if (worklet.call('worklet_inbox', packet.length) === 0) break;
      pending.shift();
    }
    const output = worklet.call('process', 128);
    const pcm = new Float32Array((worklet.exports.memory as WebAssembly.Memory).buffer, output, 256).slice();
    blocks.push(pcm);
    const feedback = worklet.drainRecords();
    for (const record of feedback) {
      if (record.tag === 0x49 && songReceipts.length < 64) songReceipts.push(Array.from(record.bytes));
    }
    expect(feedback.some((record) => record.tag === 0x60)).toBe(false);
    session.withBytes(frame(feedback), (ptr, len) => session.call('session_inbox', ptr, len));
    session.call('session_tick', worklet.call('worklet_now'));
    observe?.(messages, pcm, turn * 128);
    if (ended) break;
  }
  expect(ended, JSON.stringify({ messages: messages.filter((message) => message.kind !== 'tempo'), songReceipts, pending: pending.map((record) => record.tag),
    now: worklet.call('worklet_now'), audible: blocks.some((block) => block.some((value) => Math.abs(value) > 1e-6)) })).toBe(true);
  const acknowledgements = messages.filter((message) => message.kind === 'song-candidate-applied');
  expect(acknowledgements).toHaveLength(1);
  expect(acknowledgements[0].re).toBe(11);
  expect(acknowledgements[0].body.application_frame).toMatch(/^[0-9]+$/);
  const output = worklet.call('process', 128);
  const silence = new Float32Array((worklet.exports.memory as WebAssembly.Memory).buffer, output, 256);
  expect(silence.every((value) => Math.abs(value) < 1e-7)).toBe(true);
  const pcm = new Float32Array(blocks.length * 256);
  blocks.forEach((block, index) => pcm.set(block, index * 256));
  expect(pcm.some((value) => Math.abs(value) > 1e-6)).toBe(true);
  return pcm;
}

describe('real browser song ABI', () => {
  it('rejects malformed/incomplete catalogs and invalid rates without losing valid banks', async () => {
    const { session, worklet } = await pair();
    expect(upload(session, 0.25)).toBe(1);
    expect(catalog(session, { banks: [{ name: 'bd', members: ['bd:0'] }] })).toBe(1);
    for (const body of [{ banks: [{ name: 'bd', members: ['missing'] }] },
      { banks: [{ name: 'bd', members: [] }, { name: 'bd', members: [] }] }, { banks: [], unexpected: true }])
      expect(catalog(session, body)).toBe(0);
    expect(session.call('session_init', 8000.5, 1024 * 1024)).toBe(0);
    expect(session.call('session_song_bank_catalog', 0, 1024 * 1024 + 1)).toBe(0);
    apply(session, sample);
    playback(session, worklet);
  }, 30000);

  it('keeps submitted candidate PCM immutable when uploads and future assets change', async () => {
    const baseline = await pair();
    expect(upload(baseline.session, 0.25)).toBe(1);
    expect(catalog(baseline.session, { banks: [{ name: 'bd', members: ['bd:0'] }] })).toBe(1);
    apply(baseline.session, sample);
    const original = playback(baseline.session, baseline.worklet);
    const changed = await pair();
    expect(upload(changed.session, 0.25)).toBe(1);
    expect(catalog(changed.session, { banks: [{ name: 'bd', members: ['bd:0'] }] })).toBe(1);
    apply(changed.session, sample);
    expect(upload(changed.session, 0.75)).toBe(1);
    expect(changed.session.call('session_song_refresh_assets')).toBe(1);
    expect(playback(changed.session, changed.worklet)).toEqual(original);
  }, 30000);

  it('plays the original whole-code song through genuine worklet receipts and ends in silence', async () => {
    const { session, worklet } = await pair();
    apply(session, tone);
    playback(session, worklet);
  }, 30000);

  it('acknowledges actual mute and unmute without replaying skipped audio', async () => {
    const { session, worklet } = await pair();
    expect(upload(session, 0.25)).toBe(1);
    expect(catalog(session, { banks: [{ name: 'bd', members: ['bd:0'] }] })).toBe(1);
    apply(session, sample.replace('duration: 1/8', 'duration: 3/2'));
    let muted = false;
    let unmuted = false;
    let silentBlocks = 0;
    let noReplayBlocks = 0;
    let resumed = false;
    const requestMute = (epoch: string, value: boolean, seq: number) => {
      session.callStr('session_apply', JSON.stringify({ v: 1, seq, kind: 'mute-instrument',
        body: { epoch, selector: { family: [{ kind: 'builtin', name: 'bd' }] }, muted: value } }));
    };
    let muteAck: Message | undefined;
    let unmuteAck: Message | undefined;
    playback(session, worklet, (messages, pcm, frame) => {
      const applied = messages.find((message) => message.kind === 'song-candidate-applied');
      const audible = pcm.some((value) => Math.abs(value) > 1e-6);
      if (!applied) return;
      const nextOnset = Number(applied.body.application_frame) + 16000;
      if (!muted && audible) {
        requestMute(applied.body.epoch!, true, 12);
        muted = true;
      }
      muteAck = messages.find((message) => message.kind === 'song-instrument-muted' && message.re === 12);
      if (muteAck && !unmuted && frame >= Number(muteAck.body.application_frame) + 64) {
        expect(audible).toBe(false);
        silentBlocks++;
        if (frame >= Number(muteAck.body.application_frame) + 4000) {
          requestMute(applied.body.epoch!, false, 13);
          unmuted = true;
        }
      }
      unmuteAck = messages.find((message) => message.kind === 'song-instrument-muted' && message.re === 13);
      if (unmuteAck && frame + 128 <= nextOnset) {
        expect(audible).toBe(false);
        noReplayBlocks++;
      }
      if (unmuteAck && frame >= nextOnset && audible) resumed = true;
    });
    expect(muteAck?.body.muted).toBe(true);
    expect(unmuteAck?.body.muted).toBe(false);
    expect(silentBlocks).toBeGreaterThan(0);
    expect(noReplayBlocks).toBeGreaterThan(0);
    expect(resumed).toBe(true);
  }, 30000);
});

it('reports the actual song host profile after genuine bootstrap and preserves it on invalid reinitialization', async () => {
  const { session, worklet } = await pair();
  const pending: WasmRecord[] = [];
  for (let turn = 0; turn < 128; turn++) {
    pending.push(...session.drainRecords().filter((record) => record.tag < 0x40));
    for (let sent = 0; pending.length && sent < 4; sent++) {
      const record = pending[0];
      const packet = new Uint8Array(record.bytes.length + 1);
      packet[0] = record.tag;
      packet.set(record.bytes, 1);
      const ptr = worklet.call('staging_ptr');
      new Uint8Array((worklet.exports.memory as WebAssembly.Memory).buffer, ptr, packet.length).set(packet);
      if (worklet.call('worklet_inbox', packet.length) === 0) break;
      pending.shift();
    }
    worklet.call('process', 128);
    const feedback = worklet.drainRecords();
    expect(feedback.some((record) => record.tag === 0x60)).toBe(false);
    session.withBytes(frame(feedback), (p, n) => session.call('session_inbox', p, n));
    session.call('session_tick', worklet.call('worklet_now'));
  }
  expect(pending).toHaveLength(0);
  const request = (epoch: bigint) => {
    const bytes = new Uint8Array(10);
    bytes.set([0x1b, 15]);
    new DataView(bytes.buffer).setBigUint64(2, epoch, true);
    const ptr = worklet.call('staging_ptr');
    new Uint8Array((worklet.exports.memory as WebAssembly.Memory).buffer, ptr, bytes.length).set(bytes);
    expect(worklet.call('worklet_inbox', bytes.length)).toBe(1);
  };
  const report = (epoch: bigint) => {
    worklet.call('process', 128);
    const records = worklet.drainRecords();
    expect(records.some((record) => record.tag === 0x60)).toBe(false);
    const record = records.find((record) => record.tag === 0x49 && record.bytes[0] === 9);
    expect(record).toBeDefined();
    expect(record!.bytes.length).toBe(73);
    const bytes = record!.bytes;
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    expect(view.getBigUint64(1, true)).toBe(epoch);
    expect(view.getUint32(17, true)).toBe(8000);
    expect(view.getUint32(29, true)).toBe(128 - 73);
    expect(view.getUint32(33, true)).toBe(51 - 1);
    return view.getBigUint64(61, true);
  };
  const epoch = 1n << 63n;
  request(epoch);
  const memory = report(epoch);
  const now = worklet.call('worklet_now');
  request(epoch + 1n);
  expect(worklet.call('worklet_init', 8000.5, 1024 * 1024, 16)).toBe(0);
  expect(worklet.call('worklet_now')).toBe(now);
  expect(report(epoch + 1n)).toBe(memory);
}, 30000);
