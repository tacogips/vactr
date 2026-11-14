// Criterion 7: multi-site BindingKey independence. `hats.lpf`/`hats.hpf`
// and `hats.lpf.1`/`hats.lpf.2` operate their own sites under interleaved
// CCs, persist and read back per key in both modes, survive a moved line
// (same keys, new spans), migrate with reordered ordinals, and a broken
// mapping shows STALE.

import { afterEach, describe, expect, it } from 'vitest';
import type { WireBinding, WireDirectives, WireSite } from '../../src/protocol/types';
import { cleanup, fakeMidi, noDirectives, q, setup, site, spanOf } from './fixtures';

afterEach(cleanup);

const HATS = 's [:hh] > lpf 800 > hpf 300 > lpf 2000 > d1  #@ hats: lpf hpf';
const TEXT = `${HATS}\ns [:bd] > gain 0.5 > d2`;

const KEYS = { lpf1: 'hats.lpf.1.cutoff', hpf1: 'hats.hpf.1.cutoff', lpf2: 'hats.lpf.2.cutoff' };

function hatSites(text: string, ids: [number, number, number]): WireSite[] {
  const call = (name: string, occ: number, ordinal: number) => ({
    name,
    head: spanOf(text, `${name} `, occ),
    ordinal,
    arg: 0,
    param: 'cutoff',
  });
  return [
    site(text, '800', ids[0], { key: KEYS.lpf1, call: call('lpf', 0, 1) }),
    site(text, '300', ids[1], { key: KEYS.hpf1, call: call('hpf', 0, 1) }),
    site(text, '2000', ids[2], { key: KEYS.lpf2, call: call('lpf', 1, 2) }),
  ];
}

function table(text: string, ccs: Record<string, number>): WireDirectives {
  const d = noDirectives();
  const directive = spanOf(text, '#@ hats: lpf hpf');
  d.bindings = Object.entries(ccs).map(
    ([key, cc]): WireBinding => ({ key, span: directive, param: 'cutoff', cc, directive }),
  );
  return d;
}

function rig(mode: 'directive' | 'external-file') {
  const h = setup(TEXT);
  const midi = fakeMidi();
  h.deps.midi = midi;
  h.area.router.sync();
  const ccs = { [KEYS.lpf1]: 21, [KEYS.hpf1]: 22, [KEYS.lpf2]: 23 };
  h.evalResult(hatSites(TEXT, [1, 2, 3]), { directives: mode === 'directive' ? table(TEXT, ccs) : noDirectives() });
  return { h, midi, ccs };
}

async function learnAll(h: ReturnType<typeof setup>, midi: ReturnType<typeof fakeMidi>, ccs: Record<string, number>) {
  for (const [key, cc] of Object.entries(ccs)) {
    const e = h.area.table.byKey(key);
    if (!e) throw new Error(`no ${key}`);
    const p = h.deps.bind?.learn(e.site.id);
    midi.cc(cc, 0, 1);
    await p;
  }
}

const ids = (h: ReturnType<typeof setup>) => h.transport.of('set-tweak').map((e) => e.body.id);

function interleave(midi: ReturnType<typeof fakeMidi>): void {
  for (const cc of [21, 22, 23, 21, 23, 22]) midi.cc(cc, 64, 1);
}

