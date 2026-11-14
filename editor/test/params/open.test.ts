// Criterion 4 (opening): call groups from `site.call`, the editor kind
// from `manifest.editors` (the ED-WIRE editor-table shape: `peq`
// eq-curve, `env-adsr` envelope-shape, `euclid` euclid-ring without
// `ctl`), `scalar` for anything else, and handle binding by `call.param`
// then by `call.arg`. A real BindArea (ED-BIND) on a RecordingTransport.

import { afterEach, describe, expect, it } from 'vitest';
import { ParamsArea } from '../../src/params/mount';
import { bindHandles, callGroups, editorFor } from '../../src/params/open';
import type { EditorDecl, EditorKind, SiteCall, WireSite } from '../../src/protocol/types';
import { cleanup, FILE, formsByLine, setup, site, spanOf } from '../bind/fixtures';
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
  { name: 'odd', kind: 'no-such-kind' as EditorKind, params: [] },
];

const TEXT = 'd1 s [:bd] > peq 120.0 3.0 > env-adsr 0.01 0.2 0.5 0.3 > euclid 3 8 0 > odd 1.5 > wobble 2.5';

/** The index of the first `literal` occurrence at or after `anchor`. */
const after = (literal: string, anchor: string): number => {
  const stop = TEXT.indexOf(anchor);
  let n = 0;
  for (let at = TEXT.indexOf(literal); at >= 0 && at < stop; at = TEXT.indexOf(literal, at + 1)) n += 1;
  return n;
};

const call = (name: string, arg: number, param?: string): SiteCall => ({
  name,
  head: spanOf(TEXT, name),
  ordinal: 1,
  arg,
  ...(param === undefined ? {} : { param }),
});

function sites(): WireSite[] {
  return [
    site(TEXT, '120.0', 1, { call: call('peq', 0, 'low-freq') }),
    site(TEXT, '3.0', 2, { call: call('peq', 1, 'low-gain') }),
    // env-adsr: bound by `call.param` (named) and by `call.arg` (no param).
    site(TEXT, '0.01', 3, { call: call('env-adsr', 0) }),
    site(TEXT, '0.2', 4, { call: call('env-adsr', 1, 'decay') }),
    site(TEXT, '0.5', 5, { call: call('env-adsr', 2) }),
    site(TEXT, '0.3', 6, { call: call('env-adsr', 3, 'release') }),
    site(TEXT, '3', 7, { call: call('euclid', 0, 'hits') }, after('3', 'euclid')),
    site(TEXT, '8', 8, { call: call('euclid', 1, 'steps') }),
    site(TEXT, '0', 9, { call: call('euclid', 2, 'rotation') }, after('0', 'euclid')),
    site(TEXT, '1.5', 10, { call: call('odd', 0) }),
    site(TEXT, '2.5', 11, { call: call('wobble', 0) }),
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

function rig() {
  fakes = installCanvasFakes();
  const h = setup(TEXT, { editors: EDITORS });
  const params = new ParamsArea(h.root, h.deps, { file: FILE });
  areas.push(params);
  h.evalResult(sites());
  h.transport.clear();
  const idOf = (name: string): string => {
    const g = params.groups().find((x) => x.name === name);
    if (!g) throw new Error(`no group ${name}`);
    return g.id;
  };
  return { h, params, idOf };
}

describe('call groups and editor kinds (criterion 4)', () => {
  it('groups sites by (form, call name, head, ordinal)', () => {
    const groups = callGroups(sites(), formsByLine(TEXT));
    expect(groups.map((g) => [g.name, g.sites.map((s) => s.id)])).toEqual([
      ['peq', [1, 2]],
      ['env-adsr', [3, 4, 5, 6]],
      ['euclid', [7, 8, 9]],
      ['odd', [10]],
      ['wobble', [11]],
    ]);
    expect(groups.every((g) => g.form === 0)).toBe(true);
  });

  it('peq opens eq-curve, env-adsr envelope-shape, euclid euclid-ring; unknown names and kinds open scalar', () => {
    const { params, idOf, h } = rig();
    const kinds: Record<string, EditorKind> = {};
    for (const name of ['peq', 'env-adsr', 'euclid', 'odd', 'wobble']) {
      const open = params.open(idOf(name));
      kinds[name] = open?.kind as EditorKind;
      expect(h.root.querySelector('.params-kind')?.getAttribute('data-kind')).toBe(open?.kind);
    }
    expect(kinds).toEqual({
      peq: 'eq-curve',
      'env-adsr': 'envelope-shape',
      euclid: 'euclid-ring',
      odd: 'scalar',
      wobble: 'scalar',
    });
    expect(editorFor({ name: 'wobble' }, EDITORS)).toEqual({ kind: 'scalar' });
    // Opening sends nothing.
    expect(h.transport.sent).toEqual([]);
  });

  it('binds handles by call.param first, else by call.arg', () => {
    const env = callGroups(sites(), formsByLine(TEXT)).find((g) => g.name === 'env-adsr');
    if (!env) throw new Error('no env-adsr group');
    const decl = EDITORS[1] as EditorDecl;
    expect([...bindHandles(env, decl)]).toEqual([
      ['attack', 3],
      ['decay', 4],
      ['sustain', 5],
      ['release', 6],
    ]);
    // A named argument out of positional order still binds by its param.
    const swapped = { sites: [site(TEXT, '0.2', 4, { call: call('env-adsr', 0, 'release') })] };
    expect(bindHandles(swapped, decl).get('release')).toBe(4);
    expect(bindHandles(swapped, decl).get('attack')).toBeNull();
  });

  it('lists every call group with an open button, and a click on a call head in the code opens it', () => {
    const { h, params } = rig();
    const rows = [...h.root.querySelectorAll('.params-group')].map((r) => r.querySelector('.params-open')?.textContent);
    expect(rows).toEqual(['eq-curve', 'envelope-shape', 'euclid-ring', 'scalar', 'scalar']);
    const heads = [...h.view.dom.querySelectorAll('.params-call-head')].map((e) => e.textContent);
    expect(heads).toEqual(['peq', 'env-adsr', 'euclid', 'odd', 'wobble']);
    const env = h.view.dom.querySelector('.params-call-head[data-group$=":env-adsr:1"]') as HTMLElement;
    env.click();
    expect(params.current?.kind).toBe('envelope-shape');
    expect(params.current?.handles.map((x) => [x.param, x.siteId])).toEqual([
      ['attack', 3],
      ['decay', 4],
      ['sustain', 5],
      ['release', 6],
    ]);
    (h.root.querySelector('.params-group:nth-child(3) .params-open') as HTMLButtonElement).click();
    expect(params.current?.kind).toBe('euclid-ring');
    expect(h.transport.sent).toEqual([]);
    expect(h.text()).toBe(TEXT);
  });
});
