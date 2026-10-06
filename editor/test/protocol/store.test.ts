import { describe, expect, it } from 'vitest';
import { Store, nameKey, siteKey, slotKey } from '../../src/protocol/store';
import type { BindingsBody, Diagnostic, EvalResultBody, WireSite } from '../../src/protocol/types';

const site = (id: number, value: number, extra: Partial<WireSite> = {}): WireSite => ({
  id,
  span: { start: id * 10, end: id * 10 + 3 },
  tier: 'direct',
  origin: 'pattern-literal',
  value,
  form_gen: 1,
  ...extra,
});

const diag = (slot: string, message: string): Diagnostic => ({
  code: 'runtime',
  severity: 'error',
  message,
  span: { start: 0, end: 1 },
  file: 'a.vact',
  slot,
});

function evalResult(file: string, sites: WireSite[]): EvalResultBody {
  return {
    file,
    doc_revision: 1,
    forms: [],
    diagnostics: [],
    sites,
    directives: { file_level: {}, entries: [] },
  };
}

function bindings(b: Partial<BindingsBody>): BindingsBody {
  return { pass: 1, changed: [], sites: [], states: [], ...b };
}

describe('Store', () => {
  it('notifies each affected subscriber exactly once per message', () => {
    const store = new Store();
    let both = 0;
    let onlyA = 0;
    store.subscribe([nameKey('a'), nameKey('b'), siteKey(1)], () => (both += 1));
    store.subscribe([nameKey('a')], () => (onlyA += 1));
    store.apply({
      kind: 'bindings',
      body: bindings({
        changed: [
          { name: 'a', value: '1', form_gen: 2 },
          { name: 'b', value: '2', form_gen: 2 },
        ],
        sites: [site(1, 0.5)],
        states: [
          { name: 'a', state: 'ok', value: '1' },
          { name: 'b', state: 'ok', value: '2' },
        ],
      }),
    });
    expect(both).toBe(1);
    expect(onlyA).toBe(1);
  });

  it('does not notify an unrelated subscriber', () => {
    const store = new Store();
    let unrelated = 0;
    let tempo = 0;
    store.subscribe([nameKey('zzz'), siteKey(99), slotKey('d9')], () => (unrelated += 1));
    store.subscribe(['tempo'], () => (tempo += 1));
    store.apply({ kind: 'bindings', body: bindings({ changed: [{ name: 'a', value: '1', form_gen: 1 }] }) });
    store.apply({ kind: 'levels', body: { levels: [{ source: ':master', rms: 0.1 }] } });
    store.apply({ kind: 'diag', body: { add: [diag('d1', 'x')], clear: [] } });
    expect(unrelated).toBe(0);
    expect(tempo).toBe(0);
    store.apply({ kind: 'tempo', body: { bpm: 90, beats_per_cycle: 4, cycle: [1, 1] } });
    expect(tempo).toBe(1);
  });

  it('indexes 10,000 subscriptions and visits only affected subscribers once', () => {
    const store = new Store();
    const seen: string[] = [];
    for (let i = 0; i < 10_000; i += 1) store.subscribe([nameKey(`k${i}`)], () => seen.push(`k${i}`));
    store.subscribe([nameKey('k7'), nameKey('also-k7')], () => seen.push('multi'));
    const before = store.stats.notifyVisits;
    store.apply({ kind: 'bindings', body: bindings({ changed: [{ name: 'k7', value: '7', form_gen: 1 }] }) });
    expect(store.stats.notifyVisits - before).toBe(2);
    expect(seen).toEqual(['k7', 'multi']);
    const control = new Store();
    let controlCalls = 0;
    control.subscribe([nameKey('a')], () => controlCalls++);
    control.subscribe([nameKey('b')], () => controlCalls++);
    control.subscribe([nameKey('c')], () => controlCalls++);
    control.apply({ kind: 'bindings', body: bindings({ changed: [
      { name: 'a', value: '1', form_gen: 1 }, { name: 'b', value: '2', form_gen: 1 }, { name: 'c', value: '3', form_gen: 1 },
    ] }) });
    expect(control.stats.notifyVisits).toBe(3);
    expect(controlCalls).toBe(3);
  });

  it('notifies indexed keys in registration order and snapshots subscriptions during callbacks', () => {
    const store = new Store();
    const seen: string[] = [];
    let unsubscribeLater = () => {};
    store.subscribe([nameKey('x')], () => {
      seen.push('first');
      store.subscribe([nameKey('x')], () => seen.push('new'));
      unsubscribeLater();
    });
    unsubscribeLater = store.subscribe([nameKey('y')], () => seen.push('second'));
    store.subscribe([nameKey('x')], () => seen.push('third'));
    store.apply({ kind: 'bindings', body: bindings({ changed: [
      { name: 'y', value: '2', form_gen: 1 }, { name: 'x', value: '1', form_gen: 1 },
    ] }) });
    expect(seen).toEqual(['first', 'third']);
  });

  it('applies a bindings batch atomically', () => {
    const store = new Store();
    store.apply({ kind: 'eval-result', body: evalResult('a.vact', [site(1, 0.1, { key: 'lead.lpf' })]) });
    const observed: unknown[] = [];
    store.subscribe([nameKey('a')], (changed, s) => {
      // Every part of the batch is visible together.
      observed.push({
        a: s.name('a'),
        b: s.name('b'),
        site: s.site(1),
        pass: s.pass,
        changed: [...changed].sort(),
      });
    });
    const failure = diag('', 'boom');
    store.apply({
      kind: 'bindings',
      body: bindings({
        pass: 7,
        changed: [{ name: 'a', value: '3', form_gen: 4 }],
        sites: [site(1, 0.9, { form_gen: 4 })],
        states: [
          { name: 'a', state: 'ok', value: '3' },
          { name: 'b', state: 'failed', value: '1', diagnostic: failure },
        ],
      }),
    });
    expect(observed).toEqual([
      {
        a: { state: 'ok', value: '3', form_gen: 4 },
        b: { state: 'failed', value: '1', diagnostic: failure },
        site: { ...site(1, 0.9, { form_gen: 4 }), key: 'lead.lpf' },
        pass: 7,
        changed: ['name:a', 'name:b', 'site:1', 'sites'],
      },
    ]);
  });

  it('clears a badge when the state returns to ok', () => {
    const store = new Store();
    store.apply({ kind: 'bindings', body: bindings({ states: [{ name: 'x', state: 'blocked', value: '0', blocked_on: 'y' }] }) });
    expect(store.name('x')).toEqual({ state: 'blocked', value: '0', blocked_on: 'y' });
    store.apply({ kind: 'bindings', body: bindings({ states: [{ name: 'x', state: 'ok', value: '5' }] }) });
    expect(store.name('x')).toEqual({ state: 'ok', value: '5' });
  });

  it('queues a message applied from inside a callback', () => {
    const store = new Store();
    const seen: string[] = [];
    store.subscribe([nameKey('a')], (_c, s) => {
      seen.push(`a:${s.name('a')?.value}:${s.name('b')?.value}`);
      if (s.name('b') === undefined) {
        s.apply({ kind: 'bindings', body: bindings({ changed: [{ name: 'b', value: '2', form_gen: 1 }] }) });
        seen.push(`after-nested:${s.name('b')?.value}`);
      }
    });
    store.subscribe([nameKey('b')], (_c, s) => seen.push(`b:${s.name('b')?.value}`));
    store.apply({ kind: 'bindings', body: bindings({ changed: [{ name: 'a', value: '1', form_gen: 1 }] }) });
    expect(seen).toEqual(['a:1:undefined', 'after-nested:undefined', 'b:2']);
  });

  it('replaces a file site table on eval-result and keeps other files', () => {
    const store = new Store();
    store.apply({ kind: 'eval-result', body: evalResult('a.vact', [site(1, 1), site(2, 2)]) });
    store.apply({ kind: 'eval-result', body: evalResult('b.vact', [site(5, 5)]) });
    const changed: string[][] = [];
    store.subscribe([siteKey(2)], (c) => changed.push([...c].filter((k) => k.startsWith('site:')).sort()));
    store.apply({ kind: 'eval-result', body: evalResult('a.vact', [site(3, 3)]) });
    expect(store.sitesOf('a.vact').map((s) => s.id)).toEqual([3]);
    expect(store.sitesOf('b.vact').map((s) => s.id)).toEqual([5]);
    expect(store.site(2)).toBeUndefined();
    expect(changed).toEqual([['site:1', 'site:2', 'site:3']]);
  });

  it('adds and clears runtime diagnostics per slot', () => {
    const store = new Store();
    const hits: string[][] = [];
    store.subscribe([slotKey('d1')], (c) => hits.push([...c].sort()));
    store.apply({ kind: 'diag', body: { add: [diag('d1', 'x'), diag('d2', 'y')], clear: [] } });
    expect(store.runtimeDiagnostics().get('d1')?.map((d) => d.message)).toEqual(['x']);
    store.apply({ kind: 'diag', body: { add: [], clear: [{ slot: 'd1' }] } });
    expect(store.runtimeDiagnostics().has('d1')).toBe(false);
    expect(hits).toEqual([
      ['diag', 'slot:d1', 'slot:d2'],
      ['diag', 'slot:d1'],
    ]);
  });

  it('keeps the latest levels, tempo and manifest including TASK-010 fields', () => {
    const store = new Store();
    store.apply({
      kind: 'levels',
      body: {
        levels: [{ source: ':master', rms: 0.2, bands: [1, 2, 3, 4, 5, 6, 7, 8] }],
        analyzers: [{ bus: 'drums', kind: 'spectrum', id: 900, cells: [0.1, 0.2] }],
      },
    });
    store.apply({
      kind: 'tempo',
      body: { bpm: 128, beats_per_cycle: 4, cycle: [3, 1], clock: { source: 'midi', locked: true } },
    });
    store.apply({
      kind: 'manifest',
      body: {
        sounds: ['bd'],
        synths: [],
        controls: [],
        editors: [
          {
            name: 'euclid',
            kind: 'euclid-ring',
            params: [{ name: 'hits', range: [0, 16], curve: 'stepped', unit: 'none', group: 0 }],
          },
        ],
      },
    });
    expect(store.levels?.analyzers?.[0]?.id).toBe(900);
    expect(store.tempo?.clock).toEqual({ source: 'midi', locked: true });
    expect(store.manifest?.editors?.[0]?.kind).toBe('euclid-ring');
  });

  it('ignores kinds it does not track', () => {
    const store = new Store();
    let calls = 0;
    store.subscribe(['sites', 'diag', 'levels', 'tempo'], () => (calls += 1));
    store.apply({ kind: 'playing', body: { events: [] } });
    store.apply({ kind: 'stale-binding', body: { target: 3, reason: 'stale-form-gen' } });
    expect(calls).toBe(0);
  });
});
