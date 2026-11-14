import type { InvokeFn } from '../documents/tauri-templates';

export const VIM_KEYMAP_KEY = 'vactr.vimKeymap';
// Keep aligned with KEYMAP_MAX_BYTES in vim/keymap-schema.ts.
const MAX_BYTES = 262_144;

export type KeymapStoreResult =
  | { ok: true }
  | { ok: false; reason: 'too-large' | 'unavailable' | 'error'; message?: string };

export interface KeymapStore {
  get(): Promise<string | null>;
  set(text: string): Promise<KeymapStoreResult>;
  reset(): Promise<KeymapStoreResult>;
}

function isTooLarge(text: string): boolean {
  return new TextEncoder().encode(text).byteLength > MAX_BYTES;
}

export class LocalKeymapStore implements KeymapStore {
  constructor(private readonly storage: Pick<Storage, 'getItem' | 'setItem' | 'removeItem'> | null) {}

  async get(): Promise<string | null> {
    try {
      return this.storage?.getItem(VIM_KEYMAP_KEY) ?? null;
    } catch {
      return null;
    }
  }

  async set(text: string): Promise<KeymapStoreResult> {
    if (isTooLarge(text)) return { ok: false, reason: 'too-large' };
    try {
      if (!this.storage) return { ok: false, reason: 'unavailable' };
      this.storage.setItem(VIM_KEYMAP_KEY, text);
      return { ok: true };
    } catch {
      return { ok: false, reason: 'unavailable' };
    }
  }

  async reset(): Promise<KeymapStoreResult> {
    try {
      if (!this.storage) return { ok: false, reason: 'unavailable' };
      this.storage.removeItem(VIM_KEYMAP_KEY);
      return { ok: true };
    } catch {
      return { ok: false, reason: 'unavailable' };
    }
  }
}

export class TauriKeymapStore implements KeymapStore {
  constructor(private readonly invoke: InvokeFn) {}

  async get(): Promise<string | null> {
    try {
      const value: unknown = await this.invoke('vim_keymap_get');
      return typeof value === 'string' ? value : null;
    } catch {
      return null;
    }
  }

  async set(text: string): Promise<KeymapStoreResult> {
    if (isTooLarge(text)) return { ok: false, reason: 'too-large' };
    try {
      await this.invoke('vim_keymap_set', { text });
      return { ok: true };
    } catch (error) {
      return { ok: false, reason: 'error', message: String(error) };
    }
  }

  async reset(): Promise<KeymapStoreResult> {
    try {
      await this.invoke('vim_keymap_reset');
      return { ok: true };
    } catch (error) {
      return { ok: false, reason: 'error', message: String(error) };
    }
  }
}
