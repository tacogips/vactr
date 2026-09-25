// `filter-response` (design 15.1.7): the response curve with one node
// (x = cutoff, y = resonance) for the filter named by the call.

import { F_MAX, F_MIN, filterDb, filterShape, freqX } from './curves';
import { dot, findHandle, polyline, standardView, type KindCtx, type KindView, type Node2D } from './handles';

export const FILTER_DB = 30;

export function render(el: HTMLElement, ctx: KindCtx): KindView {
  const cutoff = findHandle(ctx.handles, /cutoff|freq|center/) ?? ctx.handles[0];
  const res = findHandle(ctx.handles, /^res|^q$|reson|width/);
  const shape = filterShape(ctx.group.name);
  const yOf = (db: number, h: number): number => h * 0.3 - (db / FILTER_DB) * (h * 0.7);
  const resY = (h: number): number => h - (res ? res.unitPos : 0) * h;
  const view = standardView(
    el,
    'filter',
    ctx.handles,
    (s) => {
      const c = s.ctx;
      if (!c) return;
      const fc = cutoff?.value ?? 1000;
      const r = res ? res.unitPos : 0;
      const pts: [number, number][] = [];
      for (let x = 0; x <= s.w; x += 4) {
        const f = F_MIN * (F_MAX / F_MIN) ** (x / s.w);
        pts.push([x, Math.min(s.h, yOf(filterDb(f, fc, r, shape), s.h))]);
      }
      polyline(c, pts);
      dot(c, freqX(fc) * s.w, resY(s.h), cutoff?.enabled ?? false);
    },
    () => {
      const n: Node2D = { x: freqX(cutoff?.value ?? 1000) * view.surface.w, y: resY(view.surface.h) };
      if (cutoff) n.hx = cutoff;
      if (res) n.hy = res;
      return [n];
    },
  );
  return view;
}