describe('multi-site BindingKey independence (criterion 7)', () => {
  it('Directive mode: interleaved CCs reach only their own sites', () => {
    const { h, midi } = rig('directive');
    h.transport.clear();
    interleave(midi);
    expect(ids(h)).toEqual([1, 2, 3, 1, 3, 2]);
  });

  it('ExternalFile mode: learned CCs reach only their own sites and read back per key', async () => {
    const { h, midi, ccs } = rig('external-file');
    h.area.setMode('external-file');
    await learnAll(h, midi, ccs);
    expect(h.transport.of('learn')).toEqual([]);
    h.transport.clear();
    interleave(midi);
    expect(ids(h)).toEqual([1, 2, 3, 1, 3, 2]);
    await h.area.save('song.vact');
    const sidecar = h.files.files.get('song.bindings.json') ?? '';
    const saved = JSON.parse(sidecar) as { v: number; bindings: { key?: string }[] };
    expect(saved.v).toBe(1);
    for (const [key, cc] of Object.entries(ccs)) {
      // The CC moves left overlays on the sites; ExternalFile keeps them.
      expect(saved.bindings.find((b) => b.key === key)).toMatchObject({ panel: true, midi: { cc, ch: 1 } });
    }

    // Read back into a fresh editor.
    const r = setup(TEXT);
    const m2 = fakeMidi();
    r.deps.midi = m2;
    r.area.router.sync();
    r.evalResult(hatSites(TEXT, [7, 8, 9]));
    r.area.setMode('external-file');
    expect(r.area.persistence.load(sidecar)).toEqual([]);
    r.transport.clear();
    interleave(m2);
    expect(ids(r)).toEqual([7, 8, 9, 7, 9, 8]);
  });

  it('Directive mode: the saved text reads back per key through the re-evaluated table', async () => {
    const { h } = rig('directive');
    await h.area.save('song.vact');
    const saved = h.files.files.get('song.vact') ?? '';
    const r = setup(saved);
    const m2 = fakeMidi();
    r.deps.midi = m2;
    r.area.router.sync();
    r.evalResult(hatSites(saved, [4, 5, 6]), { directives: table(saved, { [KEYS.lpf2]: 23, [KEYS.lpf1]: 21, [KEYS.hpf1]: 22 }) });
    interleave(m2);
    expect(ids(r)).toEqual([4, 5, 6, 4, 6, 5]);
  });

  for (const mode of ['directive', 'external-file'] as const) {
    it(`${mode}: moving the labeled line keeps the mappings (same keys, new spans)`, async () => {
      const { h, midi, ccs } = rig(mode);
      if (mode === 'external-file') {
        h.area.setMode('external-file');
        await learnAll(h, midi, ccs);
      }
      // Move the hats line below the bd line.
      h.view.dispatch({ changes: [{ from: 0, to: HATS.length + 1 }, { from: TEXT.length, insert: `\n${HATS}` }] });
      const moved = h.text();
      expect(moved).toBe(`s [:bd] > gain 0.5 > d2\n${HATS}`);
      expect(q(h.row(1), '.bind-state')).toBe('STALE');
      h.evalResult(hatSites(moved, [11, 12, 13]), { directives: mode === 'directive' ? table(moved, ccs) : noDirectives() });
      expect(q(h.row(11), '.bind-state')).toBe('');
      h.transport.clear();
      interleave(midi);
      expect(ids(h)).toEqual([11, 12, 13, 11, 13, 12]);
    });
  }

  it('reordering same-named sites (new ordinals) migrates the mappings', async () => {
    const { h, midi, ccs } = rig('external-file');
    h.area.setMode('external-file');
    await learnAll(h, midi, ccs);
    // A new lpf before the others: 800 becomes lpf.2, 2000 becomes lpf.3.
    h.edit('s [:hh] > ', 's [:hh] > lpf 100 > ');
    const next = h.text();
    const call = (name: string, occ: number, ordinal: number) => ({ name, head: spanOf(next, `${name} `, occ), ordinal, arg: 0, param: 'cutoff' });
    h.evalResult([
      site(next, '100', 20, { key: 'hats.lpf.1.cutoff', call: call('lpf', 0, 1) }),
      site(next, '800', 21, { key: 'hats.lpf.2.cutoff', call: call('lpf', 1, 2) }),
      site(next, '300', 22, { key: 'hats.hpf.1.cutoff', call: call('hpf', 0, 1) }),
      site(next, '2000', 23, { key: 'hats.lpf.3.cutoff', call: call('lpf', 2, 3) }),
    ]);
    const set = h.area.persistence.set;
    expect(set.get('hats.lpf.2.cutoff')?.midi?.cc).toBe(21);
    expect(set.get('hats.lpf.3.cutoff')?.midi?.cc).toBe(23);
    expect(set.get('hats.lpf.1.cutoff')).toBeUndefined();
    h.transport.clear();
    interleave(midi);
    expect(ids(h)).toEqual([21, 22, 23, 21, 23, 22]);
  });

  it('a broken mapping shows STALE', () => {
    const { h, midi, ccs } = rig('directive');
    h.edit('300', '350');
    expect(q(h.row(2), '.bind-state')).toBe('STALE');
    // The session no longer resolves the key.
    const next = h.text();
    h.evalResult(
      [site(next, '800', 1, { key: KEYS.lpf1 }), site(next, '2000', 3, { key: KEYS.lpf2 })],
      { directives: table(next, ccs) },
    );
    const broken = h.area.table.all().find((e) => e.key === KEYS.hpf1);
    expect(broken?.state).toBe('stale');
    expect(h.area.panel.row(broken?.bindingId ?? '')?.dataset.state).toBe('stale');
    h.transport.clear();
    midi.cc(22, 64, 1);
    expect(h.transport.of('set-tweak')).toEqual([]);
    midi.cc(21, 64, 1);
    expect(ids(h)).toEqual([1]);
  });
});
