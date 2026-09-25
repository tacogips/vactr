// `dynamics-transfer` (design 15.1.7, 13.5): the static transfer curve
// with threshold/ratio/knee handles, crossover handles when the
// declaration is `multiband`, and the input level from a published bus
// `level` analyzer. There is no gain-reduction meter (no GR cell, 15.1.1).

import { sameBus, toDb } from '../visual/spectrum';
import { freqX, transferDb } from './curves';
import { chainBus } from './eq';
import { dot, findHandle, polyline, standardView, type KindCtx, type KindView, type Node2D } from './handles';

export const DYN_FLOOR_DB = -60;

/** The input level (dBFS) of the chain's bus `level` analyzer, when published. */
export function inputLevelDb(ctx: KindCtx): number | null {
  const bus = chainBus(ctx) ?? 'master';
  const a = ctx.deps.store.levels?.analyzers?.find((x) => x.kind === 'level' && sameBus(x.bus, bus));
  const v = a?.cells[0];
  return typeof v === 'number' ? toDb(v) : null;
}

export function render(el: HTMLElement, ctx: KindCtx): KindView {
  const thr = findHandle(ctx.handles, /thresh/);
  const ratio = findHandle(ctx.handles, /ratio/);
  const knee = findHandle(ctx.handles, /knee/);
  const cross = ctx.decl?.multiband ? ctx.handles.filter((h) => /cross|split|freq/.test(h.param)) : [];
  const px = (db: number, w: number): number => ((db - DYN_FLOOR_DB) / -DYN_FLOOR_DB) * w;
  const py = (db: number, h: number): number => h - ((db - DYN_FLOOR_DB) / -DYN_FLOOR_DB) * h;
  const out = (db: number): number => transferDb(db, thr?.value ?? -20, ratio?.value ?? 4, knee?.value ?? 0);
  const view = standardView(
    el,
    'dynamics',
    ctx.handles,
    (s) => {
      const c = s.ctx;
      if (!c) return;
      const pts: [number, number][] = [];
      for (let db = DYN_FLOOR_DB; db <= 0; db += 1) pts.push([px(db, s.w), py(out(db), s.h)]);
      polyline(c, pts);
      const t = thr?.value ?? -20;
      dot(c, px(t, s.w), py(out(t), s.h), thr?.enabled ?? false, 'thr');
      dot(c, px(0, s.w) - 4, py(out(0), s.h), ratio?.enabled ?? false, 'ratio');
      for (const h of cross) {
        c.fillStyle = h.enabled ? '#fc6' : '#555';
        c.fillRect(freqX(h.value) * s.w, s.h - 6, 2, 6);
      }
      const level = inputLevelDb(ctx);
      if (level !== null) {
        c.fillStyle = '#4fb3ff';
        c.fillRect(Math.max(0, px(level, s.w)), 0, 2, s.h);
      }
    },
    () => {
      const w = view.surface.w;
      const h = view.surface.h;
      const t = thr?.value ?? -20;
      const nodes: Node2D[] = [];
      if (thr) nodes.push({ x: px(t, w), y: py(out(t), h), hx: thr, ...(knee ? { wheel: knee } : {}) });
      if (ratio) nodes.push({ x: px(0, w) - 4, y: py(out(0), h), hy: ratio });
      for (const c of cross) nodes.push({ x: freqX(c.value) * w, y: h - 3, hx: c });
      return nodes;
    },
  );
  return view;
}
