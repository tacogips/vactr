import type { StorageLike } from '../ui/shell';

export const INLINE_WIDGETS_KEY = 'vactr.inlineWidgets';

export function readInlineWidgets(storage: StorageLike | null): boolean {
  try { return storage?.getItem(INLINE_WIDGETS_KEY) !== 'false'; }
  catch { return true; }
}

export function writeInlineWidgets(storage: StorageLike | null, enabled: boolean): void {
  try { storage?.setItem(INLINE_WIDGETS_KEY, enabled ? 'true' : 'false'); }
  catch { /* Storage may be unavailable. */ }
}
