// `wavetable-frames` (design 15.1.7): the table's frames stacked, the
// frame at the position handle highlighted.

import { findHandle, polyline, standardView, type KindCtx, type KindView, type Node2D } from './handles';

export const FRAMES = 8;

export function render(el: HTMLElement, ctx: KindCtx): KindView {
  const pos = findHandle(ctx.handles, /pos|frame|index|morph/);
  const view = standardView(
    el,
    'wavetable',
    ctx.handles,
    (s) => {
      const c = s.ctx;
      if (!c) return;
      const at = pos ? pos.unitPos * (FRAMES - 1) : 0;
      for (let f = 0; f < FRAMES; f += 1) {
        const y0 = 10 + (f / FRAMES) * (s.h - 20);
        const pts: [number, number][] = [];
        for (let x = 0; x <= s.w; x += 4) {
          const p = x / s.w;
          const v = Math.sin(2 * Math.PI * p) * (1 - f / FRAMES) + (2 * p - 1) * (f / FRAMES);
          pts.push([x, y0 - v * 6]);
        }
        polyline(c, pts, Math.round(at) === f ? '#fc6' : '#355');
      }
    },
    () => (pos ? [{ x: 8, y: 10 + pos.unitPos * (view.surface.h - 20), hy: pos } as Node2D] : []),
  );
  return view;
}
