import { describe, expect, it } from 'vitest';
import { Heightmap } from '../../src/layout/heightmap';

describe('Heightmap', () => {
  it('preserves fixed line geometry without blocks', () => {
    const map = new Heightmap(18);
    map.setLineCount(10);
    expect(map.lineTop(5)).toBe(90);
    expect(map.lineAtHeight(91)).toEqual({ line: 5, region: 'text' });
    expect(map.totalHeight()).toBe(180);
    for (let line = 0; line < 10; line++) expect(map.lineTop(line)).toBe(line * 18);
  });

  it('maps the below band to its anchor', () => {
    const map = new Heightmap(18);
    map.setLineCount(10);
    map.setBlocks([{ line: 2, above: 0, below: 40 }]);
    expect(map.lineTop(3)).toBe(94);
    expect(map.lineAtHeight(60)).toEqual({ line: 2, region: 'below' });
    expect(map.lineAtHeight(94)).toEqual({ line: 3, region: 'text' });
    expect(map.lineAtHeight(map.totalHeight())).toEqual({ line: 9, region: 'text' });
  });

  it('maps an above band including negative heights to its anchor', () => {
    const map = new Heightmap(18);
    map.setLineCount(10);
    map.setBlocks([{ line: 0, above: 10, below: 0 }]);
    expect(map.lineAtHeight(5)).toEqual({ line: 0, region: 'above' });
    expect(map.lineAtHeight(-1)).toEqual({ line: 0, region: 'above' });
    expect(map.lineTop(0)).toBe(10);
  });

  it('merges duplicate entries and rejects invalid heights', () => {
    const map = new Heightmap(18);
    map.setLineCount(10);
    map.setBlocks([{ line: 2, above: 2, below: 3 }, { line: 2, above: 4, below: 5 }]);
    expect(map.blockCount).toBe(1);
    expect(map.blockAbove(2)).toBe(6);
    expect(map.blockBelow(2)).toBe(8);
    expect(() => map.setBlocks([{ line: 2, above: -1, below: 0 }])).toThrow(RangeError);
    expect(() => map.setBlocks([{ line: 2, above: 0, below: Infinity }])).toThrow(RangeError);
  });

  it('round trips sampled rows in a large sparse map', () => {
    const map = new Heightmap(18);
    map.setLineCount(100_000);
    const blocks = Array.from({ length: 5_000 }, (_, i) => ({ line: (i * 7919) % 100_000, above: i % 7, below: (i % 11) + 1 }));
    map.setBlocks(blocks);
    for (let i = 0; i < 1_000; i++) {
      const line = (i * 9973) % 100_000;
      expect(map.lineAtHeight(map.lineTop(line) + 1)).toEqual({ line, region: 'text' });
    }
  });

  it('drops blocks outside a reduced line count', () => {
    const map = new Heightmap(18);
    map.setLineCount(10);
    map.setBlocks([{ line: 1, above: 2, below: 0 }, { line: 8, above: 0, below: 20 }]);
    map.setLineCount(2);
    expect(map.blockCount).toBe(1);
    expect(map.totalHeight()).toBe(38);
  });

  it('increments revision only on effective changes', () => {
    const map = new Heightmap(18);
    expect(map.revision).toBe(0);
    map.setLineCount(10);
    const afterCount = map.revision;
    map.setLineCount(10);
    map.setLineHeight(18);
    expect(map.revision).toBe(afterCount);
    map.setBlocks([{ line: 2, above: 1, below: 3 }, { line: 2, above: 2, below: 1 }]);
    const afterBlocks = map.revision;
    map.setBlocks([{ line: 2, above: 3, below: 4 }, { line: 99, above: 1, below: 0 }]);
    expect(map.revision).toBe(afterBlocks);
    map.setLineHeight(20);
    expect(map.revision).toBe(afterBlocks + 1);
    map.setLineCount(2);
    expect(map.revision).toBe(afterBlocks + 2);
    expect(map.blockCount).toBe(0);
  });
});
