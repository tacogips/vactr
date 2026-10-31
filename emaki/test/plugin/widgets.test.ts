import { ChangeSet, Text } from '@codemirror/state';
import { describe, expect, it, vi } from 'vitest';
import { WidgetSet } from '../../src/plugin/widgets';
const content = { kind: 'dom' as const, mount: () => () => {} };
describe('WidgetSet', () => {
  it('maps anchors, reconciles keys, sums blocks and removes deleted anchors', () => {
    const doc = Text.of(['abc', 'def', 'ghi']); const set = new WidgetSet(doc.length); const removed = vi.fn(); set.onRemoved(removed);
    set.set('a', [{ key: 'x', pos: 5, side: 'below', height: 10, content }, { key: 'y', pos: 6, side: 'below', height: 20, content }]);
    expect(set.entries(doc)).toEqual([{ line: 1, above: 0, below: 30 }]);
    const revision = set.revision;
    set.set('a', [{ key: 'x', pos: 5, side: 'below', height: 15, content }]);
    expect(set.revision).toBeGreaterThan(revision); expect(set.blocks(doc)).toHaveLength(1); expect(removed).toHaveBeenCalledWith('a\0y');
    const insert = ChangeSet.of([{ from: 0, insert: 'new\n' }], doc.length); set.map(insert); const next = insert.apply(doc);
    expect(set.blocks(next)[0].line).toBe(2);
    const del = ChangeSet.of([{ from: 8, to: 12 }], next.length); set.map(del);
    expect(set.blocks(del.apply(next))).toHaveLength(0); expect(removed).toHaveBeenCalledWith('a\0x');
  });
  it('rejects invalid sizes and duplicate keys', () => {
    const set = new WidgetSet(1); const spec = { key: 'x', pos: 0, side: 'below' as const, height: -1, content };
    expect(() => set.set('a', [spec])).toThrow(RangeError);
    expect(() => set.set('a', [{ ...spec, height: 1 }, { ...spec, height: 1 }])).toThrow(RangeError);
  });
});
