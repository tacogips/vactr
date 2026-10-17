import { describe, expect, it, vi } from 'vitest';
import {
  LocalKeymapStore,
  TauriKeymapStore,
  VIM_KEYMAP_KEY,
  type KeymapStoreResult,
} from '../../src/code/vim-keymap-store';
import type { InvokeFn } from '../../src/documents/tauri-templates';

describe('LocalKeymapStore', () => {
  it('returns null for an empty storage', async () => {
    const store = new LocalKeymapStore({ getItem: () => null, setItem: vi.fn(), removeItem: vi.fn() });
    await expect(store.get()).resolves.toBeNull();
  });

  it('stores and reads the raw JSON text under the pinned key', async () => {
    const values = new Map<string, string>();
    const store = new LocalKeymapStore({
      getItem: (key) => values.get(key) ?? null,
      setItem: (key, value) => values.set(key, value),
      removeItem: (key) => values.delete(key),
    });
    expect(await store.set('{"version":1}')).toEqual({ ok: true });
    expect(values.get(VIM_KEYMAP_KEY)).toBe('{"version":1}');
    await expect(store.get()).resolves.toBe('{"version":1}');
  });

  it('resets the stored value', async () => {
    const values = new Map([[VIM_KEYMAP_KEY, 'text']]);
    const store = new LocalKeymapStore({ getItem: (key) => values.get(key) ?? null, setItem: vi.fn(), removeItem: (key) => values.delete(key) });
    expect(await store.reset()).toEqual({ ok: true });
    await expect(store.get()).resolves.toBeNull();
  });

  it('returns null when storage is unavailable', async () => {
    await expect(new LocalKeymapStore(null).get()).resolves.toBeNull();
  });

  it('reports unavailable storage on write and reset', async () => {
    const store = new LocalKeymapStore(null);
    await expect(store.set('x')).resolves.toEqual({ ok: false, reason: 'unavailable' });
    await expect(store.reset()).resolves.toEqual({ ok: false, reason: 'unavailable' });
  });

  it('catches storage access errors', async () => {
    const store = new LocalKeymapStore({
      getItem: () => { throw new Error('blocked'); },
      setItem: () => { throw new Error('blocked'); },
      removeItem: () => { throw new Error('blocked'); },
    });
    await expect(store.get()).resolves.toBeNull();
    await expect(store.set('x')).resolves.toEqual({ ok: false, reason: 'unavailable' });
    await expect(store.reset()).resolves.toEqual({ ok: false, reason: 'unavailable' });
  });

  it('rejects oversized UTF-8 text without writing', async () => {
    const setItem = vi.fn();
    const store = new LocalKeymapStore({ getItem: vi.fn(), setItem, removeItem: vi.fn() });
    const result = await store.set('é'.repeat(131_073));
    expect(result).toEqual({ ok: false, reason: 'too-large' });
    expect(setItem).not.toHaveBeenCalled();
  });
});

describe('TauriKeymapStore', () => {
  it('gets text and returns null for non-string values', async () => {
    const invoke = vi.fn<InvokeFn>().mockResolvedValueOnce('{"version":1}').mockResolvedValueOnce(42);
    const store = new TauriKeymapStore(invoke as unknown as InvokeFn);
    await expect(store.get()).resolves.toBe('{"version":1}');
    await expect(store.get()).resolves.toBeNull();
    expect(invoke).toHaveBeenCalledWith('vim_keymap_get');
  });

  it('sets text with the pinned command and argument', async () => {
    const invoke = vi.fn<InvokeFn>().mockResolvedValue(undefined);
    expect(await new TauriKeymapStore(invoke as unknown as InvokeFn).set('json')).toEqual({ ok: true });
    expect(invoke).toHaveBeenCalledWith('vim_keymap_set', { text: 'json' });
  });

  it('resets through the pinned command', async () => {
    const invoke = vi.fn<InvokeFn>().mockResolvedValue(undefined);
    expect(await new TauriKeymapStore(invoke as unknown as InvokeFn).reset()).toEqual({ ok: true });
    expect(invoke).toHaveBeenCalledWith('vim_keymap_reset');
  });

  it('returns errors instead of throwing for failed commands', async () => {
    const invoke = vi.fn<InvokeFn>().mockRejectedValue(new Error('invoke failed'));
    const store = new TauriKeymapStore(invoke as unknown as InvokeFn);
    await expect(store.get()).resolves.toBeNull();
    await expect(store.set('x')).resolves.toEqual({ ok: false, reason: 'error', message: 'Error: invoke failed' });
    await expect(store.reset()).resolves.toEqual({ ok: false, reason: 'error', message: 'Error: invoke failed' });
  });

  it('rejects oversized text before invoking Tauri', async () => {
    const invoke = vi.fn<InvokeFn>();
    const result: KeymapStoreResult = await new TauriKeymapStore(invoke as unknown as InvokeFn).set('x'.repeat(262_145));
    expect(result).toEqual({ ok: false, reason: 'too-large' });
    expect(invoke).not.toHaveBeenCalled();
  });
});
