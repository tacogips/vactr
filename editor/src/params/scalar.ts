// `scalar` (design 15.1.7): sliders only, for a call with no declaration
// or an unknown kind.

import { handleRows, type KindCtx, type KindView } from './handles';

export function render(el: HTMLElement, ctx: KindCtx): KindView {
  return handleRows(el, ctx.handles, () => {});
}
