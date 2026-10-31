import { ChangeSet, Text } from '@codemirror/state';
import { describe, expect, it } from 'vitest';
import { LineFormatSet } from '../../src/decor';

describe('line format sets', () => {
  it('maps an anchor after inserted lines and resolves zero-based lines', () => {
    const set = new LineFormatSet(() => 7);
    set.set([{ pos: 5, style: {} }]);
    const changes = ChangeSet.of({ from: 0, insert: 'x\n' }, 7);
    set.map(changes);
    expect(set.resolve(Text.of(['x', 'one', 'two']))[0]!.line).toBe(2);
  });
  it('removes anchors deleted inside a change', () => {
    const set = new LineFormatSet(() => 7);
    set.set([{ pos: 5, style: {} }]);
    set.map(ChangeSet.of({ from: 4, to: 7 }, 7));
    expect(set.formats).toEqual([]);
  });
  it('rejects invalid anchors', () => {
    const set = new LineFormatSet(() => 7);
    expect(() => set.set([{ pos: 8, style: {} }])).toThrow(RangeError);
  });
});
