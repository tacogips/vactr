import { describe, expect, it } from 'vitest';
import { diffLines } from '../../src/diff';

describe('diffLines', () => {
  it('handles identical and empty inputs', () => {
    expect(diffLines(['x'], ['x'])).toEqual([]);
    expect(diffLines([], ['x', 'y'])).toEqual([{ aFrom: 0, aTo: 0, bFrom: 0, bTo: 2 }]);
  });
  it('trims a 10k line change', () => {
    const a = Array.from({ length: 10000 }, (_, i) => String(i));
    const b = [...a]; b[5000] = 'changed';
    const start = performance.now();
    expect(diffLines(a, b)).toEqual([{ aFrom: 5000, aTo: 5001, bFrom: 5000, bTo: 5001 }]);
    console.log(`10k single change: ${(performance.now() - start).toFixed(3)} ms`);
  });
  it('finds insertions and separate end edits', () => {
    expect(diffLines(['a', 'b'], ['a', 'x', 'y', 'z', 'b'])).toEqual([{ aFrom: 1, aTo: 1, bFrom: 1, bTo: 4 }]);
    expect(diffLines(['a', 'b', 'c'], ['b', 'c', 'd'])).toEqual([{ aFrom: 0, aTo: 1, bFrom: 0, bTo: 0 }, { aFrom: 3, aTo: 3, bFrom: 2, bTo: 3 }]);
  });
  it('falls back to one trimmed hunk at the cost cap', () => {
    const a = ['same', 'a', 'k1', 'b', 'k2', 'c', 'k3', 'd', 'k4', 'e', 'end'];
    const b = ['same', 'A', 'k1', 'B', 'k2', 'C', 'k3', 'D', 'k4', 'E', 'end'];
    expect(diffLines(a, b, { maxCost: 2 })).toEqual([{ aFrom: 1, aTo: 10, bFrom: 1, bTo: 10 }]);
    expect(diffLines(a, b)).toEqual([
      { aFrom: 1, aTo: 2, bFrom: 1, bTo: 2 },
      { aFrom: 3, aTo: 4, bFrom: 3, bTo: 4 },
      { aFrom: 5, aTo: 6, bFrom: 5, bTo: 6 },
      { aFrom: 7, aTo: 8, bFrom: 7, bTo: 8 },
      { aFrom: 9, aTo: 10, bFrom: 9, bTo: 10 },
    ]);
  });
  it('reconstructs b across 200 seeded cases', () => {
    let seed = 123456;
    const random = () => { seed |= 0; seed = (seed + 0x6d2b79f5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t ^= t + Math.imul(t ^ (t >>> 7), 61 | t); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
    for (let trial = 0; trial < 200; trial++) {
      const make = () => Array.from({ length: Math.floor(random() * 30) }, () => String(Math.floor(random() * 8)));
      const a = make(), b = make(), hunks = diffLines(a, b);
      const reconstructed: string[] = []; let pos = 0, bpos = 0, edits = 0;
      for (const h of hunks) {
        expect(h.aFrom).toBeGreaterThanOrEqual(pos); expect(h.bFrom).toBeGreaterThanOrEqual(bpos);
        reconstructed.push(...a.slice(pos, h.aFrom), ...b.slice(h.bFrom, h.bTo));
        edits += h.aTo - h.aFrom + h.bTo - h.bFrom; pos = h.aTo; bpos = h.bTo;
      }
      reconstructed.push(...a.slice(pos));
      expect(reconstructed).toEqual(b); expect(edits).toBeLessThanOrEqual(a.length + b.length);
    }
  });
});
