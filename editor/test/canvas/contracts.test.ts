import { describe, expect, expectTypeOf, it } from 'vitest';
import { EditorState } from '@codemirror/state';
import type { CodeApi, CodeAnnotation, CodeSurface, ResourceBudget } from '../../src/app/apis';
import { Client } from '../../src/protocol/client';
import { decodeClient, decodeServer, MAX_TELEMETRY_BYTES } from '../../src/protocol/envelope';
import { Store } from '../../src/protocol/store';
import type { ClockProbeReply, ServerMsg, TransportSample } from '../../src/protocol/types';
import { RecordingTransport } from '../support/recording';

const sample = (extra: Partial<TransportSample> = {}): TransportSample => ({
  epoch: 'run-1', sample_time: 10, cycle: [2, 1], bpm: 120, beats_per_cycle: 4,
  running: true, latency_seconds: 0.02, latency_kind: 'estimate', uncertainty_seconds: 0.003, ...extra,
});
const tempo = (transport?: TransportSample): ServerMsg => ({ kind: 'tempo', body: {
  bpm: 120, beats_per_cycle: 4, cycle: [0, 1], ...(transport ? { transport } : {}),
} });
const event = { slot: 'kick', beat: [0, 1], time: 10, dur: [1, 4] };
const frame = (kind: string, body: unknown, extra = {}): string => JSON.stringify({ v: 1, seq: 1, kind, body, ...extra });
const manifest = { sounds: ['kick'], synths: [], controls: [] };

