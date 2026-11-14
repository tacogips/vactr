// Criterion 4 (handles are sliders): for `peq`, `env-adsr` and `euclid`, a
// dragged parameter-editor handle records the IDENTICAL messages as the
// equivalent slider move through ED-BIND's panel: the same `set-tweak`
// (id, form_gen, value) in overlay mode, the same verified text edit plus
// form `eval` in source-edit mode, and the same `learn`. Two identical
// rigs (a real BindArea on a RecordingTransport each): one moved by the
// handle, one by the slider.

import { afterEach, describe, expect, it } from 'vitest';
import type { Handle } from '../../src/params/handles';
import { ParamsArea } from '../../src/params/mount';
import type { EditorDecl, SiteCall, WireSite } from '../../src/protocol/types';
import { cleanup, fakeMidi, FILE, settle, setup, site, spanOf, type Harness } from '../bind/fixtures';
import { installCanvasFakes, type CanvasFakes } from '../support/canvas';

const EDITORS: EditorDecl[] = [
  {
    name: 'peq',
    kind: 'eq-curve',
    params: [
      { name: 'low-freq', ctl: 40, range: [20, 2000], curve: 'log', unit: 'hz', group: 1 },
      { name: 'low-gain', ctl: 41, range: [-24, 24], curve: 'linear', unit: 'db', group: 1 },
    ],
  },
  {
    name: 'env-adsr',
    kind: 'envelope-shape',
    params: [
      { name: 'attack', ctl: 11, range: [0, 10], curve: 'linear', unit: 's', group: 0 },
      { name: 'decay', ctl: 12, range: [0, 10], curve: 'linear', unit: 's', group: 0 },
      { name: 'sustain', ctl: 13, range: [0, 1], curve: 'linear', unit: 'none', group: 0 },
      { name: 'release', ctl: 14, range: [0, 10], curve: 'linear', unit: 's', group: 0 },
    ],
  },
  {
    name: 'euclid',
    kind: 'euclid-ring',
    params: [
      { name: 'hits', range: [0, 32], curve: 'stepped', unit: 'none', group: 0 },
      { name: 'steps', range: [1, 32], curve: 'stepped', unit: 'none', group: 0 },
      { name: 'rotation', range: [0, 32], curve: 'stepped', unit: 'none', group: 0 },
    ],
  },
];

const TEXT = ['d1 s [:bd] > peq 120.0 3.0', 'd2 saw 110 > env-adsr 0.01 0.2 0.5 0.3', 'd3 s [:hh] > euclid 3 8 0'].join('\n');

const call = (name: string, arg: number, param: string): SiteCall => ({ name, head: spanOf(TEXT, name), ordinal: 1, arg, param });

function sites(): WireSite[] {
  const line3 = TEXT.indexOf('euclid');
  const occ = (lit: string): number => TEXT.slice(0, line3).split(lit).length - 1;
  return [
    site(TEXT, '120.0', 1, { call: call('peq', 0, 'low-freq') }),
    site(TEXT, '3.0', 2, { call: call('peq', 1, 'low-gain') }),
    site(TEXT, '0.01', 3, { call: call('env-adsr', 0, 'attack') }),
    site(TEXT, '0.2', 4, { call: call('env-adsr', 1, 'decay') }),
    site(TEXT, '0.5', 5, { call: call('env-adsr', 2, 'sustain') }),
    site(TEXT, '0.3', 6, { call: call('env-adsr', 3, 'release') }),
    site(TEXT, '3', 7, { call: call('euclid', 0, 'hits') }, occ('3')),
    site(TEXT, '8', 8, { call: call('euclid', 1, 'steps') }, occ('8')),
    site(TEXT, '0', 9, { call: call('euclid', 2, 'rotation') }, occ('0')),
  ];
}

let areas: ParamsArea[] = [];
let fakes: CanvasFakes | null = null;

afterEach(() => {
  for (const a of areas.splice(0)) a.dispose();
  cleanup();
  fakes?.restore();
  fakes = null;
});

interface Rig {
  h: Harness;
  params: ParamsArea;
}

function rig(): Rig {
  const h = setup(TEXT, { editors: EDITORS });
  const params = new ParamsArea(h.root, h.deps, { file: FILE });
  areas.push(params);
  h.evalResult(sites());
  h.transport.clear();
  return { h, params };
}

function handleOf(r: Rig, callName: string, param: string): Handle {
  const g = r.params.groups().find((x) => x.name === callName);
  const open = g ? r.params.open(g.id) : null;
  const h = open?.handles.find((x) => x.param === param);
  if (!h) throw new Error(`no handle ${callName}.${param}`);
  return h;
}

function slide(r: Rig, siteId: number, value: number): void {
  const slider = r.h.row(siteId).querySelector('.bind-slider') as HTMLInputElement;
  slider.value = String(value);
  slider.dispatchEvent(new Event('input'));
}

