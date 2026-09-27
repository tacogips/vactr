// `eq-curve` (design 15.1.7, 13.5): one draggable node per band (x =
// frequency, y = gain, the wheel = q) over the summed response curve, with
// the live spectrum behind it. The spectrum is `VisualApi.mountSpectrum`
// on the chain's bus (a `bus` sibling site of the same top-level form),
// else the master bands.

import { bellDb, F_MAX, F_MIN, freqX, shelfDb } from './curves';
import { createComponent } from 'solid-js';
import { render as renderSolid } from 'solid-js/web';
import { EqSpectrumView } from './kind-chrome';
import { dot, findHandle, polyline, standardView, type Handle, type KindCtx, type KindView, type Node2D } from './handles';

export const EQ_DB = 24;

export interface Band {
  group: number;
  freq?: Handle;
  gain?: Handle;
  q?: Handle;
}

/** The bands of a declaration: params grouped by `ParamMeta.group`. */
export function bands(ctx: KindCtx): Band[] {
  const byGroup = new Map<number, Band>();
  for (const p of ctx.decl?.params ?? []) {
    const h = findHandle(ctx.handles, p.name);
    if (!h) continue;
    const b = byGroup.get(p.group) ?? { group: p.group };
    if (/freq|cutoff|center/.test(p.name)) b.freq ??= h;
    else if (/gain|band|tilt|amount/.test(p.name)) b.gain ??= h;
    else if (/(^|-)q$|width|res/.test(p.name)) b.q ??= h;
    byGroup.set(p.group, b);
  }
  return [...byGroup.values()].filter((b) => b.freq || b.gain).sort((a, b) => a.group - b.group);
}

/** The bus named by a `bus` sibling site of the chain, else undefined (master). */
export function chainBus(ctx: KindCtx): string | undefined {
  const site = ctx.chain.find((g) => g.name === 'bus')?.sites[0];
  return site ? String(site.value) : undefined;
}

function bandDb(b: Band, f: number, i: number, n: number): number {
  const f0 = b.freq?.value ?? F_MIN * 10 ** (1 + i);
  const g = b.gain?.value ?? 0;
  if (b.group === 1 || (n > 2 && i === 0 && !b.q)) return shelfDb(f, f0, g, false);
  if (b.group === 4 || (n > 2 && i === n - 1 && !b.q)) return shelfDb(f, f0, g, true);
  return bellDb(f, f0, g, b.q?.value ?? 0.7);
}

export function render(el: HTMLElement, ctx: KindCtx): KindView {
  const holder = ctx.doc.createElement('div');
  const disposeBack = renderSolid(() => createComponent(EqSpectrumView, {}), holder);
  const back = holder.firstElementChild as HTMLElement;
  el.appendChild(back);
  const bus = chainBus(ctx);
  const spectrum = ctx.deps.visual?.mountSpectrum(back, bus === undefined ? {} : { bus }) ?? null;
  const list = bands(ctx);
  const yOf = (db: number, h: number): number => h / 2 - (db / EQ_DB) * (h / 2);
  const view = standardView(
    el,
    'eq',
    ctx.handles,
    (s) => {
      const c = s.ctx;
      if (!c) return;
      const pts: [number, number][] = [];
      for (let x = 0; x <= s.w; x += 4) {
        const f = F_MIN * (F_MAX / F_MIN) ** (x / s.w);
        const db = list.reduce((acc, b, i) => acc + bandDb(b, f, i, list.length), 0);
        pts.push([x, yOf(db, s.h)]);
      }
      polyline(c, pts);
      for (const b of list) {
        const on = (b.freq?.enabled ?? false) || (b.gain?.enabled ?? false);
        dot(c, freqX(b.freq?.value ?? 1000) * s.w, yOf(b.gain?.value ?? 0, s.h), on);
      }
    },
    () =>
      list.map((b): Node2D => {
        const n: Node2D = { x: freqX(b.freq?.value ?? 1000) * view.surface.w, y: yOf(b.gain?.value ?? 0, view.surface.h) };
        if (b.freq) n.hx = b.freq;
        if (b.gain) n.hy = b.gain;
        if (b.q) n.wheel = b.q;
        return n;
      }),
  );
  return {
    update: () => view.update(),
    dispose() {
      view.dispose();
      spectrum?.dispose();
      disposeBack();
      back.remove();
    },
  };
}
