// `xy-pad` (design 15.1.7, 13.5): any two numeric sites on one pad. The
// user picks the X and Y sites from the group's sites (default: its first
// two) or from any site of the file.

import { createComponent, createSignal } from 'solid-js';
import { render as renderSolid } from 'solid-js/web';
import { XyChrome, type AxisOption } from './kind-chrome';
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
  const [options, setOptions] = createSignal<AxisOption[]>([]);
  const [x, setX] = createSignal('');
  const [y, setY] = createSignal('');
  const holder = doc.createElement('div');
  const disposeChrome = renderSolid(() => createComponent(XyChrome, {
    options, x, y,
    onPick: (xValue, yValue) => pick(xValue ? Number(xValue) : null, yValue ? Number(yValue) : null),
  }), holder);
  el.append(...Array.from(holder.childNodes));
  const body = el.querySelector('.params-xy-body') as HTMLElement;
  let hx: Handle | null = null;
  let hy: Handle | null = null;
  let view: (KindView & { surface: { w: number; h: number } }) | null = null;

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
    setOptions(all().map((site) => ({ id: site.id, label: describe(site) })));
  };
  const pick = (xId: number | null, yId: number | null): void => {
    hx = xId === null ? null : siteHandle(xId, 'x', ctx.host, editors);
    hy = yId === null ? null : siteHandle(yId, 'y', ctx.host, editors);
    setX(xId === null ? '' : String(xId));
    setY(yId === null ? '' : String(yId));
    build();
  };
  fill();
  const own = ctx.group.sites;
  pick(own[0]?.id ?? null, own[1]?.id ?? null);
  return {
    pick,
    axes: () => [hx, hy],
    update() {
      fill();
      view?.update();
    },
    dispose() {
      view?.dispose();
      disposeChrome();
    },
  };
}
