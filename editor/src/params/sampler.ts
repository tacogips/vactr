// `sampler-wave` (design 15.1.7 "Sampler waveform", 13.5 revised sampler
// requirements).
//
// - The waveform is `deps.code.samples.frames(bank, n)`: `bank` is the
//   chain's `s`/`bank` keyword (read from the form text, display only) and
//   `n` the chain's `n` site value. Without both: "no preview".
// - Start/end/loop handles are the `begin`/`end`/`loop` sites of the same
//   top-level form (or the call's own params of those names).
// - `slice n`, `chop n` and `striate n` counts draw as grid overlays.
// - MANUAL slice markers are the numeric sites of a `slice` point-list
//   argument; dragging one writes it through `BindApi.writeSite`.
// - Clicking slice k writes k into the SELECTED, EXISTING index literal (a
//   site of this form whose call is `n`, or the `slice` index argument).
//   With no such selection the click does nothing and shows a hint. The
//   editor never adds, removes or reorders steps: sequence structure stays
//   code-only, and this module has no API that could change it.
// - The `n` site's "browse" opens the sample browser at the bank.

import { createComponent, createSignal } from 'solid-js';
import { render as renderSolid } from 'solid-js/web';
import { SamplerChrome } from './kind-chrome';
import type { SampleFrames } from '../app/apis';
import type { WireSite } from '../protocol/types';
import { peaks } from './curves';
import {
  findHandle,
  NODE_RADIUS,
  siteHandle,
  standardView,
  type Handle,
  type KindCtx,
  type KindView,
  type Node2D,
} from './handles';
import type { CallGroup } from './open';

export const SELECT_HINT = 'select an index literal';
export const NO_PREVIEW = 'no preview';

export type OverlayKind = 'slice' | 'chop' | 'striate';

export interface SliceOverlay {
  kind: OverlayKind;
  count: number;
}

export interface SamplerView extends KindView {
  /** The begin/end/loop handles that are present. */
  regionHandles(): Handle[];
  /** The manual slice markers (one handle per point literal). */
  markers(): Handle[];
  overlays(): SliceOverlay[];
  /** The slice regions as `[from, to]` in 0..1. */
  slices(): [number, number][];
  /** Clicking slice `k`: writes into the selected index literal; false (and the hint) when none. */
  clickSlice(k: number): boolean;
}

const formGroups = (ctx: KindCtx): CallGroup[] => [ctx.group, ...ctx.chain];

/** The first site value of the chain call `name` (the call itself included). */
export function chainValue(ctx: KindCtx, name: string): number | undefined {
  return formGroups(ctx).find((g) => g.name === name)?.sites[0]?.value;
}

