// `stereo-field` (design 15.1.7): a pan field, left to right, with the
// width drawn as the spread around the position.

import { dot, findHandle, standardView, type KindCtx, type KindView, type Node2D } from './handles';

export function render(el: HTMLElement, ctx: KindCtx): KindView {
  const pos = findHandle(ctx.handles, /pan|pos|balance|angle/);
  const width = findHandle(ctx.handles, /width|spread|amount/) ?? (pos ? undefined : ctx.handles[0]);
  const view = standardView(
    el,
    'stereo',
    ctx.handles,
    (s) => {
      const c = s.ctx;
      if (!c) return;
      const x = (pos ? pos.unitPos : 0.5) * s.w;
      const spread = (width ? width.unitPos : 0.5) * s.w * 0.5;
      c.fillStyle = 'rgba(143, 221, 255, 0.3)';
      c.fillRect(Math.max(0, x - spread), s.h / 2 - 10, Math.min(s.w, spread * 2), 20);
      c.fillStyle = '#333';
      c.fillRect(s.w / 2, 0, 1, s.h);
      dot(c, x, s.h / 2, (pos ?? width)?.enabled ?? false);
    },
    () => {
      const s = view.surface;
      const n: Node2D = { x: (pos ? pos.unitPos : 0.5) * s.w, y: s.h / 2 };
      if (pos) n.hx = pos;
      if (width) n.hy = width;
      return [n];
    },
  );
  return view;
}
