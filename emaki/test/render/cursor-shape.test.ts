import { describe, expect, it } from 'vitest';
import { cursorColor, cursorRect } from '../../src/render/cursor-shape';
import type { Rect as CodeRect } from '../../src/types';

const caret: CodeRect = { left: 10, right: 11, top: 20, bottom: 40 };

describe('cursor shape geometry', () => {
  it('keeps the bar rectangle unchanged', () => {
    expect(cursorRect('bar', caret, null)).toEqual(caret);
  });

  it('uses the next same-line caret edge for a block', () => {
    expect(cursorRect('block', caret, { left: 26, right: 27, top: 20, bottom: 40 })).toEqual({ ...caret, right: 26 });
  });

  it('falls back to eight pixels when next is on a different line', () => {
    expect(cursorRect('block', caret, { left: 26, right: 27, top: 40, bottom: 60 })).toEqual({ ...caret, right: 18 });
  });

  it('falls back to eight pixels when the next rectangle is absent', () => {
    expect(cursorRect('block', caret, null)).toEqual({ ...caret, right: 18 });
  });

  it('falls back to eight pixels when next does not advance horizontally', () => {
    expect(cursorRect('block', caret, { left: 10, right: 11, top: 20, bottom: 40 })).toEqual({ ...caret, right: 18 });
  });

  it('draws underline as a two-pixel strip at the caret bottom', () => {
    expect(cursorRect('underline', caret, null)).toEqual({ left: 10, right: 18, top: 38, bottom: 40 });
  });

  it('uses the measured width of a wide grapheme', () => {
    expect(cursorRect('block', caret, { left: 26, right: 27, top: 20, bottom: 40 }).right - caret.left).toBe(16);
  });

  it('premultiplies only block cursor color', () => {
    const base = [0.9, 0.92, 0.94, 0.8] as const;
    expect(cursorColor('block', [...base])).toMatchObject([expect.closeTo(0.405), expect.closeTo(0.414), expect.closeTo(0.423), expect.closeTo(0.36)]);
    expect(cursorColor('bar', [...base])).toEqual([...base]);
    expect(cursorColor('underline', [...base])).toEqual([...base]);
  });
});
