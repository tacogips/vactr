import { describe, expect, it } from 'vitest';
import { isApplePlatform, stopShortcut } from '../../src/ui/stop-keys';

describe('stopShortcut', () => {
  it('maps Apple Meta Period to gentle stop and Shift to cut', () => {
    expect(stopShortcut({ code: 'Period', key: '.', metaKey: true, ctrlKey: false, altKey: false, shiftKey: false }, true)).toBe('stop-all');
    expect(stopShortcut({ code: 'Period', key: '>', metaKey: true, ctrlKey: false, altKey: false, shiftKey: true }, true)).toBe('cut');
  });

  it('requires the platform modifier and rejects Alt', () => {
    expect(stopShortcut({ code: 'Period', key: '.', metaKey: false, ctrlKey: true, altKey: false, shiftKey: false }, true)).toBeNull();
    expect(stopShortcut({ code: 'Period', key: '.', metaKey: false, ctrlKey: true, altKey: false, shiftKey: false }, false)).toBe('stop-all');
    expect(stopShortcut({ code: 'Period', key: '>', metaKey: false, ctrlKey: true, altKey: false, shiftKey: true }, false)).toBe('cut');
    expect(stopShortcut({ code: 'Period', key: '.', metaKey: true, ctrlKey: false, altKey: false, shiftKey: false }, false)).toBeNull();
    expect(stopShortcut({ code: 'Period', key: '.', metaKey: true, ctrlKey: false, altKey: true, shiftKey: false }, true)).toBeNull();
  });

  it('uses a key fallback only when code is empty', () => {
    expect(stopShortcut({ code: '', key: '.', metaKey: true, ctrlKey: false, altKey: false, shiftKey: false }, true)).toBe('stop-all');
    expect(stopShortcut({ code: '', key: '>', metaKey: true, ctrlKey: false, altKey: false, shiftKey: true }, true)).toBe('cut');
    expect(stopShortcut({ code: 'KeyA', key: '.', metaKey: true, ctrlKey: false, altKey: false, shiftKey: false }, true)).toBeNull();
  });

  it('detects Mac and iPad platform names', () => {
    expect(isApplePlatform('MacIntel')).toBe(true);
    expect(isApplePlatform('iPad')).toBe(true);
    expect(isApplePlatform('Linux x86_64')).toBe(false);
  });
});