describe('canvas predecessor contracts', () => {
  it('exports headless state and annotation descriptors without a runtime view', () => {
    const state = EditorState.create({ doc: '日本語' });
    const annotation: CodeAnnotation = { from: 0, to: 3, kind: 'playing' };
    expect(state.doc.sliceString(annotation.from, annotation.to)).toBe('日本語');
    expectTypeOf<CodeSurface['state']>().toEqualTypeOf<EditorState>();
    expectTypeOf<CodeApi['surface']>().toEqualTypeOf<CodeSurface | undefined>();
    expectTypeOf<ResourceBudget['reserve']>().toEqualTypeOf<(bytes: number) => boolean>();
  });

  it('preserves v1 legacy envelopes and marks legacy tempo unsynchronized', () => {
    const d = decodeServer(frame('playing', { events: [event] }));
    expect(d).toEqual({ ok: true, env: { v: 1, seq: 1, kind: 'playing', body: { events: [event] } } });
    const store = new Store(); store.apply(tempo());
    expect(store.tempo?.bpm).toBe(120); expect(store.synchronized).toBe(false);
  });

  it('retains processing samples, original end time, epoch and revision identity', () => {
    const transport = sample();
    const src = { file: 'song', span: { start: 0, end: 9 }, doc_revision: 7, form_gen: 2 };
    const ev = { ...event, epoch: 'run-1', end_time: 10.5, src };
    expect(decodeServer(frame('playing', { events: [ev] }))).toMatchObject({ ok: true, env: { body: { events: [ev] } } });
    expect(decodeServer(frame('tempo', { ...tempo().body, transport }))).toMatchObject({ ok: true, env: { body: { transport } } });
    const store = new Store(); store.apply(tempo(transport));
    expect(store.transportSample).toEqual(transport); expect(store.synchronized).toBe(true);
  });

  it.each([
    { time: -1 }, { end_time: 9 }, { beat: [0, 0] }, { dur: [-1, 2] }, { dur: [1, -1] },
    { epoch: '' }, { src: { file: 's', doc_revision: 1, form_gen: 1, span: { start: 5, end: 1 } } },
    { src: { file: 's', doc_revision: 1.5, form_gen: 1, span: { start: 0, end: 1 } } },
  ])('rejects malformed playing fields %j', (bad) => {
    expect(decodeServer(frame('playing', { events: [{ ...event, ...bad }] })).ok).toBe(false);
  });

  it.each([
    { bpm: 0 }, { beats_per_cycle: -1 }, { sample_time: -1 }, { cycle: [0, 0] },
    { latency_seconds: -0.1 }, { latency_kind: 'invented' }, { latency_kind: ['estimate'] }, { latency_kind: { toString: 1, valueOf: 1 } }, { uncertainty_seconds: -1 },
    { latency_kind: 'measured', latency_seconds: null }, { latency_kind: 'unavailable', latency_seconds: 0.1 },
    { epoch: '' }, { running: 1 },
  ])('rejects malformed transport fields %j', (bad) => {
    expect(decodeServer(frame('tempo', { ...tempo().body, transport: { ...sample(), ...bad } })).ok).toBe(false);
  });

  it('rejects nonfinite JSON numbers and invalid levels', () => {
    expect(decodeServer('{"v":1,"seq":1,"kind":"tempo","body":{"bpm":1e309,"beats_per_cycle":4,"cycle":[0,1]}}').ok).toBe(false);
    expect(decodeServer(frame('levels', { levels: [{ source: 'master', rms: -1 }] })).ok).toBe(false);
    expect(decodeServer(frame('levels', { levels: [], time: 1.5, epoch: 'run-1' })).ok).toBe(true);
    expect(decodeServer(frame('levels', { levels: [], time: -1 })).ok).toBe(false);
    expect(decodeServer(frame('levels', { levels: [], epoch: '' })).ok).toBe(false);
    expect(decodeServer(frame('tempo', tempo().body, { v: { toString: 1, valueOf: 1 } })).ok).toBe(false);
  });

  it('accepts 4096 events, rejects 4097 and enforces UTF8 byte ceiling', () => {
    expect(decodeServer(frame('playing', { events: Array.from({ length: 4096 }, () => event) })).ok).toBe(true);
    expect(decodeServer(frame('playing', { events: Array.from({ length: 4097 }, () => event) })).ok).toBe(false);
    const base = frame('playing', { events: [], padding: '' });
    const available = MAX_TELEMETRY_BYTES - new TextEncoder().encode(base).length;
    const padding = '日'.repeat(Math.floor(available / 3)) + 'a'.repeat(available % 3);
    const exact = frame('playing', { events: [], padding });
    expect(new TextEncoder().encode(exact).length).toBe(MAX_TELEMETRY_BYTES);
    expect(decodeServer(exact).ok).toBe(true);
    expect(decodeServer(frame('playing', { events: [], padding: padding + 'a' })).ok).toBe(false);
    expect(decodeServer(frame('manifest', { ...manifest, padding: 'a'.repeat(MAX_TELEMETRY_BYTES) })).ok).toBe(true);
  });

  it('validates probe roundtrip descriptors and host correlation', async () => {
    const reply: ClockProbeReply = { page_send: 1, engine_receive: 2, engine_send: 2.01, epoch: 'run-1',
      correlation: { engine_time: 2, output_time: 1.98 }, latency_seconds: 0.02,
      latency_kind: 'measured', uncertainty_seconds: 0.001 };
    expect(decodeClient(frame('clock-probe', { page_send: 1 })).ok).toBe(true);
    expect(decodeClient(frame('clock-probe', { page_send: -1 })).ok).toBe(false);
    expect(decodeServer(frame('clock-probe', reply)).ok).toBe(true);
    expect(decodeServer(frame('clock-probe', { ...reply, engine_send: 1.9 })).ok).toBe(false);
    expect(decodeServer(frame('clock-probe', { ...reply, correlation: { engine_time: -1, output_time: 2 } })).ok).toBe(false);
    const t = new RecordingTransport();
    t.respond((e) => e.kind === 'clock-probe' ? [{ kind: 'clock-probe', re: e.seq, body: reply }] : []);
    const client = new Client(t); expect((await client.clockProbe(1)).body).toEqual(reply); client.close();
  });

  it('rejects request65 visibly and frees capacity on replies; synchronous replies work', async () => {
    const t = new RecordingTransport(); const client = new Client(t);
    const pending = Array.from({ length: 64 }, () => client.manifest());
    await expect(client.manifest()).rejects.toThrow('busy'); expect(t.sent).toHaveLength(64);
    t.emit({ kind: 'manifest', re: 1, body: manifest }); await pending[0];
    const next = client.manifest(); expect(t.sent).toHaveLength(65);
    for (let i = 2; i <= 65; i++) t.emit({ kind: 'manifest', re: i, body: manifest });
    await Promise.all([...pending, next]); expect(client.queueStats.pendingRequests).toBe(0);
    t.respond((e) => [{ kind: 'manifest', re: e.seq, body: manifest }]);
    expect((await client.manifest()).kind).toBe('manifest'); client.close();
  });

  it('bounds client telemetry flood and preserves eval, ack and error replies', async () => {
    const t = new RecordingTransport(); const store = new Store(); const client = new Client(t, { store });
    const a = client.eval('s', ''); const b = client.manifest(); const c = client.manifest();
    const seen: string[] = []; client.on('*', (e) => seen.push(e.kind));
    client.on('tempo', () => {
      if (seen.length > 1) return;
      for (let i = 0; i < 100; i++) t.emit({ kind: 'playing', body: { events: [{ ...event, beat: [0, 1], dur: [1, 4], time: i }] } });
      for (let i = 0; i < 100; i++) t.emit({ kind: 'levels', body: { levels: [{ source: 'master', rms: i }] } });
      t.emit({ kind: 'eval-result', re: 1, body: { file: 's', doc_revision: 1, forms: [], sites: [], diagnostics: [], directives: { file_level: {}, entries: [] } } });
      t.emit({ kind: 'manifest', re: 2, body: manifest });
      t.emit({ kind: 'protocol-error', re: 3, body: { code: 'bad-body', message: 'test' } });
      expect(client.queueStats.telemetryQueued).toBe(64);
      expect(client.queueStats.dropped).toBe(37); expect(client.queueStats.coalesced).toBe(99);
    });
    t.emit(tempo());
    expect((await a).kind).toBe('eval-result'); expect((await b).kind).toBe('manifest'); expect((await c).kind).toBe('protocol-error');
    expect(store.levels?.levels[0]?.rms).toBe(99); expect(seen.filter((s) => s === 'playing')).toHaveLength(63);
    client.close();
  });

  it('bounds store reentrant telemetry and retains control batches atomically', () => {
    const store = new Store(); let flooded = false;
    store.subscribe(['tempo'], () => {
      if (flooded) return; flooded = true;
      for (let i = 0; i < 100; i++) store.apply({ kind: 'playing', body: { events: [] } });
      for (let i = 0; i < 100; i++) store.apply({ kind: 'levels', body: { levels: [{ source: 'master', rms: i }] } });
      store.apply({ kind: 'manifest', body: manifest });
      expect(store.queueStats).toEqual({ telemetryQueued: 64, dropped: 37, coalesced: 99 });
    });
    store.apply(tempo()); expect(store.manifest).toEqual(manifest); expect(store.levels?.levels[0]?.rms).toBe(99);
  });

  it('rejects stale samples within epoch, reanchors new epoch and clears legacy timing', () => {
    const store = new Store(); store.apply(tempo(sample())); store.apply(tempo(sample({ sample_time: 9 })));
    expect(store.transportSample?.sample_time).toBe(10);
    store.apply(tempo(sample({ epoch: 'run-2', sample_time: 1 }))); expect(store.transportSample?.sample_time).toBe(1);
    store.apply(tempo()); expect(store.transportSample).toBeNull(); expect(store.synchronized).toBe(false);
  });

  it('coalescing never replaces a newer queued sample with an older one', () => {
    const store = new Store(); let first = true;
    store.subscribe(['tempo'], () => { if (!first) return; first = false;
      store.apply(tempo(sample({ sample_time: 12 }))); store.apply(tempo(sample({ sample_time: 11 })));
    });
    store.apply(tempo(sample())); expect(store.transportSample?.sample_time).toBe(12);
  });

  it('client coalescing preserves the newest same-epoch transport sample', () => {
    const t = new RecordingTransport(); const store = new Store(); const client = new Client(t, { store });
    let first = true;
    client.on('tempo', () => { if (!first) return; first = false;
      t.emit(tempo(sample({ sample_time: 12 }))); t.emit(tempo(sample({ sample_time: 11 })));
    });
    t.emit(tempo(sample())); expect(store.transportSample?.sample_time).toBe(12); client.close();
  });

  it('listener failure cannot strand queued control replies', async () => {
    const t = new RecordingTransport(); const errors: unknown[] = [];
    const client = new Client(t, { onError: (e) => errors.push(e) }); const pending = client.manifest();
    client.on('tempo', () => { t.emit({ kind: 'manifest', re: 1, body: manifest }); throw new Error('listener'); });
    t.emit(tempo()); expect((await pending).kind).toBe('manifest'); expect(errors).toHaveLength(1); client.close();
  });

  it('failed synchronous send releases pending request capacity', async () => {
    const t = new RecordingTransport(); t.send = () => { throw new Error('send failed'); };
    const client = new Client(t); await expect(client.manifest()).rejects.toThrow('send failed');
    expect(client.queueStats.pendingRequests).toBe(0); client.close();
  });

  it('disposal during notification cancels remaining subscribers', () => {
    const store = new Store(); let calls = 0;
    store.subscribe(['tempo'], () => store.dispose()); store.subscribe(['tempo'], () => calls++);
    store.apply(tempo()); expect(calls).toBe(0);
    const t = new RecordingTransport(); const client = new Client(t);
    client.on('tempo', () => client.close()); client.on('tempo', () => calls++);
    t.emit(tempo()); expect(calls).toBe(0);
  });

  it('unsubscribes and disposes new listeners and store state', () => {
    const t = new RecordingTransport(); const client = new Client(t); let errors = 0; let messages = 0;
    const off = client.onDecodeError(() => errors++); const offMessage = client.on('*', () => messages++);
    t.emitRaw('bad'); off(); t.emitRaw('bad'); offMessage(); t.emit(tempo());
    expect(errors).toBe(1); expect(messages).toBe(0); client.close(); t.emit(tempo());
    const store = new Store(); let calls = 0; store.subscribe(['tempo'], () => calls++); store.apply(tempo(sample()));
    store.dispose(); store.apply(tempo(sample())); expect(calls).toBe(1); expect(store.transportSample).toBeNull();
  });
});
