// The `eq-curve` editor: the live spectrum behind the bands comes from
// `VisualApi.mountSpectrum` on the chain's bus (a `bus` sibling site),
// else the master; band nodes drag frequency (x) and gain (y) and the
// wheel moves q, each through the slider write path.

import { afterEach, describe, expect, it, vi } from 'vitest';
import type { VisualApi } from '../../src/app/apis';
import { ParamsArea } from '../../src/params/mount';
import type { EditorDecl, SiteCall, WireSite } from '../../src/protocol/types';
import { mountSpectrum } from '../../src/visual/spectrum';
import { cleanup, FILE, setup, site, spanOf, type Harness } from '../bind/fixtures';
import { installCanvasFakes, type CanvasFakes } from '../support/canvas';

const PEQ: EditorDecl = {
  name: 'peq',
  kind: 'eq-curve',
  params: [
    { name: 'low-freq', ctl: 40, range: [20, 2000], curve: 'log', unit: 'hz', group: 1 },
    { name: 'low-gain', ctl: 41, range: [-24, 24], curve: 'linear', unit: 'db', group: 1 },
    { name: 'mid1-freq', ctl: 42, range: [20, 10000], curve: 'log', unit: 'hz', group: 2 },
    { name: 'mid1-gain', ctl: 43, range: [-24, 24], curve: 'linear', unit: 'db', group: 2 },
    { name: 'mid1-q', ctl: 44, range: [0.1, 10], curve: 'log', unit: 'none', group: 2 },
  ],
};

const WITH_BUS = 'd1 s [:bd] > peq 120.0 3.0 1000.0 -2.0 0.7 > bus 2';
const NO_BUS = 'd1 s [:bd] > peq 120.0 3.0 1000.0 -2.0 0.7';

function sites(text: string): WireSite[] {
  const call = (name: string, arg: number, param?: string): SiteCall => ({
    name,
    head: spanOf(text, name),
    ordinal: 1,
    arg,
    ...(param ? { param } : {}),
  });
  const out = [
    site(text, '120.0', 1, { call: call('peq', 0, 'low-freq') }),
    site(text, '3.0', 2, { call: call('peq', 1, 'low-gain') }),
    site(text, '1000.0', 3, { call: call('peq', 2, 'mid1-freq') }),
    site(text, '-2.0', 4, { call: call('peq', 3, 'mid1-gain') }),
    site(text, '0.7', 5, { call: call('peq', 4, 'mid1-q') }),
  ];
  if (text.includes('bus 2')) {
    const at = text.indexOf('bus 2') + 4;
    out.push({ ...site(text, '2', 6, { call: call('bus', 0) }), span: { start: at, end: at + 1 } });
  }
  return out;
}

let areas: ParamsArea[] = [];
let fakes: CanvasFakes | null = null;

afterEach(() => {
  for (const a of areas.splice(0)) a.dispose();
  cleanup();
  fakes?.restore();
  fakes = null;
});

function rig(text: string, visual?: (h: Harness) => VisualApi): { h: Harness; params: ParamsArea; spectrum: ReturnType<typeof vi.fn> } {
  fakes = installCanvasFakes();
  const h = setup(text, { editors: [PEQ] });
  const dispose = vi.fn();
  const spectrum = vi.fn((_el: HTMLElement, _src: { bus?: string }) => ({ dispose }));
  h.deps.visual = visual ? visual(h) : { mountSpectrum: spectrum };
  const params = new ParamsArea(h.root, h.deps, { file: FILE });
  areas.push(params);
  h.evalResult(sites(text));
  h.transport.clear();
  const g = params.groups().find((x) => x.name === 'peq');
  if (!g || !params.open(g.id)) throw new Error('peq did not open');
  return { h, params, spectrum };
}

const canvasOf = (h: Harness): HTMLCanvasElement => h.root.querySelector('.params-eq') as HTMLCanvasElement;
const pointer = (el: Element, type: string, x: number, y: number): void => {
  el.dispatchEvent(new MouseEvent(type, { clientX: x, clientY: y, bubbles: true }));
};

describe('eq-curve', () => {
  it('mounts the spectrum on the chain bus', () => {
    const { h, spectrum, params } = rig(WITH_BUS);
    expect(params.current?.kind).toBe('eq-curve');
    expect(spectrum).toHaveBeenCalledTimes(1);
    const [el, src] = spectrum.mock.calls[0] as [HTMLElement, { bus?: string }];
    expect(src).toEqual({ bus: '2' });
    expect(h.root.querySelector('.params-kind')?.contains(el)).toBe(true);
    expect(h.transport.sent).toEqual([]);
  });

  it('mounts the master spectrum when the chain has no bus', () => {
    const { spectrum } = rig(NO_BUS);
    expect(spectrum.mock.calls.map((c) => c[1])).toEqual([{}]);
  });

  it('draws the bus analyzer cells behind the bands with the real spectrum mount', () => {
    const r = rig(WITH_BUS, (h) => ({ mountSpectrum: (el, src) => mountSpectrum(el, h.store, src) }));
    r.h.emit({
      kind: 'levels',
      body: { levels: [{ source: ':master', rms: 0.1, bands: [0.1, 0.1] }], analyzers: [{ bus: '2', kind: 'spectrum', id: 1, cells: [0.5, 0.25, 0.125] }] },
    });
    const canvas = r.h.root.querySelector('.params-spectrum .visual-spectrum') as HTMLCanvasElement;
    expect(canvas.dataset.source).toBe('analyzer');
  });

  it('a band node drags frequency along x only, and the wheel moves q', () => {
    const { h } = rig(NO_BUS);
    const c = canvasOf(h);
    // low band: 120 Hz, +3 dB -> x = freqX(120) * 240, y = 60 - 3/24 * 60.
    const x = (Math.log(120 / 20) / Math.log(1000)) * 240;
    pointer(c, 'pointerdown', x, 52.5);
    pointer(c, 'pointermove', x + 24, 52.5);
    pointer(c, 'pointerup', x + 24, 52.5);
    const tweaks = h.transport.of('set-tweak').map((e) => e.body.id);
    expect(tweaks).toEqual([1]);
    const freq = h.transport.of('set-tweak')[0]?.body.value ?? 0;
    expect(freq).toBeGreaterThan(120);
    // mid band: 1000 Hz, -2 dB; the wheel writes mid1-q.
    const mx = (Math.log(1000 / 20) / Math.log(1000)) * 240;
    c.dispatchEvent(new WheelEvent('wheel', { clientX: mx, clientY: 65, deltaY: -100, bubbles: true }));
    expect(h.transport.of('set-tweak').map((e) => e.body.id)).toEqual([1, 5]);
    expect(h.text()).toBe(NO_BUS);
  });
});
