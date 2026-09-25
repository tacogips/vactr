// `xy-pad` (design 15.1.7, 13.5): any two numeric sites on one pad. The
// user picks the X and Y sites from the group's sites (default: its first
// two) or from any site of the file.

import type { WireSite } from '../protocol/types';
import { dot, siteHandle, standardView, type Handle, type KindCtx, type KindView } from './handles';

export interface XyView extends KindView {
  /** Re-binds the pad axes to two site ids. */
  pick(x: number | null, y: number | null): void;
  axes(): [Handle | null, Handle | null];
}

function describe(s: WireSite): string {
  const c = s.call;
  return c ? `${c.name} ${c.param ?? `#${c.arg}`} (${s.value})` : `site ${s.id} (${s.value})`;
}

export function render(el: HTMLElement, ctx: KindCtx): XyView {
  const doc = ctx.doc;
  const editors = ctx.deps.store.manifest?.editors;
  const all = (): WireSite[] => {
    const own = ctx.group.sites;
    const rest = ctx.deps.store.sitesOf(ctx.file).filter((s) => !own.some((o) => o.id === s.id));
    return [...own, ...rest];
  };
  const pickers = doc.createElement('div');
  pickers.className = 'params-xy-pick';
  el.appendChild(pickers);
  const select = (axis: string): HTMLSelectElement => {
    const sel = doc.createElement('select');
    sel.className = `params-xy-${axis}`;
    pickers.appendChild(sel);
    return sel;
  };
  const sx = select('x');
  const sy = select('y');
  let hx: Handle | null = null;
  let hy: Handle | null = null;
  let view: (KindView & { surface: { w: number; h: number } }) | null = null;
  const body = doc.createElement('div');
  el.appendChild(body);

  const build = (): void => {
    view?.dispose();
    const handles = [hx, hy].filter((h): h is Handle => h !== null);
    const v = standardView(
      body,
      'xy',
      handles,
      (s) => {
        if (!s.ctx) return;
        s.ctx.fillStyle = '#222';
        s.ctx.fillRect(0, 0, s.w, s.h);
        dot(s.ctx, (hx ? hx.unitPos : 0.5) * s.w, (1 - (hy ? hy.unitPos : 0.5)) * s.h, handles.length > 0);
      },
      () => {
        const n = { x: (hx ? hx.unitPos : 0.5) * v.surface.w, y: (1 - (hy ? hy.unitPos : 0.5)) * v.surface.h };
        return [{ ...n, ...(hx ? { hx } : {}), ...(hy ? { hy } : {}) }];
      },
    );
    view = v;
  };
  const fill = (): void => {
    for (const sel of [sx, sy]) {
      const keep = sel.value;
      sel.textContent = '';
      for (const s of all()) {
        const o = doc.createElement('option');
        o.value = String(s.id);
        o.textContent = describe(s);
        sel.appendChild(o);
      }
      if (keep) sel.value = keep;
    }
  };
  const pick = (x: number | null, y: number | null): void => {
    hx = x === null ? null : siteHandle(x, 'x', ctx.host, editors);
    hy = y === null ? null : siteHandle(y, 'y', ctx.host, editors);
    if (x !== null) sx.value = String(x);
    if (y !== null) sy.value = String(y);
    build();
  };
  fill();
  const own = ctx.group.sites;
  pick(own[0]?.id ?? null, own[1]?.id ?? null);
  const onPick = (): void => pick(sx.value ? Number(sx.value) : null, sy.value ? Number(sy.value) : null);
  sx.addEventListener('change', onPick);
  sy.addEventListener('change', onPick);
  return {
    pick,
    axes: () => [hx, hy],
    update() {
      fill();
      view?.update();
    },
    dispose() {
      view?.dispose();
      pickers.remove();
      body.remove();
    },
  };
}
