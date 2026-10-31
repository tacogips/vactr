import { expect, it } from 'vitest';
import { diffWords } from '../../src/diff';
it('returns changed character ranges', () => {
  expect(diffWords('let x = 1', 'let y = 1')).toEqual({ a: [{ from: 4, to: 5 }], b: [{ from: 4, to: 5 }] });
  expect(diffWords('same', 'same')).toEqual({ a: [], b: [] });
  expect(diffWords('😀 x', '😀 y')).toEqual({ a: [{ from: 3, to: 4 }], b: [{ from: 3, to: 4 }] });
});
it('treats long lines as whole', () => {
  expect(diffWords('a'.repeat(3000), 'b')).toEqual({ a: [{ from: 0, to: 3000 }], b: [{ from: 0, to: 1 }] });
});
