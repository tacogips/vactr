// `delay-taps` (design 15.1.7): beat-aligned taps. The time handle snaps
// to the nearest 1/4 beat of the current `tempo`; the feedback handle sets
// the tap decay.

import { beatSeconds, snapTime } from './curves';
import { dot, findHandle, standardView, type KindCtx, type KindView, type Node2D } from './handles';

export const TAPS = 6;
export const DEFAULT_BPM = 120;

export function render(el: HTMLElement, ctx: KindCtx): KindView {
  const time = findHandle(ctx.handles, /time|delay|^dt$/) ?? ctx.handles[0];
  const fb = findHandle(ctx.handles, /feedback|fb|decay/);
  const bpm = (): number => ctx.deps.store.tempo?.bpm ?? DEFAULT_BPM;
  if (time) time.snap = (v) => snapTime(v, time.unit, bpm());
  const secs = (): number => (time ? time.value / (time.unit === 'ms' ? 1000 : 1) : 0.25);
  const view = standardView(
    el,
    'delay',
    ctx.handles,
    (s) => {
      const c = s.ctx;
      if (!c) return;
      const window = Math.max(beatSeconds(bpm()) * 4, secs() * TAPS);
      c.fillStyle = '#333';
      for (let b = 0; b * beatSeconds(bpm()) <= window; b += 1) c.fillRect((b * beatSeconds(bpm()) / window) * s.w, 0, 1, s.h);
      const g = fb ? Math.min(0.99, Math.max(0, fb.unitPos)) : 0.5;
      for (let k = 1; k <= TAPS; k += 1) {
        const x = ((k * secs()) / window) * s.w;
        const h = g ** (k - 1) * (s.h - 8);
        c.fillStyle = '#8fd';
        c.fillRect(x, s.h - h, 3, h);
      }
      dot(c, (secs() / window) * s.w, 8, time?.enabled ?? false, 'time');
    },
    () => {
      const window = Math.max(beatSeconds(bpm()) * 4, secs() * TAPS);
      const n: Node2D = { x: (secs() / window) * view.surface.w, y: 8 };
      if (time) n.hx = time;
      if (fb) n.hy = fb;
      return [n];
    },
  );
  return view;
}