/** The chain's `s`/`bank` keyword, read (display only) from the form text. */
export function sampleBank(ctx: KindCtx): string | null {
  const text = ctx.formText();
  if (text === null) return null;
  const m = /(?:^|[\s(>[])(?:s|sound|bank)\s+:([A-Za-z0-9_.#-]+)/.exec(text);
  return m?.[1] ?? null;
}

/** The sample frames of the chain's bank at index `n` (browser tier only). */
export function findSample(ctx: KindCtx, n: number): SampleFrames | null {
  const bank = sampleBank(ctx);
  if (bank === null) return null;
  return ctx.deps.code?.samples.frames(bank, Math.max(0, Math.round(n))) ?? null;
}

/** True when `site` is an index literal of this form's index pattern. */
export function isIndexSite(ctx: KindCtx, site: WireSite | undefined): boolean {
  const c = site?.call;
  if (!site || !c) return false;
  const own = formGroups(ctx).some((g) => g.sites.some((s) => s.id === site.id));
  return own && (c.name === 'n' || (c.name === 'slice' && c.arg === 1));
}

function sliceArgSites(ctx: KindCtx): WireSite[] {
  return formGroups(ctx)
    .filter((g) => g.name === 'slice')
    .flatMap((g) => g.sites.filter((s) => (s.call?.arg ?? 0) === 0));
}

/** A point list (more than one literal, or a fraction) is manual markers; one integer is a count. */
function isPointList(sites: readonly WireSite[]): boolean {
  return sites.length > 1 || sites.some((s) => !Number.isInteger(s.value));
}

export function render(el: HTMLElement, ctx: KindCtx): SamplerView {
  const doc = ctx.doc;
  const editors = ctx.deps.store.manifest?.editors;
  const region = (name: string): Handle | undefined => {
    const own = findHandle(ctx.handles, name);
    if (own) return own;
    const s = formGroups(ctx)
      .find((g) => g.name === name)
      ?.sites.find((x) => (x.call?.arg ?? 0) === 0);
    return s ? siteHandle(s.id, name, ctx.host, editors, { range: [0, 1] }) : undefined;
  };
  const begin = region('begin');
  const end = region('end');
  const loop = region('loop');
  const regionList = [begin, end, loop].filter((h): h is Handle => h !== undefined);
  const pointSites = sliceArgSites(ctx);
  const markerList = isPointList(pointSites)
    ? pointSites.map((s, i) => siteHandle(s.id, `marker ${i + 1}`, ctx.host, editors, { range: [0, 1] }))
    : [];
  const rest = ctx.handles.filter((h) => !regionList.includes(h));

  const overlays = (): SliceOverlay[] => {
    const out: SliceOverlay[] = [];
    for (const kind of ['slice', 'chop', 'striate'] as const) {
      const sites = formGroups(ctx)
        .filter((g) => g.name === kind)
        .flatMap((g) => g.sites.filter((s) => (s.call?.arg ?? 0) === 0));
      if (kind === 'slice' && isPointList(sites)) continue;
      const count = Math.round(sites[0]?.value ?? 0);
      if (count > 0) out.push({ kind, count });
    }
    return out;
  };
  const slices = (): [number, number][] => {
    if (markerList.length > 0) {
      const pts = markerList.map((m) => m.value);
      return pts.map((p, i): [number, number] => [p, pts[i + 1] ?? 1]);
    }
    const n = overlays()[0]?.count ?? 0;
    return Array.from({ length: n }, (_, i): [number, number] => [i / n, (i + 1) / n]);
  };

  const nSite = formGroups(ctx).find((g) => g.name === 'n')?.sites[0];
  const [hint, setHint] = createSignal('');
  const [preview, setPreview] = createSignal('');
  const chromeHolder = doc.createElement('div');
  const disposeChrome = renderSolid(() => createComponent(SamplerChrome, {
    hint, preview, browse: nSite !== undefined,
    onBrowse: () => {
      const bank = sampleBank(ctx);
      ctx.deps.code?.samples.openBrowser(bank ?? undefined);
    },
  }), chromeHolder);
  el.append(...Array.from(chromeHolder.childNodes));

  const view = standardView(
    el,
    'sampler',
    [...regionList, ...markerList, ...rest],
    (s) => {
      const c = s.ctx;
      const frames = findSample(ctx, chainValue(ctx, 'n') ?? 0);
      setPreview(frames ? '' : NO_PREVIEW);
      if (!c) return;
      if (frames) {
        c.fillStyle = '#8fd';
        peaks(frames.data, frames.channels, s.w).forEach(([lo, hi], x) => {
          c.fillRect(x, s.h / 2 - hi * (s.h / 2), 1, Math.max(1, (hi - lo) * (s.h / 2)));
        });
      }
      for (const o of overlays()) {
        c.fillStyle = o.kind === 'slice' ? '#666' : o.kind === 'chop' ? '#665' : '#566';
        for (let i = 1; i < o.count; i += 1) c.fillRect((i / o.count) * s.w, 0, 1, s.h);
      }
      c.fillStyle = '#fc6';
      for (const m of markerList) c.fillRect(m.value * s.w - 1, 0, 2, s.h);
      c.fillStyle = 'rgba(0, 0, 0, 0.5)';
      if (begin) c.fillRect(0, 0, begin.value * s.w, s.h);
      if (end) c.fillRect(end.value * s.w, 0, s.w - end.value * s.w, s.h);
      if (loop && loop.value > 0) c.fillText('loop', 4, 12);
    },
    () => nodes(),
  );
  const nodes = (): Node2D[] => {
    const { w, h } = view.surface;
    const out: Node2D[] = [];
    if (begin) out.push({ x: begin.value * w, y: h - 6, hx: begin });
    if (end) out.push({ x: end.value * w, y: h - 6, hx: end });
    for (const m of markerList) out.push({ x: m.value * w, y: 6, hx: m });
    return out;
  };

  const clickSlice = (k: number): boolean => {
    const id = ctx.deps.code?.selectedSiteId() ?? null;
    const site = id === null ? undefined : ctx.host.site(id);
    if (id === null || !isIndexSite(ctx, site)) {
      setHint(SELECT_HINT);
      return false;
    }
    setHint('');
    ctx.host.bind()?.writeSite(id, k);
    return true;
  };
  const canvas = view.surface.canvas;
  const onClick = (ev: Event): void => {
    const e = ev as MouseEvent;
    const r = canvas.getBoundingClientRect();
    const x = (e.clientX - r.left) * (r.width > 0 ? view.surface.w / r.width : 1);
    const y = (e.clientY - r.top) * (r.height > 0 ? view.surface.h / r.height : 1);
    if (nodes().some((n) => Math.hypot(n.x - x, n.y - y) <= NODE_RADIUS)) return;
    const u = x / view.surface.w;
    const k = slices().findIndex(([a, b]) => u >= a && u < b);
    if (k >= 0) clickSlice(k);
  };
  canvas.addEventListener('click', onClick);

  return {
    regionHandles: () => regionList,
    markers: () => markerList,
    overlays,
    slices,
    clickSlice,
    update: () => view.update(),
    dispose() {
      canvas.removeEventListener('click', onClick);
      view.dispose();
      disposeChrome();
    },
  };
}
