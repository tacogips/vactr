import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Client } from '../../src/protocol/client';
import { decodeClient, decodeServer } from '../../src/protocol/envelope';
import { Store, songKey } from '../../src/protocol/store';
import { RecordingTransport } from '../support/recording';
import { WasmCore } from '../../src/protocol/wasm';
import { VactrHost } from '../../worklet/host.js';
import { FakeCore, fakeNode } from '../support/fake-core';
import { SampleLibrary, type DecodedAudio, type FetchLike } from '../../src/code/samples';

const file = 'song.vact';
const ready = { epoch: '18446744073709551615', doc_revision: 1 };
const applied = { ...ready, application_frame: '9007199254740993' };
const frame = (kind: string, body: unknown) => JSON.stringify({ v: 1, seq: 1, kind, body });

describe('song protocol consumer', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('preserves exact decimal epochs and frames and rejects noncanonical or overflowing values', () => {
    expect(decodeServer(frame('song-candidate-applied', applied)).ok).toBe(true);
    for (const epoch of [1, '01', '-1', '18446744073709551616', '1.0']) {
      expect(decodeServer(frame('song-candidate-ready', { ...ready, epoch })).ok).toBe(false);
    }
    expect(decodeServer(frame('song-candidate-applied', { ...applied, application_frame: 9007199254740992 })).ok).toBe(false);
    expect(decodeServer(frame('song-candidate-failed', { epoch: null, doc_revision: null, code: 'load', message: 'missing' })).ok).toBe(true);
  });

  it('validates selector families without silently accepting malformed or duplicate sounds', () => {
    const body = { epoch: ready.epoch, muted: true, selector: { family: [
      { kind: 'builtin', name: 'bd' }, { kind: 'instrument', id: 3 },
      { kind: 'sample', path: 'samples/kick.wav', file: null }, { kind: 'buffer', id: '9007199254740993' },
    ] } };
    expect(decodeClient(frame('mute-instrument', body)).ok).toBe(true);
    for (const family of [[], [{ kind: 'builtin', name: ':bd' }],
      [{ kind: 'instrument', id: -1 }], [{ kind: 'buffer', id: '00' }],
      [{ kind: 'sample', path: 'a\u0000b', file: null }],
      [{ kind: 'builtin', name: 'bd' }, { kind: 'builtin', name: 'bd' }],
      [{ kind: 'builtin', name: 'bd', extra: true }]]) {
      expect(decodeClient(frame('mute-instrument', { ...body, selector: { family } })).ok).toBe(false);
    }
  });

  it('flushes document edits before Apply and retains correlation through synchronous Ready to Applied', async () => {
    const transport = new RecordingTransport();
    const store = new Store();
    const client = new Client(transport, { store });
    client.document(file).edit([{ from: 0, to: 0, insert_len: 1 }], [{ start: 0, end: 1 }]);
    transport.respond((env) => env.kind === 'apply-song' ? [{ kind: 'song-candidate-ready', re: env.seq,
      body: { epoch: ready.epoch, doc_revision: env.body.doc_revision } }] : []);
    const response = await client.applySong(file, 'song-code');
    expect(transport.kinds()).toEqual(['doc-changed', 'apply-song']);
    expect(transport.of('apply-song')[0].body).toEqual({ file, code: 'song-code', doc_revision: 2, edit_epoch: 1 });
    expect(response.kind).toBe('song-candidate-ready');
    expect(store.song(file)?.pending?.ready).toBe(true);
    expect(store.song(file)?.applied).toBeUndefined();
    transport.emit({ kind: 'song-candidate-applied', re: response.re, body: { ...applied, doc_revision: 2 } });
    expect(store.song(file)?.applied?.application_frame).toBe('9007199254740993');
    expect(store.song(file)?.pending).toBeUndefined();
    client.close();
  });

  it('preserves active playback across failed candidates and ignores wrong request/revision/epoch receipts', () => {
    const store = new Store();
    store.beginSongApply(file, 1, 1);
    store.apply({ kind: 'song-candidate-applied', re: 1, body: applied });
    store.beginSongApply(file, 2, 2);
    store.apply({ kind: 'song-candidate-ready', re: 999, body: { epoch: '2', doc_revision: 2 } });
    store.apply({ kind: 'song-candidate-ready', re: 2, body: { epoch: '2', doc_revision: 3 } });
    expect(store.song(file)?.pending?.ready).toBe(false);
    store.apply({ kind: 'song-candidate-ready', re: 2, body: { epoch: '2', doc_revision: 2 } });
    store.apply({ kind: 'song-candidate-applied', re: 2, body: { epoch: '3', doc_revision: 2, application_frame: '4' } });
    expect(store.song(file)?.applied).toEqual(applied);
    store.apply({ kind: 'song-candidate-failed', re: 2, body: { epoch: '2', doc_revision: 2, code: 'load', message: 'missing' } });
    expect(store.song(file)?.applied).toEqual(applied);
    expect(store.song(file)?.pending).toBeUndefined();
    expect(store.song(file)?.failure?.code).toBe('load');
  });

  it('invalidates pending UI after a document revision changes without erasing active playback', () => {
    const store = new Store();
    store.beginSongApply(file, 1, 1);
    store.apply({ kind: 'song-candidate-applied', re: 1, body: applied });
    store.beginSongApply(file, 2, 2);
    store.songDocumentChanged(file, 3);
    store.apply({ kind: 'song-candidate-applied', re: 2, body: { epoch: '2', doc_revision: 2, application_frame: '4' } });
    expect(store.song(file)).toEqual({ draftRevision: 3, applied });
  });

  it('distinguishes documents with the same revision and ignores duplicate Applied receipts', () => {
    const store = new Store();
    const listener = vi.fn();
    store.beginSongApply(file, 1, 1);
    store.beginSongApply('other.vact', 1, 2);
    store.subscribe([songKey(file)], listener);
    store.apply({ kind: 'song-candidate-applied', re: 2, body: applied });
    expect(store.song(file)?.applied).toBeUndefined();
    expect(listener).not.toHaveBeenCalled();
    store.apply({ kind: 'song-candidate-applied', re: 1, body: applied });
    store.apply({ kind: 'song-candidate-applied', re: 1, body: applied });
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it('queues reentrant song updates until every subscriber sees the same completed state', () => {
    const store = new Store();
    const seen: number[] = [];
    store.subscribe([songKey(file)], () => {
      if (store.song(file)?.draftRevision === 1) store.songDocumentChanged(file, 2);
    });
    store.subscribe([songKey(file)], () => seen.push(store.song(file)!.draftRevision));
    store.beginSongApply(file, 1, 1);
    expect(seen).toEqual([1, 2]);
  });

  it('clears a refused outgoing candidate while preserving the last applied song', async () => {
    const transport = new RecordingTransport();
    const store = new Store();
    store.beginSongApply(file, 1, 99);
    store.apply({ kind: 'song-candidate-applied', re: 99, body: applied });
    const client = new Client(transport, { store });
    transport.respond(() => { throw new Error('transport unavailable'); });
    await expect(client.applySong(file, 'song-code')).rejects.toThrow('transport unavailable');
    expect(store.song(file)?.pending).toBeUndefined();
    expect(store.song(file)?.applied).toEqual(applied);
    client.close();
  });

  it('accepts only exact valid mute acknowledgements and known finite states', () => {
    const mute = { epoch: '1', selector: { family: [{ kind: 'builtin', name: 'bd' }] },
      muted: true, application_frame: '9007199254740993' };
    expect(decodeServer(frame('song-instrument-muted', mute)).ok).toBe(true);
    for (const change of [{ epoch: '01' }, { application_frame: 1 }, { selector: { family: [] } }, { muted: 'true' }]) {
      expect(decodeServer(frame('song-instrument-muted', { ...mute, ...change })).ok).toBe(false);
    }
    for (const state of ['prepared', 'playing', 'draining', 'ended', 'failed']) {
      expect(decodeServer(frame('song-transport-state', { epoch: ready.epoch, state })).ok).toBe(true);
    }
    expect(decodeServer(frame('song-transport-state', { epoch: '1', state: 'running' })).ok).toBe(false);
    expect(decodeServer(frame('song-transport-state', { epoch: '1', state: 'playing', instruments: [mute.selector] })).ok).toBe(true);
    expect(decodeServer(frame('song-transport-state', { epoch: '1', state: 'playing', instruments: [{ family: [] }] })).ok).toBe(false);
    expect(decodeServer(frame('song-transport-state', { epoch: '1', state: 'playing', instruments: 'bd' })).ok).toBe(false);
  });

  it('updates mute state only from actual current-epoch acknowledgements', () => {
    const transport = new RecordingTransport();
    const store = new Store();
    const client = new Client(transport, { store });
    store.beginSongApply(file, 1, 99);
    store.apply({ kind: 'song-candidate-applied', re: 99, body: applied });
    const selector = { family: [{ kind: 'builtin' as const, name: 'bd' }] };
    const request = client.muteInstrument(ready.epoch, selector, true);
    expect(store.song(file)?.instrumentMutes).toBeUndefined();
    transport.emit({ kind: 'song-instrument-muted', re: 1,
      body: { epoch: '1', selector, muted: true, application_frame: '12' } });
    expect(store.song(file)?.instrumentMutes).toBeUndefined();
    transport.emit({ kind: 'song-instrument-muted', re: 1,
      body: { epoch: ready.epoch, selector, muted: true, application_frame: '12' } });
    expect(store.song(file)?.instrumentMutes).toEqual([{ sound: selector.family[0], muted: true, application_frame: '12' }]);
    void request;
    client.close();
  });

  it('orders exact mute frames per sound, preserving unrelated members of overlapping families', () => {
    const store = new Store();
    store.beginSongApply(file, 1, 1);
    store.apply({ kind: 'song-candidate-applied', re: 1, body: applied });
    const bd = { kind: 'builtin' as const, name: 'bd' };
    const hh = { kind: 'builtin' as const, name: 'hh' };
    store.apply({ kind: 'song-instrument-muted', body: { epoch: ready.epoch, selector: { family: [bd, hh] },
      muted: true, application_frame: '9007199254740993' } });
    store.apply({ kind: 'song-instrument-muted', body: { epoch: ready.epoch, selector: { family: [bd] },
      muted: false, application_frame: '9007199254740994' } });
    store.apply({ kind: 'song-instrument-muted', body: { epoch: ready.epoch, selector: { family: [hh, bd] },
      muted: true, application_frame: '9007199254740992' } });
    expect(store.song(file)?.instrumentMutes).toEqual([
      { sound: bd, muted: false, application_frame: '9007199254740994' },
      { sound: hh, muted: true, application_frame: '9007199254740993' },
    ]);
  });

  it('retains active mute/state during draft edits and rejects stale finite progress', () => {
    const store = new Store();
    store.beginSongApply(file, 1, 1);
    store.apply({ kind: 'song-candidate-applied', re: 1, body: applied });
    store.apply({ kind: 'song-transport-state', body: { epoch: ready.epoch, state: 'playing' } });
    store.apply({ kind: 'song-instrument-muted', body: { epoch: ready.epoch,
      selector: { family: [{ kind: 'builtin', name: 'bd' }] }, muted: true, application_frame: '42' } });
    store.beginSongApply(file, 2, 2);
    store.songDocumentChanged(file, 3);
    expect(store.song(file)?.instrumentMutes?.[0].muted).toBe(true);
    expect(store.song(file)?.transport?.state).toBe('playing');
    store.apply({ kind: 'song-transport-state', body: { epoch: '1', state: 'failed' } });
    expect(store.song(file)?.transport?.state).toBe('playing');
    store.apply({ kind: 'song-transport-state', body: { epoch: ready.epoch, state: 'ended' } });
    store.apply({ kind: 'song-transport-state', body: { epoch: ready.epoch, state: 'prepared' } });
    expect(store.song(file)?.transport?.state).toBe('ended');
  });

  it('replaces old epoch status on Applied and ignores subsequent old mute/state receipts', () => {
    const store = new Store();
    store.beginSongApply(file, 1, 1);
    store.apply({ kind: 'song-candidate-applied', re: 1, body: applied });
    store.apply({ kind: 'song-transport-state', body: { epoch: ready.epoch, state: 'ended' } });
    store.beginSongApply(file, 2, 2);
    store.apply({ kind: 'song-candidate-applied', re: 2, body: { epoch: '2', doc_revision: 2, application_frame: '100' } });
    store.apply({ kind: 'song-transport-state', body: { epoch: ready.epoch, state: 'failed' } });
    store.apply({ kind: 'song-instrument-muted', body: { epoch: ready.epoch,
      selector: { family: [{ kind: 'builtin', name: 'bd' }] }, muted: true, application_frame: '101' } });
    expect(store.song(file)?.transport).toBeUndefined();
    expect(store.song(file)?.instrumentMutes).toBeUndefined();
  });

  it('correlates synchronous Apply even when a pending-state subscriber sends another request', async () => {
    const transport = new RecordingTransport();
    const store = new Store();
    const client = new Client(transport, { store });
    store.subscribe([songKey(file)], () => {
      client.hush();
    });
    transport.respond((env) => env.kind === 'apply-song'
      ? [{ kind: 'song-candidate-ready', re: env.seq, body: ready }] : []);
    expect((await client.applySong(file, 'song-code')).kind).toBe('song-candidate-ready');
    expect(store.song(file)?.pending?.request).toBe(transport.of('apply-song')[0].seq);
    expect(store.song(file)?.pending?.ready).toBe(true);
    expect(transport.envelopes.map((env) => env.seq)).toEqual([1, 2]);
    expect(transport.kinds()).toEqual(['apply-song', 'hush']);
    client.close();
  });
});

describe('browser song assets', () => {
  const pcm = (): DecodedAudio => ({ numberOfChannels: 1, sampleRate: 48000, length: 2,
    getChannelData: () => new Float32Array([0, 0.5]) });

  it('refreshes assets before actual Apply and passes complete catalogs through raw string ABI', () => {
    const fake = new FakeCore();
    const calls: string[] = [];
    const catalogs: unknown[] = [];
    fake.exports.session_song_refresh_assets = () => { calls.push('refresh'); return 1; };
    fake.exports.session_song_bank_catalog = (ptr: number, len: number) => {
      catalogs.push(JSON.parse(new TextDecoder().decode(new Uint8Array(fake.memory.buffer, ptr, len))));
      return 1;
    };
    fake.on('session_apply', () => { calls.push('apply'); return 1; });
    const core = new WasmCore();
    core.attach(new VactrHost(null, fakeNode(), fake.exports, { wasmUrl: '', processorUrl: '', init: 'session', onRecord: core.onRecord }));
    core.songBankCatalog({ bd: ['bd:0', 'bd:1'] });
    core.apply(frame('apply-song', { file, code: 'song-code', doc_revision: 1, edit_epoch: 0 }));
    expect(calls).toEqual(['refresh', 'apply']);
    expect(catalogs).toEqual([{ banks: [{ name: 'bd', members: ['bd:0', 'bd:1'] }] }]);
    fake.exports.session_song_refresh_assets = () => 0;
    expect(() => core.apply(frame('apply-song', { file, code: 'song-code', doc_revision: 1, edit_epoch: 0 }))).toThrow('preparation is unavailable');
    expect(calls).toEqual(['refresh', 'apply']);
  });

  it('never certifies partially loaded banks and records complete member order after uploads', async () => {
    let missing = true;
    const uploads: string[] = [];
    const catalogs: Record<string, string[]>[] = [];
    const fetch: FetchLike = async (url) => ({ ok: !url.endsWith('/missing') || !missing, status: 200,
      json: async () => ({ bd: ['good', 'missing'] }), arrayBuffer: async () => new ArrayBuffer(4) });
    const library = new SampleLibrary({ fetch, decode: async () => pcm(), core: {
      samplePut: (key) => { uploads.push(key); },
      songBankCatalog: (banks) => { catalogs.push(structuredClone(banks)); },
    } });
    await library.loadMap('map');
    expect(await library.loadBank('bd')).toBe(1);
    expect(catalogs.every((catalog) => catalog.bd === undefined)).toBe(true);
    missing = false;
    expect(await library.loadBank('bd')).toBe(2);
    expect(uploads).toEqual(['bd:0', 'bd:0', 'bd:1']);
    expect(catalogs.at(-1)).toEqual({ bd: ['bd:0', 'bd:1'] });
  });

  it('prevents an obsolete map decode from uploading or certifying a new map', async () => {
    let begin!: () => void;
    let finish!: (value: DecodedAudio) => void;
    const started = new Promise<void>((resolve) => { begin = resolve; });
    const decoded = new Promise<DecodedAudio>((resolve) => { finish = resolve; });
    const upload = vi.fn();
    const catalogs: Record<string, string[]>[] = [];
    const fetch: FetchLike = async () => ({ ok: true, status: 200,
      json: async () => ({ bd: ['good'] }), arrayBuffer: async () => new ArrayBuffer(4) });
    const library = new SampleLibrary({ fetch, decode: () => { begin(); return decoded; }, core: {
      samplePut: upload, songBankCatalog: (banks) => { catalogs.push(structuredClone(banks)); },
    } });
    await library.loadMap('old-map');
    const pending = library.loadBank('bd');
    await started;
    await library.loadMap('new-map');
    finish(pcm());
    await expect(pending).rejects.toThrow('Sample map changed');
    expect(upload).not.toHaveBeenCalled();
    expect(catalogs.at(-1)).toEqual({});
  });
});
