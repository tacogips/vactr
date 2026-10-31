import { ChangeSet, Text } from '@codemirror/state';
import { describe, expect, it } from 'vitest';
import { DecorationStore, MarkSet } from '../../src/decor';

describe('mark sets', () => {
  it('maps insertions at and around endpoints without growing', () => {
    for (const [at, expected] of [[0, [3, 5]], [2, [3, 5]], [4, [2, 4]]] as const) {
      const set = new MarkSet(() => 6);
      set.set([{ from: 2, to: 4, style: {} }]);
      set.map(ChangeSet.of({ from: at, insert: 'x' }, 6));
      expect([set.marks[0]!.from, set.marks[0]!.to]).toEqual(expected);
    }
  });
  it('drops deleted marks except minimum-width marks', () => {
    const set = new MarkSet(() => 6);
    set.set([{ from: 2, to: 4, style: {} }, { from: 2, to: 4, style: { minWidth: 2 } }]);
    set.map(ChangeSet.of({ from: 1, to: 5 }, 6));
    expect(set.marks).toEqual([{ from: 1, to: 1, style: { minWidth: 2 } }]);
  });
  it('validates and isolates stored copies', () => {
    const set = new MarkSet(() => 6);
    expect(() => set.set([{ from: 0, to: 7, style: {} }])).toThrow(RangeError);
    const mark = { from: 1, to: 2, style: { opacity: 0.5 } };
    set.set([mark]); mark.style.opacity = 1;
    expect(set.marks[0]!.style.opacity).toBe(0.5);
    expect(Object.isFrozen(set.marks[0]!.style)).toBe(true);
  });
  it('collects owners and animated providers in registration order', () => {
    const store = new DecorationStore(() => 3);
    store.marks('a').set([{ from: 0, to: 1, style: {} }]);
    store.marks('b').set([{ from: 1, to: 2, style: {} }]);
    const remove = store.addAnimated(() => [{ from: 2, to: 3, style: {} }]);
    const result = store.collect(0, Text.of(['abc']));
    expect(result.marks.map(m => m.from)).toEqual([0, 1]);
    expect(result.animatedMarks).toHaveLength(1);
    remove();
    expect(store.collect(0, Text.of(['abc'])).animatedMarks).toHaveLength(0);
    store.marks('a').clear();
    expect(store.collect(0, Text.of(['abc'])).key).not.toBe(result.key);
  });
});
