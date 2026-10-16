import type { StorageLike } from '../ui/shell';

export const VIM_SETTING_KEY = 'vactr.vimMode';

export function readVimEnabled(storage: StorageLike | null): boolean {
  try {
    return storage?.getItem(VIM_SETTING_KEY) === 'true';
  } catch {
    return false;
  }
}

export function writeVimEnabled(storage: StorageLike | null, enabled: boolean): void {
  try {
    storage?.setItem(VIM_SETTING_KEY, enabled ? 'true' : 'false');
  } catch {
    // Storage may be unavailable in private or embedded browser contexts.
  }
}
