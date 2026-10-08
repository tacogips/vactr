import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Client, TWEAK_INTERVAL_MS } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import type { ServerEnvelope } from '../../src/protocol/types';
import { RecordingTransport } from '../support/recording';

const FILE = 'song.vact';
const manifest = { sounds: [], synths: [], controls: [] };

function edit(client: Client, at = 0): void {
  client.document(FILE).edit([{ from: at, to: at, insert_len: 1 }], [{ start: at, end: at + 1 }]);
}

describe('Client', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('increments seq and correlates replies by re', async () => {
    const t = new RecordingTransport();
    const client = new Client(t);
    client.hush();
    const a = client.manifest();
    const b = client.eval(FILE, 'x');
    expect(t.envelopes.map((e) => e.seq)).toEqual([1, 2, 3]);
    expect(t.envelopes.every((e) => e.v === 1)).toBe(true);
    // Replies arrive out of order; each resolves its own request.
    t.push({
      kind: 'eval-result',
      re: 3,
      body: { file: FILE, doc_revision: 1, forms: [], diagnostics: [], sites: [], directives: { file_level: {}, entries: [] } },
    });
    t.push({ kind: 'manifest', re: 2, body: manifest });
    t.deliver();
    expect((await a).kind).toBe('manifest');
    expect((await b).kind).toBe('eval-result');
  });

  it('resolves a synchronous reply sent during send (wasm transport)', async () => {
    const t = new RecordingTransport();
    t.respond((env) => (env.kind === 'manifest?' ? [{ kind: 'manifest', re: env.seq, body: manifest }] : []));
    const client = new Client(t);
    const env = await client.manifest();
    expect(env.re).toBe(1);
  });

  it('flushes a pending doc-changed before every write', () => {
    const t = new RecordingTransport();
    const client = new Client(t);
    edit(client);
    client.setTweak(FILE, 4, 1, 0.5);
    edit(client);
    client.setVar(FILE, 'amt', 2, 1);
    edit(client);
    void client.learn(FILE, 'hats.lpf', 74, 1);
    edit(client);
    void client.eval(FILE, 'xxxx');
    expect(t.kinds()).toEqual([
      'doc-changed',
      'set-tweak',
      'doc-changed',
      'set-var',
      'doc-changed',
      'learn',
      'doc-changed',
      'eval',
    ]);
    // Nothing pending: no extra doc-changed.
    t.clear();
    vi.advanceTimersByTime(1000);
    client.setVar(FILE, 'amt', 3, 1);
    expect(t.kinds()).toEqual(['set-var']);
  });

  it('stamps writes with the current edit epoch and revision', () => {
    const t = new RecordingTransport();
    const client = new Client(t);
    edit(client);
    edit(client, 1);
    client.setTweak(FILE, 4, 7, 0.25);
    void client.eval(FILE, 'ab', { start: 0, end: 2 });
    const [changed] = t.of('doc-changed');
    expect(changed?.body).toMatchObject({ doc_revision: 3, base_revision: 1, edit_epoch: 2 });
    expect(t.of('set-tweak')[0]?.body).toEqual({ file: FILE, id: 4, form_gen: 7, value: 0.25, edit_epoch: 2 });
    expect(t.of('eval')[0]?.body).toEqual({
      file: FILE,
      code: 'ab',
      span: { start: 0, end: 2 },
      doc_revision: 3,
      edit_epoch: 2,
    });
  });

  it('rate-limits setTweak per target, latest wins, trailing value sent', () => {
    const t = new RecordingTransport();
    const client = new Client(t);
    client.setTweak(FILE, 1, 1, 0.1);
    client.setTweak(FILE, 1, 1, 0.2);
    client.setTweak(FILE, 1, 1, 0.3);
    client.setTweak(FILE, 2, 1, 0.9); // another target is independent
    expect(t.of('set-tweak').map((e) => e.body.value)).toEqual([0.1, 0.9]);
    vi.advanceTimersByTime(TWEAK_INTERVAL_MS - 1);
    expect(t.of('set-tweak')).toHaveLength(2);
    vi.advanceTimersByTime(1);
    expect(t.of('set-tweak').map((e) => e.body.value)).toEqual([0.1, 0.9, 0.3]);
    // Nothing more queued: no further send.
    vi.advanceTimersByTime(100);
    expect(t.of('set-tweak')).toHaveLength(3);
    // After the interval a new value goes out at once.
    client.setTweak(FILE, 1, 1, 0.4);
    expect(t.of('set-tweak').map((e) => e.body.value)).toEqual([0.1, 0.9, 0.3, 0.4]);
  });

  it('sends stop-all with an empty body', () => {
    const t = new RecordingTransport();
    const client = new Client(t);
    client.stopAll();
    expect(t.of('stop-all').map((e) => e.body)).toEqual([{}]);
  });

  it('rate-limits momentary targets per site and sends the latest trailing target', () => {
    const t = new RecordingTransport();
    const client = new Client(t);
    edit(client);
    client.momentary(FILE, 4, 7, 0.1, 500);
    client.momentary(FILE, 4, 7, 0.2, 500);
    client.momentary(FILE, 4, 7, 0.3, 500);
    expect(t.of('momentary').map((e) => e.body.target)).toEqual([0.1]);
    vi.advanceTimersByTime(TWEAK_INTERVAL_MS - 1);
    expect(t.of('momentary')).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(t.of('momentary').map((e) => e.body.target)).toEqual([0.1, 0.3]);
    expect(t.of('momentary').map((e) => e.body.edit_epoch)).toEqual([1, 1]);
  });

  it('sends momentary release immediately and cancels the queued drag', () => {
    const t = new RecordingTransport();
    const client = new Client(t);
    client.momentary(FILE, 4, 7, 0.2, 400);
    client.momentary(FILE, 4, 7, 0.3, 400);
    client.momentary(FILE, 4, 7, null, 0);
    expect(t.of('momentary').map((e) => e.body.target)).toEqual([0.2, null]);
    vi.advanceTimersByTime(100);
    expect(t.of('momentary').map((e) => e.body.target)).toEqual([0.2, null]);
  });

  it('clamps momentary ramp duration to 10000 milliseconds', () => {
    const t = new RecordingTransport();
    const client = new Client(t);
    client.momentary(FILE, 4, 7, 0.5, 20000);
    expect(t.of('momentary')[0]?.body.ramp_ms).toBe(10000);
  });

  it('flushes an edit made while a trailing tweak is queued before sending it', () => {
    const t = new RecordingTransport();
    const client = new Client(t);
    client.setTweak(FILE, 1, 1, 0.1);
    client.setTweak(FILE, 1, 1, 0.2);
    edit(client);
    vi.advanceTimersByTime(TWEAK_INTERVAL_MS);
    expect(t.kinds()).toEqual(['set-tweak', 'doc-changed', 'set-tweak']);
    // Stamped with the epoch current when the value was produced.
    expect(t.of('set-tweak')[1]?.body.edit_epoch).toBe(0);
  });

  it('hands every message to the store and to kind listeners', () => {
    const t = new RecordingTransport();
    const store = new Store();
    const client = new Client(t, { store });
    const seen: ServerEnvelope[] = [];
    client.on('tempo', (e) => seen.push(e));
    t.emit({ kind: 'tempo', body: { bpm: 120, beats_per_cycle: 4, cycle: [0, 1] } });
    expect(store.tempo?.bpm).toBe(120);
    expect(seen).toHaveLength(1);
  });

  it('reports an undecodable frame without throwing', () => {
    const t = new RecordingTransport();
    const client = new Client(t);
    const errors: string[] = [];
    client.onDecodeError((e) => errors.push(e.code));
    t.emitRaw('{nope');
    t.emitRaw(JSON.stringify({ v: 2, seq: 1, kind: 'tempo', body: {} }));
    t.emitRaw(JSON.stringify({ v: 1, seq: 1, kind: 'mystery', body: {} }));
    t.emitRaw(JSON.stringify({ v: 1, seq: 1, kind: 'bindings', body: { pass: 1 } }));
    expect(errors).toEqual(['bad-json', 'unsupported-version', 'unknown-kind', 'bad-shape']);
  });

  it('rejects pending requests on close', async () => {
    const t = new RecordingTransport();
    const client = new Client(t);
    const p = client.manifest();
    client.close();
    await expect(p).rejects.toThrow('client closed');
    expect(t.closed).toBe(true);
  });
});
