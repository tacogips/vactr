// `euclid-ring` (design 15.1.7, 13.5): the euclidean rhythm on a ring
// with hits, steps and rotation handles. Rotation writes wrap modulo the
// current step count.

import { euclidPattern, wrap } from './curves';
import { dot, findHandle, standardView, type KindCtx, type KindView, type Node2D } from './handles';

export interface EuclidView extends KindView {
  /** Moves the rotation by `by` steps (wrapping); returns the written value. */
  rotate(by: number): number | null;
  pattern(): boolean[];
}

export function render(el: HTMLElement, ctx: KindCtx): EuclidView {
  const hits = findHandle(ctx.handles, 'hits') ?? ctx.handles[0];
  const steps = findHandle(ctx.handles, 'steps') ?? ctx.handles[1];
  const rot = findHandle(ctx.handles, /rot/) ?? ctx.handles[2];
  const n = (): number => Math.max(1, Math.round(steps?.value ?? 8));
  if (rot) rot.snap = (v) => wrap(v, n());
  if (hits) hits.snap = (v) => Math.min(Math.max(0, v), n());
  const pattern = (): boolean[] => euclidPattern(hits?.value ?? 0, n(), rot?.value ?? 0);
  const at = (i: number, w: number, h: number, k: number): [number, number] => {
    const r = Math.min(w, h) / 2 - 10;
    const a = -Math.PI / 2 + (i / k) * 2 * Math.PI;
    return [w / 2 + Math.cos(a) * r, h / 2 + Math.sin(a) * r];
  };
  const view = standardView(
    el,
    'euclid',
    ctx.handles,
    (s) => {
      const c = s.ctx;
      if (!c) return;
      const p = pattern();
      p.forEach((hit, i) => {
        const [x, y] = at(i, s.w, s.h, p.length);
        c.fillStyle = hit ? '#fc6' : '#333';
        c.fillRect(x - 3, y - 3, 6, 6);
      });
      c.fillText(`${hits?.value ?? 0}/${n()}`, s.w / 2 - 10, s.h / 2 + 4);
      const [rx, ry] = at(rot?.value ?? 0, s.w, s.h, n());
      dot(c, rx, ry, rot?.enabled ?? false, 'rot');
    },
    () => {
      const s = view.surface;
      const out: Node2D[] = [];
      if (hits) out.push({ x: s.w / 2, y: s.h / 2, hy: hits });
      if (steps) out.push({ x: s.w - 8, y: s.h / 2, hy: steps });
      if (rot) {
        const [x, y] = at(rot.value, s.w, s.h, n());
        out.push({ x, y, hx: rot });
      }
      return out;
    },
  );
  return {
    update: () => view.update(),
    dispose: () => view.dispose(),
    pattern,
    rotate(by) {
      if (!rot) return null;
      const v = rot.set(rot.value + by);
      view.update();
      return v;
    },
  };
}
