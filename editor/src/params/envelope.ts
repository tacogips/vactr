// `envelope-shape` (design 15.1.7): the stage shape with one handle per
// stage. ADSR: attack/decay/release drag along x, sustain along y. Perc:
// attack/release. Line: from/to along y, dur along x.

import { dot, findHandle, polyline, standardView, type Handle, type KindCtx, type KindView, type Node2D } from './handles';

export type EnvForm = 'adsr' | 'perc' | 'line';

export function envForm(name: string): EnvForm {
  if (name === 'line') return 'line';
  if (/perc/.test(name)) return 'perc';
  return 'adsr';
}

interface Pt {
  x: number;
  y: number;
  hx?: Handle | undefined;
  hy?: Handle | undefined;
}

/** The stage breakpoints in (seconds, level) and the handle each moves. */
export function stages(ctx: KindCtx): Pt[] {
  const h = (n: string): Handle | undefined => findHandle(ctx.handles, n);
  const v = (x: Handle | undefined, d: number): number => x?.value ?? d;
  switch (envForm(ctx.group.name)) {
    case 'line': {
      const from = h('from');
      const to = h('to');
      const dur = h('dur');
      return [
        { x: 0, y: v(from, 1), hy: from },
        { x: v(dur, 1), y: v(to, 0), hx: dur, hy: to },
      ];
    }
    case 'perc': {
      const a = h('attack');
      const r = h('release');
      return [
        { x: 0, y: 0 },
        { x: v(a, 0.01), y: 1, hx: a },
        { x: v(a, 0.01) + v(r, 0.3), y: 0, hx: r },
      ];
    }
    case 'adsr': {
      const a = h('attack');
      const d = h('decay');
      const s = h('sustain');
      const r = h('release');
      const ta = v(a, 0.01);
      const td = ta + v(d, 0.1);
      const hold = Math.max(0.1, td * 0.5);
      return [
        { x: 0, y: 0 },
        { x: ta, y: 1, hx: a },
        { x: td, y: v(s, 1), hx: d, hy: s },
        { x: td + hold, y: v(s, 1) },
        { x: td + hold + v(r, 0.1), y: 0, hx: r },
      ];
    }
  }
}

export function render(el: HTMLElement, ctx: KindCtx): KindView {
  const layout = (w: number, h: number): { pts: Pt[]; X: (x: number) => number; Y: (y: number) => number } => {
    const pts = stages(ctx);
    const span = Math.max(1e-3, ...pts.map((p) => p.x));
    return { pts, X: (x) => 4 + (x / span) * (w - 8), Y: (y) => h - 4 - Math.max(0, Math.min(1, y)) * (h - 8) };
  };
  const view = standardView(
    el,
    'envelope',
    ctx.handles,
    (s) => {
      const c = s.ctx;
      if (!c) return;
      const { pts, X, Y } = layout(s.w, s.h);
      polyline(c, pts.map((p) => [X(p.x), Y(p.y)]));
      for (const p of pts) if (p.hx || p.hy) dot(c, X(p.x), Y(p.y), (p.hx ?? p.hy)?.enabled ?? false);
    },
    () => {
      const { pts, X, Y } = layout(view.surface.w, view.surface.h);
      return pts
        .filter((p) => p.hx || p.hy)
        .map((p): Node2D => ({ x: X(p.x), y: Y(p.y), ...(p.hx ? { hx: p.hx } : {}), ...(p.hy ? { hy: p.hy } : {}) }));
    },
  );
  return view;
}
