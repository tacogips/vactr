import { describe, expect, it, vi } from 'vitest';
import { LocalKeymapStore } from 'emaki/vim';
import { migrateLegacyActionIds, TauriKeymapStore, VIM_KEYMAP_KEY } from '../../src/code/vim-keymap-store';
import type { InvokeFn } from '../../src/documents/tauri-templates';

type StorageLike = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;
const local = (storage: StorageLike) => new LocalKeymapStore({ key: VIM_KEYMAP_KEY, storage });

describe('LocalKeymapStore under the vactr key', () => {
  it('returns null for an empty storage', async () => {
    const store = local({ getItem: () => null, setItem: vi.fn(), removeItem: vi.fn() });
    await expect(store.load()).resolves.toBeNull();
  });

  it('stores and reads the raw JSON text under the pinned key', async () => {
    const values = new Map<string, string>();
    const store = local({
      getItem: (key) => values.get(key) ?? null,
      setItem: (key, value) => { values.set(key, value); },
      removeItem: (key) => { values.delete(key); },
    });
    expect(await store.save('{"version":1}')).toEqual({ ok: true });
    expect(values.get(VIM_KEYMAP_KEY)).toBe('{"version":1}');
    await expect(store.load()).resolves.toBe('{"version":1}');
  });

  it('clears the stored value', async () => {
    const values = new Map([[VIM_KEYMAP_KEY, 'text']]);
    const store = local({ getItem: (key) => values.get(key) ?? null, setItem: vi.fn(), removeItem: (key) => { values.delete(key); } });
    await store.clear();
    await expect(store.load()).resolves.toBeNull();
  });

  it('catches storage access errors', async () => {
    const store = local({
      getItem: () => { throw new Error('blocked'); },
      setItem: () => { throw new Error('blocked'); },
      removeItem: () => { throw new Error('blocked'); },
    });
    await expect(store.load()).resolves.toBeNull();
    await expect(store.save('x')).resolves.toEqual({ ok: false, reason: 'unavailable' });
    await expect(store.clear()).resolves.toBeUndefined();
  });

  it('rejects oversized UTF-8 text without writing', async () => {
    const setItem = vi.fn();
    const store = local({ getItem: vi.fn(), setItem, removeItem: vi.fn() });
    const result = await store.save('é'.repeat(131_073));
    expect(result).toEqual({ ok: false, reason: 'too-large' });
    expect(setItem).not.toHaveBeenCalled();
  });
});

describe('TauriKeymapStore', () => {
  it('loads text and returns null for non-string values', async () => {
    const invoke = vi.fn<InvokeFn>().mockResolvedValueOnce('{"version":1}').mockResolvedValueOnce(42);
    const store = new TauriKeymapStore(invoke as unknown as InvokeFn);
    await expect(store.load()).resolves.toBe('{"version":1}');
    await expect(store.load()).resolves.toBeNull();
    expect(invoke).toHaveBeenCalledWith('vim_keymap_get');
  });

  it('saves text with the pinned command and argument', async () => {
    const invoke = vi.fn<InvokeFn>().mockResolvedValue(undefined);
    expect(await new TauriKeymapStore(invoke as unknown as InvokeFn).save('json')).toEqual({ ok: true });
    expect(invoke).toHaveBeenCalledWith('vim_keymap_set', { text: 'json' });
  });

  it('clears through the pinned command', async () => {
    const invoke = vi.fn<InvokeFn>().mockResolvedValue(undefined);
    await new TauriKeymapStore(invoke as unknown as InvokeFn).clear();
    expect(invoke).toHaveBeenCalledWith('vim_keymap_reset');
  });

  it('returns null on a failed load and rejects failed save and clear with the invoke error', async () => {
    const invoke = vi.fn<InvokeFn>().mockRejectedValue(new Error('invoke failed'));
    const store = new TauriKeymapStore(invoke as unknown as InvokeFn);
    await expect(store.load()).resolves.toBeNull();
    await expect(store.save('x')).rejects.toThrow('invoke failed');
    await expect(store.clear()).rejects.toThrow('invoke failed');
  });

  it('rejects oversized text before invoking Tauri', async () => {
    const invoke = vi.fn<InvokeFn>();
    const result = await new TauriKeymapStore(invoke as unknown as InvokeFn).save('x'.repeat(262_145));
    expect(result).toEqual({ ok: false, reason: 'too-large' });
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe('migrateLegacyActionIds', () => {
  const ids = ['evaluate', 'evaluateAll', 'hush', 'format', 'completion', 'diagnostic.next', 'diagnostic.prev'];

  it('rewrites each of the seven legacy action IDs', () => {
    for (const id of ids) expect(migrateLegacyActionIds(`{ "action": "vactr.${id}" }`)).toBe(`{ "action": "emaki.${id}" }`);
  });

  it('keeps other text, formatting and non-exact tokens unchanged', () => {
    const text = '{\n  "desc": "vactr.evaluate runs code",\n  "action": "vactr.unknown",\n  "x": "vactr.evaluateX"\n}';
    expect(migrateLegacyActionIds(text)).toBe(text);
  });

  it('is idempotent', () => {
    const once = migrateLegacyActionIds('["vactr.hush", "vactr.diagnostic.next"]');
    expect(migrateLegacyActionIds(once)).toBe(once);
    expect(once).toBe('["emaki.hush", "emaki.diagnostic.next"]');
  });
});
