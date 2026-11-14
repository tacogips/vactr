// `length-handle` (design 15.1.7): a bar whose length is the `hold`/
// `fast`/`slow` factor, dragged at its end.

import { dot, standardView, type KindCtx, type KindView } from './handles';

export function render(el: HTMLElement, ctx: KindCtx): KindView {
  const f = ctx.handles[0];
  const view = standardView(
    el,
    'length',
    ctx.handles,
    (s) => {
      const c = s.ctx;
      if (!c) return;
      const x = 8 + (f ? f.unitPos : 0.5) * (s.w - 16);
      c.fillStyle = '#8fd';
      c.fillRect(8, s.h / 2 - 6, x - 8, 12);
      dot(c, x, s.h / 2, f?.enabled ?? false, f ? `x${f.value}` : undefined);
    },
    () => (f ? [{ x: 8 + f.unitPos * (view.surface.w - 16), y: view.surface.h / 2, hx: f }] : []),
  );
  return view;
}