function sourceEdit(r: Rig, siteId: number): void {
  (r.h.row(siteId).querySelector('.bind-mode') as HTMLButtonElement).click();
}

/** Every client envelope without its `seq`. */
const messages = (r: Rig): unknown[] => r.h.transport.envelopes.map(({ seq: _seq, ...m }) => m);

const CASES: { call: string; param: string; site: number; delta: number }[] = [
  { call: 'peq', param: 'low-freq', site: 1, delta: 0.1 },
  { call: 'peq', param: 'low-gain', site: 2, delta: -0.05 },
  { call: 'env-adsr', param: 'sustain', site: 5, delta: 0.2 },
  { call: 'env-adsr', param: 'release', site: 6, delta: 0.03 },
  { call: 'euclid', param: 'hits', site: 7, delta: 0.1 },
];

describe('handles are sliders (criterion 4)', () => {
  it.each(CASES)('$call $param: a handle drag records the identical set-tweak as the slider (overlay)', (c) => {
    fakes = installCanvasFakes();
    const a = rig();
    const b = rig();
    const v = handleOf(a, c.call, c.param).drag(c.delta);
    if (v === null) throw new Error('handle disabled');
    slide(b, c.site, v);
    const tweaks = a.h.transport.of('set-tweak').map((e) => e.body);
    expect(tweaks).toEqual([{ file: FILE, id: c.site, form_gen: 1, value: v, edit_epoch: 0 }]);
    expect(b.h.transport.of('set-tweak').map((e) => e.body)).toEqual(tweaks);
    expect(messages(a)).toEqual(messages(b));
    expect(a.h.text()).toBe(TEXT);
    expect(b.h.text()).toBe(TEXT);
  });

  it.each(CASES)('$call $param: in source-edit mode the handle records the identical verified edit and form eval', (c) => {
    fakes = installCanvasFakes();
    const a = rig();
    const b = rig();
    sourceEdit(a, c.site);
    sourceEdit(b, c.site);
    const v = handleOf(a, c.call, c.param).drag(c.delta);
    if (v === null) throw new Error('handle disabled');
    slide(b, c.site, v);
    expect(a.h.text()).not.toBe(TEXT);
    expect(a.h.text()).toBe(b.h.text());
    expect(a.h.transport.kinds()).toEqual(['doc-changed', 'eval']);
    expect(messages(a)).toEqual(messages(b));
  });

  it.each([
    ['peq', 'low-freq', 1],
    ['env-adsr', 'attack', 3],
    ['euclid', 'steps', 8],
  ] as const)('%s %s: learn on the handle sends the same learn as the slider', async (callName, param, siteId) => {
    fakes = installCanvasFakes();
    const a = rig();
    const b = rig();
    const ma = fakeMidi();
    const mb = fakeMidi();
    a.h.deps.midi = ma;
    b.h.deps.midi = mb;
    // The learn resolves on the session's reply; only its request is compared here.
    void handleOf(a, callName, param).learn();
    (b.h.row(siteId).querySelector('.bind-learn') as HTMLButtonElement).click();
    expect(ma.learning).toBe(true);
    ma.cc(21, 64, 1);
    mb.cc(21, 64, 1);
    await settle();
    const learn = a.h.transport.of('learn').map((e) => e.body);
    expect(learn).toEqual([{ file: FILE, binding: siteId, cc: 21, ch: 1, edit_epoch: 0 }]);
    expect(b.h.transport.of('learn').map((e) => e.body)).toEqual(learn);
  });

  it('the handle row input writes through the same path as a drag', () => {
    fakes = installCanvasFakes();
    const a = rig();
    handleOf(a, 'env-adsr', 'sustain');
    const input = a.h.root.querySelector('.params-handle[data-param="sustain"] .params-handle-input') as HTMLInputElement;
    input.value = '0.75';
    input.dispatchEvent(new Event('input'));
    expect(a.h.transport.of('set-tweak').map((e) => [e.body.id, e.body.value])).toEqual([[5, 0.75]]);
  });
  it('an overlay value is shown until the literal itself changes', () => {
    fakes = installCanvasFakes();
    const a = rig();
    const h = handleOf(a, 'env-adsr', 'sustain');
    h.set(0.75);
    expect(h.value).toBe(0.75);
    // A re-evaluation with the same literal keeps the overlay value.
    a.h.evalResult(sites());
    expect(a.params.current?.handles.find((x) => x.param === 'sustain')?.value).toBe(0.75);
    // The literal re-evaluated to a new value wins.
    a.h.evalResult(sites().map((s) => (s.id === 5 ? { ...s, value: 0.4 } : s)));
    expect(a.params.current?.handles.find((x) => x.param === 'sustain')?.value).toBe(0.4);
  });
});
