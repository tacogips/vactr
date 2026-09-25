// `probability-dial` (design 15.1.7): a dial for `maybe`/`degrade-by`.

import { findHandle, standardView, type KindCtx, type KindView } from './handles';

export function render(el: HTMLElement, ctx: KindCtx): KindView {
  const p = findHandle(ctx.handles, /prob|amount|chance/) ?? ctx.handles[0];
  const view = standardView(
    el,
    'probability',
    ctx.handles,
    (s) => {
      const c = s.ctx;
      if (!c) return;
      const u = p ? p.unitPos : 0;
      const cx = s.w / 2;
      const cy = s.h / 2;
      const r = Math.min(cx, cy) - 8;
      const steps = 32;
      for (let i = 0; i < steps; i += 1) {
        const a = -Math.PI / 2 + (i / steps) * 2 * Math.PI;
        c.fillStyle = i / steps < u ? '#fc6' : '#333';
        c.fillRect(cx + Math.cos(a) * r - 2, cy + Math.sin(a) * r - 2, 4, 4);
      }
      c.fillText(`${Math.round(u * 100)}%`, cx - 10, cy + 4);
    },
    () => (p ? [{ x: view.surface.w / 2, y: view.surface.h / 2, hy: p }] : []),
  );
  return view;
}
