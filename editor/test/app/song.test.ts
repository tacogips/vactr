import { Text } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { mount } from '../../src/app/song';
import { buildLayout } from '../../src/app/layout';
import type { EditorDeps } from '../../src/app/deps';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { MemoryFiles } from '../../src/platform/files';
import { RecordingTransport } from '../support/recording';
import { MockClock } from '../support/clock';
import { loadVactrWasm, TAG_SESSION, type WasmRecord } from '../support/wasm';
import type { Transport } from '../../src/protocol/transport';
import { decodeClient } from '../../src/protocol/envelope';

const file = 'main.vact';
const selector = { family: [{ kind: 'builtin' as const, name: 'bd' }] };
const cleanups: (() => void)[] = [];

function fixture() {
  const root = document.createElement('div');
  document.body.appendChild(root);
  buildLayout(root);
  const transport = new RecordingTransport();
  const store = new Store();
  const client = new Client(transport, { store });
  const sync = new DocumentSync(client.document(file), Text.of(['song-code']));
  const surface = new CodeSurface({ sync });
  const view = new EditorView({ state: surface.state });
  const deps: EditorDeps = { client, store, files: new MemoryFiles(), clock: new MockClock(), tier: 'browser',
    code: { surface, view, currentRevision: () => sync.revision, mapWireSpan: () => null,
      selectedSiteId: () => null, samples: { frames: () => null, openBrowser: () => {} } } };
  const controls = mount(root, deps, file);
  cleanups.push(() => { controls.dispose(); view.destroy(); surface.dispose(); client.close(); store.dispose(); root.remove(); });
  const applied = async () => {
    transport.respond((env) => env.kind === 'apply-song' ? [{ kind: 'song-candidate-ready', re: env.seq,
      body: { epoch: '9', doc_revision: env.body.doc_revision } }] : []);
    await controls.applyWholeCode();
    const request = transport.of('apply-song').at(-1)!;
    transport.emit({ kind: 'song-candidate-applied', re: request.seq,
      body: { epoch: '9', doc_revision: request.body.doc_revision, application_frame: '100' } });
    transport.emit({ kind: 'song-transport-state', body: { epoch: '9', state: 'playing', instruments: [selector] } });
    return request;
  };
  const text = () => root.querySelector('[role="status"]')?.textContent;
  return { root, transport, store, client, surface, controls, deps, applied, text };
}

