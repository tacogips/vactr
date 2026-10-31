import type { Rect as CodeRect, CursorShape } from '../types';
import type { Rgba } from './palette';

export function cursorRect(shape: CursorShape, caret: CodeRect, next: CodeRect | null, fallbackWidth = 8): CodeRect {
  if (shape === 'bar') return caret;
  const right = next && next.top === caret.top && next.left > caret.left ? next.left : caret.left + fallbackWidth;
  return shape === 'block' ? { ...caret, right } : { ...caret, right, top: caret.bottom - 2 };
}

export function cursorColor(shape: CursorShape, base: Rgba): Rgba {
  return shape === 'block' ? [base[0] * 0.45, base[1] * 0.45, base[2] * 0.45, base[3] * 0.45] : base;
}
