// The ExternalFile set in the 14.5.8 format: round trip, key spellings,
// positional entries, overlays, and ignored unknown versions.

import { describe, expect, it } from 'vitest';
import { EditorBindingSet, identOf, isKeySpelling, Persistence } from '../../src/bind/persistence';

describe('EditorBindingSet (14.5.8)', () => {
  it('round-trips keys, panel membership, mappings and overlays', () => {
    const set = new EditorBindingSet();
    set.upsert({ key: 'hats.lpf.2.cutoff', panel: true, midi: { cc: 31, ch: 2 }, overlay: 1200 });
    set.upsert({ key: 'analog.cutoff', panel: true, midi: { cc: 1 } });
    set.upsert({ key: 'hats.hpf.1.cutoff', panel: false });
    set.upsert({ span: [10, 13], param: 'cutoff', panel: true, overlay: 0.25 });
    const json = set.toJson();
    expect(JSON.parse(json)).toEqual({
      v: 1,
      bindings: [
        { span: [10, 13], param: 'cutoff', panel: true, overlay: 0.25 },
        { key: 'analog.cutoff', panel: true, midi: { cc: 1 } },
        { key: 'hats.hpf.1.cutoff', panel: false },
        { key: 'hats.lpf.2.cutoff', panel: true, midi: { cc: 31, ch: 2 }, overlay: 1200 },
      ],
    });
    const back = EditorBindingSet.fromJson(json);
    expect(back.notices).toEqual([]);
    expect(back.set.entries()).toEqual(set.entries());
    expect(back.set.toJson()).toBe(json);
  });

  it('spells keys label.site.n.param or label.param', () => {
    expect(isKeySpelling('hats.lpf.2.cutoff')).toBe(true);
    expect(isKeySpelling('analog.cutoff')).toBe(true);
    expect(isKeySpelling('hats.lpf.cutoff')).toBe(false);
    expect(isKeySpelling('hats.lpf.0.cutoff')).toBe(false);
    expect(identOf({ span: [3, 7], param: 'q' })).toBe('@3:7.q');
  });

  it('ignores an unknown v with a notice', () => {
    const r = EditorBindingSet.fromJson('{"v": 2, "bindings": [{"key": "a.b", "panel": true}]}');
    expect(r.set.size).toBe(0);
    expect(r.notices).toEqual(['bindings file ignored: unsupported version 2']);
  });

  it('skips malformed entries and invalid JSON with notices', () => {
    const r = EditorBindingSet.fromJson(
      '{"v":1,"bindings":[{"key":"a.b","panel":true},{"key":"bad","panel":true},{"key":"a.c","panel":true,"midi":{"cc":200}}]}',
    );
    expect(r.set.entries().map((e) => e.key)).toEqual(['a.b']);
    expect(r.notices).toHaveLength(2);
    expect(EditorBindingSet.fromJson('{').notices[0]).toMatch(/ignored/);
  });

  it('renames migrated keys atomically', () => {
    const set = new EditorBindingSet();
    set.upsert({ key: 'hats.lpf.1.cutoff', panel: true, midi: { cc: 21 } });
    set.upsert({ key: 'hats.lpf.2.cutoff', panel: true, midi: { cc: 23 } });
    set.renameAll([
      ['hats.lpf.1.cutoff', 'hats.lpf.2.cutoff'],
      ['hats.lpf.2.cutoff', 'hats.lpf.3.cutoff'],
    ]);
    expect(set.get('hats.lpf.2.cutoff')?.midi?.cc).toBe(21);
    expect(set.get('hats.lpf.3.cutoff')?.midi?.cc).toBe(23);
    expect(set.get('hats.lpf.1.cutoff')).toBeUndefined();
  });

  it('switching into ExternalFile takes the panel snapshot; the default is directive', () => {
    const p = new Persistence();
    expect(p.mode).toBe('directive');
    p.setMode('external-file', () => [{ key: 'a.b', panel: true, midi: { cc: 4 } }]);
    expect(p.mode).toBe('external-file');
    expect(p.set.get('a.b')?.midi).toEqual({ cc: 4 });
  });
});
