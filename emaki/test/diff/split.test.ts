import { expect, it } from 'vitest';
import { alignSplit, diffLines } from '../../src/diff';
it('pairs modified lines and fills the shorter side', () => {
  expect(alignSplit([{ aFrom: 2, aTo: 4, bFrom: 2, bTo: 3 }], 5, 4)).toEqual({ removed: [3], added: [], modified: [{ a: 2, b: 2 }], fillers: [{ side: 'b', afterLine: 2, count: 1 }] });
});
it('places insertion fillers before the first line', () => {
  expect(alignSplit([{ aFrom: 0, aTo: 0, bFrom: 0, bTo: 2 }], 1, 3)).toEqual({ removed: [], added: [0, 1], modified: [], fillers: [{ side: 'a', afterLine: -1, count: 2 }] });
});
it('equalizes row totals for multiple hunks', () => {
  const a = ['a', 'b', 'c', 'd', 'e'], b = ['x', 'a', 'c', 'd', 'f', 'g'];
  const layout = alignSplit(diffLines(a, b), a.length, b.length);
  const rows = (side: 'a' | 'b', length: number) => length + layout.fillers.filter(f => f.side === side).reduce((n, f) => n + f.count, 0);
  expect(rows('a', a.length)).toBe(rows('b', b.length));
});
