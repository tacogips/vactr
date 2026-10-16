import { describe, expect, it, vi } from 'vitest';
import { readVimEnabled, VIM_SETTING_KEY, writeVimEnabled } from '../../src/code/vim-settings';
import type { StorageLike } from '../../src/ui/shell';

describe('Vim setting storage', () => {
  it('defaults off when storage is absent', () => expect(readVimEnabled(null)).toBe(false));
  it('uses the plan setting key and accepts only the exact true value', () => {
    const getItem = vi.fn((key: string) => key === VIM_SETTING_KEY ? 'true' : '1');
    expect(readVimEnabled({ getItem, setItem: vi.fn() })).toBe(true);
    expect(getItem).toHaveBeenCalledWith('vactr.vimMode');
    expect(readVimEnabled({ getItem: () => '1', setItem: vi.fn() })).toBe(false);
  });
  it('defaults off when getItem throws', () => {
    const storage: StorageLike = { getItem: () => { throw new Error('blocked'); }, setItem: vi.fn() };
    expect(readVimEnabled(storage)).toBe(false);
  });
  it('writes string booleans and swallows setItem errors', () => {
    const setItem = vi.fn();
    writeVimEnabled({ getItem: () => null, setItem }, true);
    writeVimEnabled({ getItem: () => null, setItem }, false);
    expect(setItem.mock.calls).toEqual([[VIM_SETTING_KEY, 'true'], [VIM_SETTING_KEY, 'false']]);
    expect(() => writeVimEnabled({ getItem: () => null, setItem: () => { throw new Error('blocked'); } }, true)).not.toThrow();
  });
  it('round trips the enabled preference', () => {
    let value: string | null = null;
    const storage: StorageLike = { getItem: () => value, setItem: (_key, next) => { value = next; } };
    writeVimEnabled(storage, true);
    expect(readVimEnabled(storage)).toBe(true);
  });
});
