import { describe, expect, it, vi } from 'vitest';
import { INLINE_WIDGETS_KEY, readInlineWidgets, writeInlineWidgets } from '../../src/code/inline-settings';

describe('inline widget setting', () => {
  it('defaults on and reads false', () => {
    expect(readInlineWidgets(null)).toBe(true);
    expect(readInlineWidgets({ getItem: () => 'false', setItem: vi.fn() })).toBe(false);
  });
  it('persists string booleans and tolerates inaccessible storage', () => {
    const setItem = vi.fn();
    writeInlineWidgets({ getItem: () => null, setItem }, true);
    writeInlineWidgets({ getItem: () => null, setItem }, false);
    expect(setItem.mock.calls).toEqual([[INLINE_WIDGETS_KEY, 'true'], [INLINE_WIDGETS_KEY, 'false']]);
    const storage = { getItem: (): string | null => { throw new Error('blocked'); }, setItem: (): void => { throw new Error('blocked'); } };
    expect(readInlineWidgets(storage)).toBe(true);
    expect(() => writeInlineWidgets(storage, false)).not.toThrow();
  });
});