describe('song controls', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => { for (const cleanup of cleanups.splice(0)) cleanup(); vi.useRealTimers(); });

  it('sends whole code explicitly and displays playback only after actual Applied', async () => {
    const f = fixture();
    expect(f.transport.sent).toEqual([]);
    f.transport.respond((env) => env.kind === 'apply-song' ? [{ kind: 'song-candidate-ready', re: env.seq,
      body: { epoch: '9', doc_revision: 1 } }] : []);
    await f.controls.applyWholeCode();
    expect(f.transport.of('apply-song')[0].body.code).toBe('song-code');
    expect(f.text()).toBe('Prepared revision 1');
    f.transport.emit({ kind: 'song-candidate-applied', re: 1,
      body: { epoch: '9', doc_revision: 1, application_frame: '9007199254740993' } });
    expect(f.text()).toBe('playing revision 1');
  });

  it('keeps editing separate and immediately sends pending-candidate invalidation', async () => {
    const f = fixture();
    await f.applied();
    f.transport.respond((env) => env.kind === 'apply-song' ? [{ kind: 'song-candidate-ready', re: env.seq,
      body: { epoch: '10', doc_revision: 1 } }] : []);
    await f.controls.applyWholeCode();
    const pending = f.transport.of('apply-song').at(-1)!;
    f.surface.dispatch({ changes: { from: 0, insert: '# draft\n' } });
    expect(f.store.song(file)?.pending).toBeUndefined();
    expect(f.transport.kinds()).toEqual(['apply-song', 'apply-song', 'doc-changed']);
    expect(f.text()).toBe('playing revision 1 — draft changed');
    f.transport.emit({ kind: 'song-candidate-applied', re: pending.seq,
      body: { epoch: '10', doc_revision: 1, application_frame: '200' } });
    expect(f.store.song(file)?.applied?.epoch).toBe('9');
    expect(f.transport.of('apply-song')).toHaveLength(2);
  });

  it('retains active playback and controls when a later candidate fails', async () => {
    const f = fixture();
    await f.applied();
    f.transport.respond((env) => env.kind === 'apply-song' ? [{ kind: 'song-candidate-failed', re: env.seq,
      body: { epoch: null, doc_revision: 1, code: 'load', message: 'Sample missing' } }] : []);
    await expect(f.controls.applyWholeCode()).rejects.toThrow('Sample missing');
    expect(f.text()).toBe('playing revision 1');
    expect(f.root.querySelector('[role="alert"]')?.textContent).toBe('Sample missing');
    expect(f.root.querySelector('[aria-label="Song instruments"]')?.textContent).toBe('Mute bd');
  });

  it('uses the certified inventory and waits for exact mute/unmute acknowledgements', async () => {
    const f = fixture();
    await f.applied();
    const mute = f.controls.muteInstrument(selector, true);
    expect(f.root.querySelector('[aria-label="Song instruments"]')?.textContent).toBe('Mute bd');
    const request = f.transport.of('mute-instrument').at(-1)!;
    expect(request.body).toEqual({ epoch: '9', selector, muted: true });
    f.transport.emit({ kind: 'song-instrument-muted', re: request.seq,
      body: { epoch: '9', selector, muted: true, application_frame: '9007199254740993' } });
    await mute;
    expect(f.root.querySelector('[aria-label="Song instruments"]')?.textContent).toBe('Unmute bd');
    const unmute = f.controls.muteInstrument(selector, false);
    const unmuteRequest = f.transport.of('mute-instrument').at(-1)!;
    f.transport.emit({ kind: 'song-instrument-muted', re: unmuteRequest.seq,
      body: { epoch: '9', selector, muted: false, application_frame: '9007199254740994' } });
    await unmute;
    expect(f.root.querySelector('[aria-label="Song instruments"]')?.textContent).toBe('Mute bd');
    expect(f.transport.of('apply-song')).toHaveLength(1);
  });

  it('disables ended controls and disposes subscriptions without changing the document', async () => {
    const f = fixture();
    await f.applied();
    f.transport.emit({ kind: 'song-transport-state', body: { epoch: '9', state: 'ended', instruments: [selector] } });
    expect(f.root.querySelector<HTMLButtonElement>('[aria-pressed]')?.disabled).toBe(true);
    expect(f.text()).toBe('ended revision 1');
    const code = f.surface.state.doc.toString();
    f.controls.dispose();
    expect(f.deps.song).toBeUndefined();
    expect(f.root.querySelector('[data-song-controls]')).toBeNull();
    expect(f.surface.state.doc.toString()).toBe(code);
    await expect(f.controls.applyWholeCode()).rejects.toThrow('Code editor unavailable');
  });
});

