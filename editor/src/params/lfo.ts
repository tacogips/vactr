// `lfo-shape` (design 15.1.7, 13.5): the shape named by the signal call
// (`sine`, `saw`, `tri`, `square`, `rand`, `perlin`; effects with an
// internal LFO draw a sine), with handles for the call's numeric sites
// and for an adjacent `range` call's sites in the same chain.

import { lfoAt, lfoShape } from './curves';
import { polyline, siteHandle, standardView, type Handle, type KindCtx, type KindView } from './handles';

/** The call's handles plus the chain's `range` lo/hi handles. */
export function lfoHandles(ctx: KindCtx): Handle[] {
  const out = [...ctx.handles];
  const range = ctx.chain.find((g) => g.name === 'range');
  const editors = ctx.deps.store.manifest?.editors;
  range?.sites.slice(0, 2).forEach((s, i) => out.push(siteHandle(s.id, i === 0 ? 'range lo' : 'range hi', ctx.host, editors)));
  return out;
}

export function render(el: HTMLElement, ctx: KindCtx): KindView {
  const shape = lfoShape(ctx.group.name);
  const handles = lfoHandles(ctx);
  const range = ctx.chain.find((g) => g.name === 'range');
  const lo = range ? handles.find((h) => h.siteId === range.sites[0]?.id) : undefined;
  const hi = range ? handles.find((h) => h.siteId === range.sites[1]?.id) : undefined;
  return standardView(el, 'lfo', handles, (s) => {
    const c = s.ctx;
    if (!c) return;
    const a = lo?.value ?? -1;
    const b = hi?.value ?? 1;
    const span = Math.max(Math.abs(a), Math.abs(b), 1);
    const pts: [number, number][] = [];
    for (let x = 0; x <= s.w; x += 2) {
      const v = a + ((lfoAt(shape, (x / s.w) * 2) + 1) / 2) * (b - a);
      pts.push([x, s.h / 2 - (v / span) * (s.h / 2 - 4)]);
    }
    polyline(c, pts);
    c.fillText(shape, 4, 12);
  });
}
