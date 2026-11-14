// Every EditorKind of the ED-WIRE list renders for a minimal call group
// (with and without a 2D canvas context), repaints on `levels`/`tempo`,
// and a declared parameter with no site renders a DISABLED handle
// ("not in code") that sends nothing.

import { afterEach, describe, expect, it } from 'vitest';
import type { XyView } from '../../src/params/xy';
import { NOT_IN_CODE } from '../../src/params/handles';
import { ParamsArea } from '../../src/params/mount';
import { EDITOR_KINDS } from '../../src/params/open';
import type { EditorDecl, EditorKind, WireSite } from '../../src/protocol/types';
import { cleanup, FILE, setup, site, spanOf } from '../bind/fixtures';
import { installCanvasFakes, type CanvasFakes } from '../support/canvas';

/** The ED-WIRE editor kinds (command.md "editor-decl"). */
const KINDS: EditorKind[] = [
  'eq-curve',
  'filter-response',
  'dynamics-transfer',
  'envelope-shape',
  'delay-taps',
  'reverb-room',
  'sampler-wave',
  'wavetable-frames',
  'granular-region',
  'lfo-shape',
  'stereo-field',
  'xy-pad',
  'euclid-ring',
  'probability-dial',
  'length-handle',
  'scalar',
];

/** [bound param, missing param] per kind. */
const PARAMS: Record<EditorKind, [string, string]> = {
  'eq-curve': ['freq', 'gain'],
  'filter-response': ['cutoff', 'res'],
  'dynamics-transfer': ['threshold', 'ratio'],
  'envelope-shape': ['attack', 'release'],
  'delay-taps': ['time', 'feedback'],
  'reverb-room': ['size', 'mix'],
  'sampler-wave': ['begin', 'end'],
  'wavetable-frames': ['pos', 'morph'],
  'granular-region': ['pos', 'size'],
  'lfo-shape': ['rate', 'depth'],
  'stereo-field': ['width', 'pan'],
  'xy-pad': ['x', 'y'],
  'euclid-ring': ['hits', 'steps'],
  'probability-dial': ['probability', 'bias'],
  'length-handle': ['factor', 'bias'],
  scalar: ['amount', 'bias'],
};

const nameOf = (i: number): string => `kind${String.fromCharCode(97 + i)}`;

const EDITORS: EditorDecl[] = KINDS.map((kind, i) => ({
  name: nameOf(i),
  kind,
  params: PARAMS[kind].map((name) => ({ name, range: [0, 1] as [number, number], curve: 'linear' as const, unit: 'none' as const, group: 0 })),
}));

const TEXT = KINDS.map((_, i) => `d${i + 1} saw 110 > ${nameOf(i)} 0.5`).join('\n');

function sites(): WireSite[] {
  return KINDS.map((kind, i) =>
    site(TEXT, '0.5', i + 1, { call: { name: nameOf(i), head: spanOf(TEXT, nameOf(i)), ordinal: 1, arg: 0, param: PARAMS[kind][0] } }, i),
  );
}

let areas: ParamsArea[] = [];
let fakes: CanvasFakes | null = null;

afterEach(() => {
  for (const a of areas.splice(0)) a.dispose();
  cleanup();
  fakes?.restore();
  fakes = null;
});

function rig(no2d = false) {
  fakes = installCanvasFakes({ no2d });
  const h = setup(TEXT, { editors: EDITORS });
  const params = new ParamsArea(h.root, h.deps, { file: FILE });
  areas.push(params);
  h.evalResult(sites());
  h.transport.clear();
  return { h, params };
}

describe('every editor kind', () => {
  it('the editor knows exactly the ED-WIRE kinds', () => {
    expect([...EDITOR_KINDS]).toEqual(KINDS);
  });

  it.each(KINDS.map((k, i) => [k, i] as const))('%s renders, repaints and disables its missing handle', async (kind, i) => {
    const { h, params } = rig();
    const g = params.groups().find((x) => x.name === nameOf(i));
    const o = g ? params.open(g.id) : null;
    expect(o?.kind).toBe(kind);
    expect(h.root.querySelector(`.params-kind[data-kind="${kind}"]`)).not.toBeNull();
    if (!o) return;
    const [bound, missing] = PARAMS[kind];
    expect(o.handles.map((x) => [x.param, x.siteId])).toEqual([
      [bound, i + 1],
      [missing, null],
    ]);
    // Repaints on levels and tempo.
    h.emit({ kind: 'tempo', body: { bpm: 100, beats_per_cycle: 4, cycle: [0, 1] } });
    h.emit({ kind: 'levels', body: { levels: [{ source: ':master', rms: 0.2 }], analyzers: [{ bus: 'master', kind: 'level', id: 1, cells: [0.5] }] } });
    const off = o.handles.find((x) => x.param === missing);
    expect(off?.enabled).toBe(false);
    expect(off?.set(0.9)).toBeNull();
    expect(off?.drag(0.1)).toBeNull();
    await off?.learn();
    if (kind === 'xy-pad') {
      expect((o.view as XyView).axes()[1]).toBeNull();
    } else {
      const row = h.root.querySelector(`.params-handle[data-param="${missing}"]`) as HTMLElement;
      expect(row.classList.contains('params-disabled')).toBe(true);
      expect(row.title).toBe(NOT_IN_CODE);
      const input = row.querySelector('.params-handle-input') as HTMLInputElement;
      expect(input.disabled).toBe(true);
      input.value = '0.9';
      input.dispatchEvent(new Event('input'));
    }
    expect(h.transport.sent).toEqual([]);
    expect(h.text()).toBe(TEXT);
    // The bound handle still writes.
    const v = o.handles[0]?.set(0.25);
    // delay-taps snaps its time to 1/4 beat of the tempo (bpm 100: 0.15 s).
    expect(v).toBe(kind === 'delay-taps' ? 0.3 : 0.25);
    expect(h.transport.of('set-tweak').map((e) => [e.body.id, e.body.value])).toEqual([[i + 1, v]]);
    params.close();
    expect(h.root.querySelector('.params-kind')).toBeNull();
  });

  it('renders every kind without a 2D context', () => {
    const { params } = rig(true);
    for (const g of params.groups()) {
      const o = params.open(g.id);
      expect(o).not.toBeNull();
      o?.view.update();
    }
  });
});