describe('mounted song controls with the actual backend', () => {
  it('applies, carries mute across a draft replacement, unmutes and ends through real receipts', async () => {
    const session = await loadVactrWasm();
    const worklet = await loadVactrWasm();
    expect(session.call('session_init', 8000, 1024 * 1024)).toBe(1);
    expect(worklet.call('worklet_init', 8000, 1024 * 1024, 16)).toBe(1);
    const listeners: ((text: string) => void)[] = [];
    const requests: string[] = [];
    const transport: Transport = {
      send(text) { requests.push(text); session.callStr('session_apply', text); },
      onText(callback) { listeners.push(callback); },
      close() { listeners.length = 0; },
    };
    const store = new Store();
    const errors: unknown[] = [];
    const client = new Client(transport, { store, onError: (error) => errors.push(error) });
    client.onDecodeError((error) => errors.push(error));
    const root = document.createElement('div');
    document.body.appendChild(root);
    buildLayout(root);
    const code = 'inst tone freq: float = 440:\n\tsin-osc freq > * amp\nsong {part [tone: {s :tone > gain 0.1}] duration: 8} tail-seconds: 0 > play-song';
    const sync = new DocumentSync(client.document(file), Text.of(code.split('\n')));
    const surface = new CodeSurface({ sync });
    const view = new EditorView({ state: surface.state });
    const deps: EditorDeps = { client, store, files: new MemoryFiles(), clock: new MockClock(), tier: 'browser',
      code: { surface, view, currentRevision: () => sync.revision, mapWireSpan: () => null,
        selectedSiteId: () => null, samples: { frames: () => null, openBrowser: () => {} } } };
    const controls = mount(root, deps, file);
    const pending: WasmRecord[] = [];
    const decoder = new TextDecoder();
    let frame = 0;
    const commands = (kind: string) => requests.filter((text) => {
      const decoded = decodeClient(text);
      expect(decoded.ok, text).toBe(true);
      return decoded.ok && decoded.env.kind === kind;
    });
    const status = () => root.querySelector('[role="status"]')?.textContent;
    const applyButton = () => root.querySelector<HTMLButtonElement>('[data-song-controls] > button')!;
    const muteButton = () => root.querySelector<HTMLButtonElement>('[aria-label="Song instruments"] button')!;
    async function step() {
      for (const record of session.drainRecords()) {
        if (record.tag === TAG_SESSION) {
          const text = decoder.decode(record.bytes);
          for (const listener of listeners) listener(text);
        } else if (record.tag < 0x40) pending.push(record);
      }
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
      const pcm = new Float32Array((worklet.exports.memory as WebAssembly.Memory).buffer, ptr, 128).slice();
      const records = worklet.drainRecords();
      expect(records.some((record) => record.tag === 0x60)).toBe(false);
      const feedback = new Uint8Array(records.reduce((n, record) => n + record.bytes.length + 5, 0));
      let offset = 0;
      for (const record of records) {
        new DataView(feedback.buffer).setUint32(offset, record.bytes.length + 1, true);
        feedback[offset + 4] = record.tag;
        feedback.set(record.bytes, offset + 5);
        offset += record.bytes.length + 5;
      }
      session.withBytes(feedback, (p, n) => session.call('session_inbox', p, n));
      session.call('session_tick', worklet.call('worklet_now'));
      const start = frame;
      frame += 128;
      await Promise.resolve();
      expect(errors).toEqual([]);
      expect(store.song(file)?.failure).toBeUndefined();
      return { start, pcm };
    }
    async function until(predicate: () => boolean) {
      for (let turn = 0; turn < 2048; turn++) {
        const block = await step();
        if (predicate()) return block;
      }
      throw new Error(`actual backend did not reach requested UI state: ${status()}`);
    }
    try {
      client.subscribe({ telemetry: true, levels: false, diagnostics: true });
      applyButton().click();
      expect(commands('apply-song')).toHaveLength(1);
      expect(status()).toContain('Preparing');
      await until(() => !!store.song(file)?.applied);
      const old = store.song(file)!.applied!;
      expect(status()).toBe('playing revision 1');
      let audible = false;
      for (let turn = 0; turn < 32; turn++) {
        const block = await step();
        audible = block.pcm.some((v) => Math.abs(v) > 1e-4) || audible;
      }
      expect(audible).toBe(true);
      muteButton().click();
      expect(commands('mute-instrument')).toHaveLength(1);
      await until(() => store.song(file)?.instrumentMutes?.some((mute) => mute.muted) === true);
      expect(muteButton().getAttribute('aria-pressed')).toBe('true');
      const muteFrame = BigInt(store.song(file)!.instrumentMutes![0].application_frame);
      for (let turn = 0; turn < 16; turn++) {
        const block = await step();
        if (BigInt(block.start) >= muteFrame + 128n)
          expect(block.pcm.every((v) => Math.abs(v) < 1e-7)).toBe(true);
      }
      const nextCode = code.replace('duration: 8', 'duration: 2');
      surface.dispatch({ changes: { from: 0, to: surface.state.doc.length, insert: nextCode } });
      expect(commands('apply-song')).toHaveLength(1);
      expect(store.song(file)!.applied!.epoch).toBe(old.epoch);
      expect(status()).toContain('draft changed');
      applyButton().click();
      expect(commands('apply-song')).toHaveLength(2);
      await until(() => store.song(file)?.applied?.epoch !== old.epoch);
      expect(status()).toBe('playing revision 2');
      expect(muteButton().getAttribute('aria-pressed')).toBe('true');
      for (let turn = 0; turn < 8; turn++)
        expect((await step()).pcm.every((v) => Math.abs(v) < 1e-7)).toBe(true);
      muteButton().click();
      await until(() => muteButton().getAttribute('aria-pressed') === 'false');
      audible = false;
      // Unmute permits future onsets; it does not resurrect the muted first note.
      for (let turn = 0; turn < 256 && !audible; turn++) {
        const block = await step();
        audible = block.pcm.some((v) => Math.abs(v) > 1e-4) || audible;
      }
      expect(audible).toBe(true);
      await until(() => store.song(file)?.transport?.state === 'ended');
      expect(status()).toBe('ended revision 2');
      expect(muteButton().disabled).toBe(true);
      expect((await step()).pcm.every((v) => Math.abs(v) < 1e-7)).toBe(true);
    } finally {
      controls.dispose(); view.destroy(); surface.dispose(); client.close(); store.dispose(); root.remove();
    }
  }, 30000);
});
