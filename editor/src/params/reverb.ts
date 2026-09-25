// `reverb-room` (design 15.1.7): a room whose size follows the size/decay
// handle and whose fill follows the mix/damping handle.

import { dot, findHandle, standardView, type KindCtx, type KindView, type Node2D } from './handles';

export function render(el: HTMLElement, ctx: KindCtx): KindView {
  const size = findHandle(ctx.handles, /size|decay|time|room/) ?? ctx.handles[0];
  const mix = findHandle(ctx.handles, /mix|wet|damp/);
  const view = standardView(
    el,
    'reverb',
    ctx.handles,
    (s) => {
      const c = s.ctx;
      if (!c) return;
      const u = size ? size.unitPos : 0.5;
      const w = 20 + u * (s.w - 40);
      const h = 10 + u * (s.h - 20);
      c.fillStyle = `rgba(143, 221, 255, ${0.15 + (mix ? mix.unitPos : 0.5) * 0.6})`;
      c.fillRect((s.w - w) / 2, (s.h - h) / 2, w, h);
      dot(c, (s.w + w) / 2, (s.h - h) / 2, size?.enabled ?? false);
    },
    () => {
      const s = view.surface;
      const u = size ? size.unitPos : 0.5;
      const w = 20 + u * (s.w - 40);
      const h = 10 + u * (s.h - 20);
      const n: Node2D = { x: (s.w + w) / 2, y: (s.h - h) / 2 };
      if (size) n.hx = size;
      if (mix) n.hy = mix;
      return [n];
    },
  );
  return view;
}
