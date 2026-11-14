// The `envelope-shape` editor: its stage handles move the right sites
// (ADSR, perc and line forms), and dragging a stage node writes only the
// axes that moved.

import { afterEach, describe, expect, it } from 'vitest';
import { stages } from '../../src/params/envelope';
import { ParamsArea } from '../../src/params/mount';
import type { EditorDecl, SiteCall, WireSite } from '../../src/protocol/types';
import { cleanup, FILE, setup, site, spanOf, type Harness } from '../bind/fixtures';
import { installCanvasFakes, type CanvasFakes } from '../support/canvas';

const p = (name: string, range: [number, number], unit: 's' | 'none' = 's') => ({
  name,
  range,
  curve: 'linear' as const,
  unit,
  group: 0,
});

const EDITORS: EditorDecl[] = [
  {
    name: 'env-adsr',
    kind: 'envelope-shape',
    params: [p('attack', [0, 10]), p('decay', [0, 10]), p('sustain', [0, 1], 'none'), p('release', [0, 10])],
  },
  { name: 'env-perc', kind: 'envelope-shape', params: [p('attack', [0, 10]), p('release', [0, 10])] },
  { name: 'line', kind: 'envelope-shape', params: [p('from', [0, 2], 'none'), p('to', [0, 2], 'none'), p('dur', [0, 10])] },
];

const TEXT = ['d1 saw 110 > env-adsr 0.01 0.2 0.5 0.3', 'd2 saw 220 > env-perc 0.02 0.4', 'd3 saw 330 > line 1.0 0.25 2.0'].join('\n');

function sites(): WireSite[] {
  const call = (name: string, arg: number, param: string): SiteCall => ({ name, head: spanOf(TEXT, name), ordinal: 1, arg, param });
  return [
    site(TEXT, '0.01', 1, { call: call('env-adsr', 0, 'attack') }),
    site(TEXT, '0.2', 2, { call: call('env-adsr', 1, 'decay') }),
    site(TEXT, '0.5', 3, { call: call('env-adsr', 2, 'sustain') }),
    site(TEXT, '0.3', 4, { call: call('env-adsr', 3, 'release') }),
    site(TEXT, '0.02', 5, { call: call('env-perc', 0, 'attack') }),
    site(TEXT, '0.4', 6, { call: call('env-perc', 1, 'release') }),
    site(TEXT, '1.0', 7, { call: call('line', 0, 'from') }),
    site(TEXT, '0.25', 8, { call: call('line', 1, 'to') }),
    site(TEXT, '2.0', 9, { call: call('line', 2, 'dur') }),
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

function rig(): { h: Harness; params: ParamsArea } {
  fakes = installCanvasFakes();
  const h = setup(TEXT, { editors: EDITORS });
  const params = new ParamsArea(h.root, h.deps, { file: FILE });
  areas.push(params);
  h.evalResult(sites());
  h.transport.clear();
  return { h, params };
}

function open(r: { params: ParamsArea }, name: string) {
  const g = r.params.groups().find((x) => x.name === name);
  const o = g ? r.params.open(g.id) : null;
  if (!o) throw new Error(`no ${name}`);
  return o;
}

describe('envelope-shape', () => {
  it('ADSR stage handles move the attack, decay, sustain and release sites', () => {
    const r = rig();
    const o = open(r, 'env-adsr');
    expect(o.kind).toBe('envelope-shape');
    const values: Record<string, number> = { attack: 0.05, decay: 0.4, sustain: 0.8, release: 1.5 };
    for (const h of o.handles) h.set(values[h.param] as number);
    expect(r.h.transport.of('set-tweak').map((e) => [e.body.id, e.body.value])).toEqual([
      [1, 0.05],
      [2, 0.4],
      [3, 0.8],
      [4, 1.5],
    ]);
    expect(r.h.text()).toBe(TEXT);
  });

  it('dragging the decay/sustain node up writes the sustain site only', () => {
    const r = rig();
    open(r, 'env-adsr');
    // The node sits at X(0.21) of a 0.615 s span on a 240x120 canvas, Y(0.5).
    const x = 4 + (0.21 / 0.615) * 232;
    const y = 116 - 0.5 * 112;
    const c = r.h.root.querySelector('.params-envelope') as HTMLCanvasElement;
    c.dispatchEvent(new MouseEvent('pointerdown', { clientX: x, clientY: y }));
    c.dispatchEvent(new MouseEvent('pointermove', { clientX: x, clientY: y - 12 }));
    expect(r.h.transport.of('set-tweak').map((e) => [e.body.id, e.body.value])).toEqual([[3, 0.6]]);
  });

  it('perc and line forms bind their own stages', () => {
    const r = rig();
    const perc = open(r, 'env-perc');
    expect(perc.handles.map((h) => [h.param, h.siteId])).toEqual([
      ['attack', 5],
      ['release', 6],
    ]);
    expect(stages(perc.ctx).map((s) => [s.x, s.y])).toEqual([
      [0, 0],
      [0.02, 1],
      [0.42000000000000004, 0],
    ]);
    const line = open(r, 'line');
    expect(stages(line.ctx).map((s) => [s.x, s.y])).toEqual([
      [0, 1],
      [2, 0.25],
    ]);
    line.handles.find((h) => h.param === 'to')?.set(0.5);
    expect(r.h.transport.of('set-tweak').map((e) => [e.body.id, e.body.value])).toEqual([[8, 0.5]]);
  });
});
