import { KEYMAP_MAX_BYTES, type KeymapStore, type KeymapStoreSaveResult } from 'emaki/vim';
import type { InvokeFn } from '../documents/tauri-templates';

export type { KeymapStore } from 'emaki/vim';
export const VIM_KEYMAP_KEY = 'vactr.vimKeymap';

const LEGACY_ACTION_IDS = ['evaluate', 'evaluateAll', 'hush', 'format', 'completion', 'diagnostic.next', 'diagnostic.prev'];
const LEGACY_PATTERN = new RegExp(`"vactr\\.(${LEGACY_ACTION_IDS.map((id) => id.replace('.', '\\.')).join('|')})"`, 'g');

/** Rewrites the seven exact `"vactr.<id>"` string tokens to `"emaki.<id>"`; all other text is kept. */
export function migrateLegacyActionIds(text: string): string {
  return text.replace(LEGACY_PATTERN, (_match, id: string) => `"emaki.${id}"`);
}

export class TauriKeymapStore implements KeymapStore {
  constructor(private readonly invoke: InvokeFn) {}

  async load(): Promise<string | null> {
    try {
      const value: unknown = await this.invoke('vim_keymap_get');
      return typeof value === 'string' ? value : null;
    } catch {
      return null;
    }
  }

  async save(text: string): Promise<KeymapStoreSaveResult> {
    if (new TextEncoder().encode(text).byteLength > KEYMAP_MAX_BYTES) return { ok: false, reason: 'too-large' };
    await this.invoke('vim_keymap_set', { text });
    return { ok: true };
  }

  async clear(): Promise<void> {
    await this.invoke('vim_keymap_reset');
  }
}
