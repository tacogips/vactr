// CC routing: channel filtering, the file-default channel, value scaling,
// positional matching and ExternalFile routing.

import { afterEach, describe, expect, it } from 'vitest';
import type { WireBinding, WireDirectives } from '../../src/protocol/types';
import { cleanup, EDITORS, fakeMidi, noDirectives, setup, site, spanOf } from './fixtures';

afterEach(cleanup);

const TEXT = 's [:hh] > lpf 800 > gain 0.5 > d1';

function withBindings(bindings: WireBinding[], midiCh?: number): WireDirectives {
  const d = noDirectives();
  d.bindings = bindings;
  if (midiCh !== undefined) d.file_level.midi_ch = midiCh;
  return d;
}

function sites() {
  const call = { name: 'lpf', head: spanOf(TEXT, 'lpf'), ordinal: 1, arg: 0, param: 'cutoff' };
  return [site(TEXT, '800', 1, { key: 'hats.lpf.1.cutoff', call }), site(TEXT, '0.5', 2)];
}

const binding = (extra: Partial<WireBinding>): WireBinding => ({
  span: spanOf(TEXT, 'lpf 800'),
  param: 'cutoff',
  directive: { start: 0, end: 0 },
  ...extra,
});

function rig(directives: WireDirectives) {
  const h = setup(TEXT, { editors: EDITORS });
  const midi = fakeMidi();
  h.deps.midi = midi;
  h.area.router.sync();
  h.evalResult(sites(), { directives });
  h.transport.clear();
  const sent = () => h.transport.of('set-tweak').map((e) => [e.body.id, Math.round(e.body.value * 1000) / 1000]);
  return { h, midi, sent };
}

describe('CC routing', () => {
  it('honors a binding channel', () => {
    const { midi, sent } = rig(withBindings([binding({ key: 'hats.lpf.1.cutoff', cc: 74, ch: 5 })]));
    midi.cc(74, 127, 1);
    midi.cc(74, 127, 5);
    expect(sent()).toEqual([[1, 20000]]);
  });

  it('uses the file-default channel when the binding has none', () => {
    const { midi, sent } = rig(withBindings([binding({ key: 'hats.lpf.1.cutoff', cc: 74 })], 2));
    midi.cc(74, 0, 1);
    midi.cc(74, 0, 2);
    expect(sent()).toEqual([[1, 20]]);
  });

  it('listens on every channel when neither the binding nor the file sets one', () => {
    const { midi, sent } = rig(withBindings([binding({ key: 'hats.lpf.1.cutoff', cc: 74 })]));
    midi.cc(74, 0, 3);
    midi.cc(74, 0, 9);
    expect(sent()).toEqual([
      [1, 20],
      [1, 20],
    ]);
  });

  it('scales 0..127 onto the ParamMeta range and curve, else onto 0..1', () => {
    const d = withBindings([binding({ key: 'hats.lpf.1.cutoff', cc: 74 })]);
    const { h, midi, sent } = rig(d);
    midi.cc(74, 127);
    midi.cc(74, 0);
    // A mapping learned for the plain gain literal (no ParamMeta).
    h.area.persistence.setMode('external-file', () => h.area.router.snapshot());
    const gain = h.area.table.byId(2);
    if (!gain) throw new Error('no gain');
    const { entry } = h.area.router.ident(gain);
    h.area.persistence.set.upsert({ ...entry, panel: true, midi: { cc: 7 } });
    midi.cc(7, 64);
    expect(sent()).toEqual([
      [1, 20000],
      [1, 20],
      [2, 0.504],
    ]);
  });

  it('matches an unkeyed binding by span containment plus param', () => {
    const { midi, sent } = rig(withBindings([binding({ cc: 30 }), binding({ cc: 31, param: 'q' })]));
    midi.cc(30, 127);
    midi.cc(31, 127);
    expect(sent()).toEqual([[1, 20000]]);
  });

  it('routes ExternalFile mode through the editor set, not the directive table', () => {
    const { h, midi, sent } = rig(withBindings([binding({ key: 'hats.lpf.1.cutoff', cc: 74 })]));
    h.area.setMode('external-file');
    expect(h.area.persistence.set.get('hats.lpf.1.cutoff')?.midi).toEqual({ cc: 74 });
    h.area.persistence.set.upsert({ key: 'hats.lpf.1.cutoff', panel: true, midi: { cc: 40 } });
    midi.cc(74, 127);
    midi.cc(40, 0);
    expect(sent()).toEqual([[1, 20]]);
  });
});
