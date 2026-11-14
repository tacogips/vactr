// `granular-region` (design 15.1.7): the grain region (position and size
// handles) over the sample frames when the browser tier has them.

import { chainValue, findSample } from './sampler';
import { peaks } from './curves';
import { findHandle, standardView, type KindCtx, type KindView, type Node2D } from './handles';

export function render(el: HTMLElement, ctx: KindCtx): KindView {
  const pos = findHandle(ctx.handles, /pos|start|offset/);
  const size = findHandle(ctx.handles, /size|len|dur|spread/);
  const view = standardView(
    el,
    'granular',
    ctx.handles,
    (s) => {
      const c = s.ctx;
      if (!c) return;
      const frames = findSample(ctx, chainValue(ctx, 'n') ?? 0);
      if (frames) {
        c.fillStyle = '#355';
        peaks(frames.data, frames.channels, s.w).forEach(([lo, hi], x) => {
          c.fillRect(x, s.h / 2 - hi * (s.h / 2), 1, Math.max(1, (hi - lo) * (s.h / 2)));
        });
      }
      const x = (pos ? pos.unitPos : 0) * s.w;
      const w = Math.max(2, (size ? size.unitPos : 0.1) * s.w);
      c.fillStyle = 'rgba(255, 204, 102, 0.35)';
      c.fillRect(x, 0, w, s.h);
    },
    () => {
      const n: Node2D = { x: (pos ? pos.unitPos : 0) * view.surface.w, y: view.surface.h / 2 };
      if (pos) n.hx = pos;
      if (size) n.hy = size;
      return [n];
    },
  );
  return view;
}
