import type { Rgba } from './theme';

type StorageLike = Pick<Storage, 'getItem' | 'setItem'>;
export const DEFAULT_UNAPPLIED_COLOR = '#facc15';
const KEY = 'vactr.unappliedColor';
export const validUnappliedColor = (value: string): boolean => /^#[0-9a-f]{6}$/i.test(value);
export function readUnappliedColor(storage: StorageLike | null): string {
  try { const value = storage?.getItem(KEY); return value && validUnappliedColor(value) ? value : DEFAULT_UNAPPLIED_COLOR; }
  catch { return DEFAULT_UNAPPLIED_COLOR; }
}
export function saveUnappliedColor(storage: StorageLike | null, value: string): void {
  if (!validUnappliedColor(value)) return;
  try { storage?.setItem(KEY, value); } catch { /* Editing remains available without storage. */ }
}
export function unappliedRgba(color: string): Rgba {
  const value = validUnappliedColor(color) ? color : DEFAULT_UNAPPLIED_COLOR;
  return [parseInt(value.slice(1, 3), 16) / 255, parseInt(value.slice(3, 5), 16) / 255, parseInt(value.slice(5, 7), 16) / 255, 0.28];
}
