import { describe, expect, it } from 'vitest';
import { loadVactrWasm, TAG_SESSION, type VactrWasm, type WasmRecord } from '../support/wasm';
import { decodeServer } from '../../src/protocol/envelope';
import { Store } from '../../src/protocol/store';

const decoder = new TextDecoder();
type Selector = { family: Array<{ kind: string; id?: number; name?: string }> };
type Message = { kind: string; re?: number; body: {
  epoch?: string; application_frame?: string; state?: string; instruments?: Selector[];
  selector?: Selector; muted?: boolean;
} };
const tone = (frequency: number, duration: string) =>
  `inst tone freq: float = ${frequency}:\n\tsin-osc freq > * amp\nsong {part [tone: {s :tone > gain 0.1}] duration: ${duration}} tail-seconds: 0 > play-song`;

function framed(records: WasmRecord[]) {
  const bytes = new Uint8Array(records.reduce((n, r) => n + r.bytes.length + 5, 0));
  let offset = 0;
  for (const record of records) {
    new DataView(bytes.buffer).setUint32(offset, record.bytes.length + 1, true);
    bytes[offset + 4] = record.tag;
    bytes.set(record.bytes, offset + 5);
    offset += record.bytes.length + 5;
  }
  return bytes;
}
async function actualPair() {
  const session = await loadVactrWasm();
  const worklet = await loadVactrWasm();
  expect(worklet.call('worklet_init', 8000, 1024 * 1024, 16)).toBe(1);
  expect(session.call('session_init', 8000, 1024 * 1024)).toBe(1);
  const messages: Message[] = [];
  const store = new Store();
  const pending: WasmRecord[] = [];
  const commands: WasmRecord[] = [];
  const receipts: WasmRecord[] = [];
  const allowedFailures = new Set<number>();
  let frame = 0;
  const send = (seq: number, kind: string, body: unknown) => {
    session.callStr('session_apply', JSON.stringify({ v: 1, seq, kind, body }));
  };
  send(1, 'subscribe', { telemetry: true, levels: false, diagnostics: true });
  return {
    messages, commands, receipts, send, store,
    allowFailure(seq: number) { allowedFailures.add(seq); },
    apply(seq: number, code: string, revision: number, editEpoch = 0) {
      store.beginSongApply('main.vact', revision, seq);
      send(seq, 'apply-song', { file: 'main.vact', code, doc_revision: revision, edit_epoch: editEpoch });
    },
    step() {
      for (const record of session.drainRecords()) {
        if (record.tag === TAG_SESSION) {
          const text = decoder.decode(record.bytes);
          const decoded = decodeServer(text);
          expect(decoded.ok, text).toBe(true);
          if (!decoded.ok) throw new Error(decoded.error.code);
          store.apply(decoded.env);
          messages.push(JSON.parse(text) as Message);
        }
        else if (record.tag < 0x40) { pending.push(record); commands.push(record); }
      }
      expect(messages.filter((m) => m.kind === 'song-candidate-failed' &&
        !allowedFailures.has(m.re ?? -1))).toEqual([]);
      for (let n = 0; pending.length && n < 4; n++) {
        const record = pending[0];
        const bytes = new Uint8Array(record.bytes.length + 1);
        bytes[0] = record.tag;
        bytes.set(record.bytes, 1);
        const ptr = worklet.call('staging_ptr');
        new Uint8Array((worklet.exports.memory as WebAssembly.Memory).buffer, ptr, bytes.length).set(bytes);
        if (worklet.call('worklet_inbox', bytes.length) === 0) break;
        pending.shift();
      }
      const ptr = worklet.call('process', 128);
      const pcm = new Float32Array((worklet.exports.memory as WebAssembly.Memory).buffer, ptr, 256).slice();
      const feedback = worklet.drainRecords();
      receipts.push(...feedback);
      expect(feedback.filter((r) => r.tag === 0x60)).toEqual([]);
      session.withBytes(framed(feedback), (p, n) => session.call('session_inbox', p, n));
      session.call('session_tick', worklet.call('worklet_now'));
      const start = frame;
      frame += 128;
      return { start, pcm };
    },
  };
}
const applied = (messages: Message[], seq: number) =>
  messages.find((m) => m.kind === 'song-candidate-applied' && m.re === seq);
