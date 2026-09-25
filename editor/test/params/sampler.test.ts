// Criterion 9 (sampler waveform editor): begin/end handles write their
// sites, `slice 8`/`chop 4` draw grid overlays, manual slice markers (the
// point literals of a `slice` list) drag through the standard write path
// in both modes, a slice click writes into the SELECTED EXISTING index
// literal only, a click without one is a no-op with a hint, the `n` site
// opens the sample browser, and the module has no sequence-editing API.

import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CodeApi, SampleFrames } from '../../src/app/apis';
import { ParamsArea } from '../../src/params/mount';
import * as samplerModule from '../../src/params/sampler';
import { NO_PREVIEW, SELECT_HINT, type SamplerView } from '../../src/params/sampler';
import type { SiteCall, WireSite } from '../../src/protocol/types';
import { cleanup, FILE, setup, site, spanOf, type Harness } from '../bind/fixtures';
import { installCanvasFakes, type CanvasFakes } from '../support/canvas';

const CHAIN = 'd1 s :bd > n 2 > begin 0.1 > end 0.9 > slice 8 [0 3] > chop 4';
const POINTS = 'd1 s :sn > slice [0.0 0.25 0.6] 1';

const call = (text: string, name: string, arg: number): SiteCall => ({ name, head: spanOf(text, name), ordinal: 1, arg });

function chainSites(): WireSite[] {
  return [
    site(CHAIN, '2', 1, { call: call(CHAIN, 'n', 0) }),
    site(CHAIN, '0.1', 2, { call: call(CHAIN, 'begin', 0) }),
    site(CHAIN, '0.9', 3, { call: call(CHAIN, 'end', 0) }),
    site(CHAIN, '8', 4, { call: call(CHAIN, 'slice', 0) }),
    site(CHAIN, '0', 5, { call: call(CHAIN, 'slice', 1) }, 2),
    site(CHAIN, '3', 6, { call: call(CHAIN, 'slice', 1) }),
    site(CHAIN, '4', 7, { call: call(CHAIN, 'chop', 0) }),
  ];
}

