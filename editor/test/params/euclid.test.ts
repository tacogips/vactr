// The `euclid-ring` editor: hits, steps and rotation handles move the
// right sites; the rotation handle wraps modulo the step count; hits
// never exceed the steps.

import { afterEach, describe, expect, it } from 'vitest';
import { euclidPattern } from '../../src/params/curves';
import type { EuclidView } from '../../src/params/euclid';
import { ParamsArea } from '../../src/params/mount';
import type { EditorDecl, SiteCall, WireSite } from '../../src/protocol/types';
import { cleanup, FILE, setup, site, spanOf, type Harness } from '../bind/fixtures';
import { installCanvasFakes, type CanvasFakes } from '../support/canvas';

const EUCLID: EditorDecl = {
  name: 'euclid',
  kind: 'euclid-ring',
  params: [
    { name: 'hits', range: [0, 32], curve: 'stepped', unit: 'none', group: 0 },
    { name: 'steps', range: [1, 32], curve: 'stepped', unit: 'none', group: 0 },
    { name: 'rotation', range: [0, 32], curve: 'stepped', unit: 'none', group: 0 },
  ],
};

const TEXT = 'd1 s [:hh] > euclid 3 8 7';

function sites(): WireSite[] {
  const call = (arg: number, param: string): SiteCall => ({ name: 'euclid', head: spanOf(TEXT, 'euclid'), ordinal: 1, arg, param });
  return [site(TEXT, '3', 1, { call: call(0, 'hits') }), site(TEXT, '8', 2, { call: call(1, 'steps') }), site(TEXT, '7', 3, { call: call(2, 'rotation') })];
}

let areas: ParamsArea[] = [];
let fakes: CanvasFakes | null = null;

afterEach(() => {
  for (const a of areas.splice(0)) a.dispose();
  cleanup();
  fakes?.restore();
  fakes = null;
});

function rig() {
  fakes = installCanvasFakes();
  const h: Harness = setup(TEXT, { editors: [EUCLID] });
  const params = new ParamsArea(h.root, h.deps, { file: FILE });
  areas.push(params);
  h.evalResult(sites());
  h.transport.clear();
  const o = params.open(params.groups()[0]?.id ?? '');
  if (!o) throw new Error('euclid did not open');
  const handle = (name: string) => {
    const x = o.handles.find((k) => k.param === name);
    if (!x) throw new Error(name);
    return x;
  };
  const tweaks = () => h.transport.of('set-tweak').map((e) => [e.body.id, e.body.value]);
  return { h, o, view: o.view as EuclidView, handle, tweaks };
}

describe('euclid-ring', () => {
  it('binds hits, steps and rotation to their sites and draws the pattern', () => {
    const { o, view } = rig();
    expect(o.kind).toBe('euclid-ring');
    expect(o.handles.map((h) => [h.param, h.siteId])).toEqual([
      ['hits', 1],
      ['steps', 2],
      ['rotation', 3],
    ]);
    expect(view.pattern()).toEqual(euclidPattern(3, 8, 7));
    expect(view.pattern().filter(Boolean)).toHaveLength(3);
  });

  it('the rotation handle wraps modulo steps', () => {
    const { view, handle, tweaks } = rig();
    expect(view.rotate(1)).toBe(0); // 7 + 1 wraps to 0 of 8
    expect(view.rotate(-1)).toBe(7);
    expect(handle('rotation').set(11)).toBe(3);
    expect(tweaks()).toEqual([
      [3, 0],
      [3, 7],
      [3, 3],
    ]);
  });

  it('wraps against the current step count and clamps hits to it', () => {
    const { handle, tweaks } = rig();
    handle('steps').set(5);
    expect(handle('rotation').set(6)).toBe(1);
    expect(handle('hits').set(12)).toBe(5);
    expect(tweaks()).toEqual([
      [2, 5],
      [3, 1],
      [1, 5],
    ]);
  });
});
