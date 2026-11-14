import { describe, expect, it, vi } from 'vitest';
import { GLIDE_DEFAULT_MS, GLIDE_KEY, mountGlideSetting, readGlide, writeGlide } from '../../src/bind/glide-setting';

describe('momentary glide preference', () => {
  it('defaults, clamps, rounds and tolerates unavailable storage', () => {
    localStorage.removeItem(GLIDE_KEY);
    expect(readGlide()).toBe(GLIDE_DEFAULT_MS);
    expect(writeGlide(12345)).toBe(10000);
    expect(writeGlide(333)).toBe(350);
    const broken = { getItem: () => { throw new Error('blocked'); }, setItem: () => { throw new Error('blocked'); } } as unknown as Storage;
    expect(readGlide(broken)).toBe(GLIDE_DEFAULT_MS);
    expect(writeGlide(500, broken)).toBe(500);
  });
  it('mounts labelled number/range inputs and persists changes', () => {
    const root = document.createElement('div'); document.body.append(root);
    const changed = vi.fn(); const mounted = mountGlideSetting(root, changed);
    const number = root.querySelector('input[type=number]') as HTMLInputElement;
    const range = root.querySelector('input[type=range]') as HTMLInputElement;
    expect(root.textContent).toContain('momentary glide (ms)');
    expect(number.min).toBe('0'); expect(number.max).toBe('10000'); expect(number.step).toBe('50');
    range.value = '333'; range.dispatchEvent(new Event('input'));
    expect(changed).toHaveBeenCalledWith(350); expect(localStorage.getItem(GLIDE_KEY)).toBe('350');
    expect(root.querySelector('.vact-glide-setting')).not.toBeNull();
    mounted.dispose(); root.remove();
  });
});