function pointSites(): WireSite[] {
  return [
    site(POINTS, '0.0', 1, { call: call(POINTS, 'slice', 0) }),
    site(POINTS, '0.25', 2, { call: call(POINTS, 'slice', 0) }),
    site(POINTS, '0.6', 3, { call: call(POINTS, 'slice', 0) }),
    site(POINTS, '1', 4, { call: call(POINTS, 'slice', 1) }, 1),
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
  view: SamplerView;
  select(id: number | null): void;
  frames: ReturnType<typeof vi.fn>;
  browser: ReturnType<typeof vi.fn>;
  tweaks(): [number, number][];
  canvas(): HTMLCanvasElement;
}

function rig(text: string, sites: WireSite[], preview: SampleFrames | null = null): Rig {
  fakes = installCanvasFakes();
  const h = setup(text);
  let selected: number | null = null;
  const frames = vi.fn((_bank: string, _n: number) => preview);
  const browser = vi.fn((_bank?: string) => {});
  const base = h.deps.code as CodeApi;
  h.deps.code = { ...base, selectedSiteId: () => selected, samples: { frames, openBrowser: browser } };
  const params = new ParamsArea(h.root, h.deps, { file: FILE });
  areas.push(params);
  h.evalResult(sites);
  h.transport.clear();
  const row = h.root.querySelector('.params-group[data-group$=":slice:1"]') as HTMLElement;
  (row.querySelector('.params-open-wave') as HTMLButtonElement).click();
  if (params.current?.kind !== 'sampler-wave') throw new Error('sampler did not open');
  return {
    h,
    params,
    view: params.current.view as SamplerView,
    select: (id) => {
      selected = id;
    },
    frames,
    browser,
    tweaks: () => h.transport.of('set-tweak').map((e) => [e.body.id, e.body.value]),
    canvas: () => h.root.querySelector('.params-sampler') as HTMLCanvasElement,
  };
}

const pointer = (el: Element, type: string, x: number, y: number): void => {
  el.dispatchEvent(new MouseEvent(type, { clientX: x, clientY: y, bubbles: true }));
};

const sourceEdit = (h: Harness, id: number): void => (h.row(id).querySelector('.bind-mode') as HTMLButtonElement).click();

describe('sampler waveform editor (criterion 9)', () => {
  it('begin/end handles write their sites (API and drag)', () => {
    const r = rig(CHAIN, chainSites());
    expect(r.view.regionHandles().map((x) => [x.param, x.siteId])).toEqual([
      ['begin', 2],
      ['end', 3],
    ]);
    r.view.regionHandles()[0]?.set(0.2);
    // The end handle sits at x = 0.9 * 240 on the bottom edge.
    pointer(r.canvas(), 'pointerdown', 216, 114);
    pointer(r.canvas(), 'pointermove', 192, 114);
    expect(r.tweaks()).toEqual([
      [2, 0.2],
      [3, 0.8],
    ]);
    expect(r.h.text()).toBe(CHAIN);
  });

  it('draws slice 8 and chop 4 as grid overlays', () => {
    const r = rig(CHAIN, chainSites());
    expect(r.view.overlays()).toEqual([
      { kind: 'slice', count: 8 },
      { kind: 'chop', count: 4 },
    ]);
    expect(r.view.slices()).toHaveLength(8);
    expect(r.view.markers()).toEqual([]);
    const ctx = fakes?.ctx(r.canvas());
    const lines = (color: string) => ctx?.named('fillRect').filter((c) => c.args[4] === color && c.args[2] === 1).length;
    expect(lines('#666')).toBe(7);
    expect(lines('#665')).toBe(3);
  });

  it('a slice click writes the slice index into the selected index literal only', () => {
    const r = rig(CHAIN, chainSites());
    r.select(6);
    expect(r.view.clickSlice(3)).toBe(true);
    // A canvas click on slice 5 of 8 (away from the handles).
    pointer(r.canvas(), 'click', (5.5 / 8) * 240, 60);
    expect(r.tweaks()).toEqual([
      [6, 3],
      [6, 5],
    ]);
    expect(r.h.transport.kinds().every((k) => k === 'set-tweak' || k === 'doc-changed')).toBe(true);
    // In source-edit mode the click rewrites THAT literal and nothing else.
    sourceEdit(r.h, 6);
    r.view.clickSlice(5);
    expect(r.h.text()).toBe(CHAIN.replace('[0 3]', '[0 5]'));
  });

  it('a click with no index literal selected is a no-op with a hint', () => {
    const r = rig(CHAIN, chainSites());
    expect(r.view.clickSlice(2)).toBe(false);
    expect(r.h.root.querySelector('.params-hint')?.textContent).toBe(SELECT_HINT);
    r.select(2); // `begin` is not an index literal
    pointer(r.canvas(), 'click', (2.5 / 8) * 240, 60);
    expect(r.h.transport.sent).toEqual([]);
    expect(r.h.text()).toBe(CHAIN);
  });

  it('manual slice markers drag their point literals (overlay and source-edit)', () => {
    const r = rig(POINTS, pointSites());
    expect(r.view.markers().map((m) => m.siteId)).toEqual([1, 2, 3]);
    expect(r.view.overlays()).toEqual([]);
    expect(r.view.slices()).toEqual([
      [0, 0.25],
      [0.25, 0.6],
      [0.6, 1],
    ]);
    pointer(r.canvas(), 'pointerdown', 60, 6);
    pointer(r.canvas(), 'pointermove', 72, 6);
    pointer(r.canvas(), 'pointerup', 72, 6);
    expect(r.tweaks()).toEqual([[2, 0.3]]);
    expect(r.h.text()).toBe(POINTS);

    const s = rig(POINTS, pointSites());
    sourceEdit(s.h, 2);
    pointer(s.canvas(), 'pointerdown', 60, 6);
    pointer(s.canvas(), 'pointermove', 72, 6);
    expect(s.h.text()).toBe('d1 s :sn > slice [0.0 0.3 0.6] 1');
    expect(s.h.transport.kinds()).toEqual(['doc-changed', 'eval']);
  });

  it('the n site opens the sample browser, and the waveform previews the bank frames', () => {
    const data = new Float32Array([0, 0.5, -0.5, 1, -1, 0.25]);
    const r = rig(CHAIN, chainSites(), { data, rate: 44100, channels: 1 });
    expect(r.frames).toHaveBeenCalledWith('bd', 2);
    expect(r.h.root.querySelector('.params-no-preview')?.textContent).toBe('');
    (r.h.root.querySelector('.params-browse') as HTMLButtonElement).click();
    expect(r.browser).toHaveBeenCalledWith('bd');
    expect(r.h.transport.sent).toEqual([]);
  });

  it('shows no preview without frames', () => {
    const r = rig(CHAIN, chainSites());
    expect(r.h.root.querySelector('.params-no-preview')?.textContent).toBe(NO_PREVIEW);
  });

  it('exposes no API that adds, removes or reorders steps', () => {
    expect(Object.keys(samplerModule).sort()).toEqual([
      'NO_PREVIEW',
      'SELECT_HINT',
      'chainValue',
      'findSample',
      'isIndexSite',
      'render',
      'sampleBank',
    ]);
    const r = rig(CHAIN, chainSites());
    const api = [...Object.keys(samplerModule), ...Object.keys(r.view)];
    expect(api.filter((k) => /add|remove|insert|reorder|delete|splice|append|push|move/i.test(k))).toEqual([]);
    expect(Object.keys(r.view).sort()).toEqual([
      'clickSlice',
      'dispose',
      'markers',
      'overlays',
      'regionHandles',
      'slices',
      'update',
    ]);
  });
});