function exactFrame(message: Message) {
  expect(message.body.application_frame).toMatch(/^(0|[1-9][0-9]*)$/);
  return BigInt(message.body.application_frame!);
}

// These are full replacement acceptance fixtures, separate from initial Apply.
// They require a freshly rebuilt artifact containing both DSP and controller phases.
describe('real browser atomic song Apply', () => {
  it('replaces Playing audio at an exact old-cycle boundary and retires the new finite song', async () => {
    const pair = await actualPair();
    pair.apply(11, tone(220, '8'), 1);
    let submitted = false;
    let ended = false;
    const spectral: number[] = [];
    for (let turn = 0; turn < 2048; turn++) {
      const block = pair.step();
      const old = applied(pair.messages, 11);
      if (old && !submitted && block.pcm.some((x) => Math.abs(x) > 1e-6)) {
        pair.apply(22, tone(880, '1'), 2);
        submitted = true;
      }
      const next = applied(pair.messages, 22);
      if (!next) continue;
      const boundary = exactFrame(next);
      expect(boundary).toBeGreaterThan(exactFrame(old!));
      expect((boundary - exactFrame(old!)) % 16000n).toBe(0n);
      if (BigInt(block.start) >= boundary + 512n && spectral.length < 2048) {
        // The worklet ABI returns planar left/right, 128 frames per channel.
        for (let i = 0; i < 128; i++) spectral.push(block.pcm[i]);
      }
      ended = pair.messages.some((m) => m.kind === 'song-transport-state' &&
        m.body.epoch === next.body.epoch && m.body.state === 'ended');
      if (ended) break;
    }
    expect(submitted).toBe(true);
    expect(ended, JSON.stringify(pair.messages)).toBe(true);
    expect(pair.messages.filter((m) => m.kind === 'song-candidate-applied').map((m) => m.re)).toEqual([11, 22]);
    expect(spectral).toHaveLength(2048);
    const energy = (frequency: number) => {
      let sine = 0, cosine = 0;
      spectral.forEach((sample, i) => {
        sine += sample * Math.sin(2 * Math.PI * frequency * i / 8000);
        cosine += sample * Math.cos(2 * Math.PI * frequency * i / 8000);
      });
      return Math.hypot(sine, cosine) / spectral.length;
    };
    const spectrum = {
      next: energy(880), previous: energy(220),
      rms: Math.sqrt(spectral.reduce((sum, sample) => sum + sample * sample, 0) / spectral.length),
      peak: Math.max(...spectral.map(Math.abs)),
    };
    expect(spectrum.next, JSON.stringify(spectrum)).toBeGreaterThan(1e-4);
    expect(energy(220)).toBeLessThan(energy(880) * 0.02);
    expect(pair.step().pcm.every((x) => Math.abs(x) < 1e-7)).toBe(true);
  }, 30000);

  it('preserves an acknowledged mute when unchanged definitions receive different numeric IDs', async () => {
    const pair = await actualPair();
    pair.apply(11, tone(440, '4'), 1);
    let muteSent = false, replacementSent = false, unmuteSent = false, resumed = false;
    let silenceBlocks = 0;
    let ended = false;
    for (let turn = 0; turn < 2048; turn++) {
      const block = pair.step();
      const old = applied(pair.messages, 11);
      const state = pair.messages.find((m) => m.kind === 'song-transport-state' &&
        m.body.epoch === old?.body.epoch && m.body.state === 'playing');
      if (old && state && !muteSent && block.pcm.some((x) => Math.abs(x) > 1e-6)) {
        const selector = state.body.instruments?.find((s) => s.family.some((f) => f.kind === 'instrument'));
        expect(selector).toBeDefined();
        pair.send(12, 'mute-instrument', { epoch: old.body.epoch, selector, muted: true });
        muteSent = true;
      }
      if (!replacementSent && pair.messages.some((m) => m.kind === 'song-instrument-muted' && m.re === 12)) {
        pair.apply(22, 'inst unused:\n\tsin-osc freq > * amp\n' + tone(440, '3/2'), 2);
        replacementSent = true;
      }
      const next = applied(pair.messages, 22);
      if (!next) continue;
      const boundary = exactFrame(next);
      const carried = pair.messages.find((m) => m.kind === 'song-instrument-muted' &&
        m.re === undefined && m.body.epoch === next.body.epoch && m.body.muted === true &&
        m.body.selector?.family.some((f) => f.kind === 'instrument'));
      expect(carried, 'actual Applied must publish the new epoch mute state for the editor').toBeDefined();
      expect(exactFrame(carried!)).toBe(boundary);
      expect(pair.store.song('main.vact')?.applied?.epoch).toBe(next.body.epoch);
      if (!unmuteSent) {
        const carriedAliases = pair.messages.filter((m) => m.kind === 'song-instrument-muted' &&
          m.re === undefined && m.body.epoch === next.body.epoch && m.body.muted === true);
        for (const alias of carriedAliases) expect(exactFrame(alias)).toBe(boundary);
        expect(pair.store.song('main.vact')?.instrumentMutes).toEqual(
          carriedAliases.flatMap((alias) => alias.body.selector!.family.map((sound) => ({
            sound, muted: true, application_frame: alias.body.application_frame,
          }))),
        );
      }
      if (!unmuteSent && BigInt(block.start) >= boundary) {
        expect(block.pcm.every((x) => Math.abs(x) < 1e-7)).toBe(true);
        silenceBlocks++;
        if (BigInt(block.start) >= boundary + 4000n) {
          const nextState = pair.messages.find((m) => m.kind === 'song-transport-state' &&
            m.body.epoch === next.body.epoch && m.body.state === 'playing');
          const selector = nextState?.body.instruments?.find((s) => s.family.some((f) => f.kind === 'instrument'));
          expect(selector).toBeDefined();
          const oldId = state?.body.instruments?.flatMap((s) => s.family).find((f) => f.kind === 'instrument')?.id;
          expect(selector!.family.find((f) => f.kind === 'instrument')?.id).not.toBe(oldId);
          expect(carried!.body.selector?.family).toEqual(selector!.family);
          pair.send(23, 'mute-instrument', { epoch: next.body.epoch, selector, muted: false });
          unmuteSent = true;
        }
      }
      if (unmuteSent && BigInt(block.start) >= boundary + 16000n &&
        block.pcm.some((x) => Math.abs(x) > 1e-6)) resumed = true;
      ended = pair.messages.some((m) => m.kind === 'song-transport-state' &&
        m.body.epoch === next.body.epoch && m.body.state === 'ended');
      if (ended) break;
    }
    expect(silenceBlocks).toBeGreaterThan(0);
    expect(resumed).toBe(true);
    expect(pair.messages.some((m) => m.kind === 'song-instrument-muted' && m.re === 23)).toBe(true);
    expect(ended, JSON.stringify(pair.messages)).toBe(true);
  }, 30000);

  it('cancels a posted replacement before arm and permits another Apply after real cleanup', async () => {
    const pair = await actualPair();
    pair.apply(11, tone(220, '8'), 1);
    pair.allowFailure(22);
    let requested = false, edited = false, retried = false, ended = false;
    let cancelledEpoch: bigint | undefined;
    let cancelledBoundary: bigint | undefined;
    let oldAudioAfterCancellation = false;
    const ackEpoch = (record: WasmRecord) =>
      new DataView(record.bytes.buffer, record.bytes.byteOffset, record.bytes.byteLength).getBigUint64(1, true);
    const resource = (record: WasmRecord, offset: number) => {
      const view = new DataView(record.bytes.buffer, record.bytes.byteOffset, record.bytes.byteLength);
      return `${view.getUint32(offset, true)}:${view.getUint32(offset + 4, true)}`;
    };
    for (let turn = 0; turn < 2048; turn++) {
      const block = pair.step();
      const old = applied(pair.messages, 11);
      if (old && !requested && block.pcm.some((x) => Math.abs(x) > 1e-6)) {
        pair.apply(22, tone(880, '2'), 2);
        requested = true;
      }
      const replace = pair.commands.find((r) => r.tag === 0x1b && r.bytes[0] === 20);
      if (replace && !edited) {
        const view = new DataView(replace.bytes.buffer, replace.bytes.byteOffset, replace.bytes.byteLength);
        cancelledEpoch = view.getBigUint64(1, true);
        cancelledBoundary = view.getBigUint64(9, true);
        expect(BigInt(block.start + 128)).toBeLessThan(cancelledBoundary - 64n);
        pair.send(23, 'doc-changed', {
          file: 'main.vact', doc_revision: 3, base_revision: 2,
          changes: [], dirty: [], edit_epoch: 1,
        });
        edited = true;
      }
      if (!edited) continue;
      const own = pair.receipts.filter((r) => r.tag === 0x49 && ackEpoch(r) === cancelledEpoch);
      const cancelled = own.some((r) => r.bytes[0] === 7);
      const failed = pair.messages.filter((m) => m.kind === 'song-candidate-failed' && m.re === 22);
      expect(applied(pair.messages, 22)).toBeUndefined();
      if (cancelled && !retried) {
        expect(failed).toHaveLength(1);
        const rejection = own.find((r) => r.bytes[0] === 14);
        expect(rejection).toBeDefined();
        const view = new DataView(rejection!.bytes.buffer, rejection!.bytes.byteOffset, rejection!.bytes.byteLength);
        expect(view.getBigUint64(9, true)).toBe(cancelledBoundary);
        const ready = own.filter((r) => r.bytes[0] === 4).map((r) => resource(r, 9));
        const returned = own.filter((r) => r.bytes[0] === 6);
        expect(ready.length).toBeGreaterThan(0);
        for (const key of ready) expect(returned.filter((r) => resource(r, 9) === key)).toHaveLength(1);
        // Each returned receipt retains kind as well as epoch/id/generation.
        const fullKeys = returned.map((r) => `${ackEpoch(r)}:${r.bytes[17]}:${resource(r, 9)}`);
        expect(new Set(fullKeys).size).toBe(fullKeys.length);
        oldAudioAfterCancellation = block.pcm.some((x) => Math.abs(x) > 1e-6);
        if (!oldAudioAfterCancellation) continue;
        pair.apply(33, tone(660, '1'), 3, 1);
        retried = true;
      }
      const next = applied(pair.messages, 33);
      if (next) {
        expect((exactFrame(next) - exactFrame(old!)) % 16000n).toBe(0n);
        ended = pair.messages.some((m) => m.kind === 'song-transport-state' &&
          m.body.epoch === next.body.epoch && m.body.state === 'ended');
        if (ended) break;
      }
    }
    expect(edited, 'a real compound replacement must be posted').toBe(true);
    expect(oldAudioAfterCancellation).toBe(true);
    expect(retried).toBe(true);
    expect(ended, JSON.stringify(pair.messages)).toBe(true);
    expect(pair.messages.filter((m) => m.kind === 'song-candidate-applied').map((m) => m.re)).toEqual([11, 33]);
    expect(pair.step().pcm.every((x) => Math.abs(x) < 1e-7)).toBe(true);
  }, 30000);

});
