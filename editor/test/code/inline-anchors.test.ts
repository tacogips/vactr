import { describe, expect, it } from 'vitest';
import { Text } from '@codemirror/state';
import { scopeAnchors, slotAnchors } from '../../src/code/inline/anchors';

describe('inline anchors', () => {
  it('groups mapped slots at the final form line and skips unmapped spans', () => {
    const doc = Text.of(['(one', ' two', ')', 'outside']);
    const firstSrc = new Map([['z', { span: { start: 1, end: 2 }, rev: 1 }], ['a', { span: { start: 2, end: 3 }, rev: 1 }], ['outside', { span: { start: 12, end: 13 }, rev: 1 }], ['skip', { span: { start: 99, end: 100 }, rev: 1 }]]);
    expect(slotAnchors({ doc, firstSrc, forms: [{ from: 0, to: 11 }], mapSpan: (s) => s.start === 99 ? null : { from: s.start, to: s.end } })).toEqual(new Map([[2, ['a', 'z']], [3, ['outside']]]));
  });
  it('finds only valid oscilloscope directives', () => {
    expect(scopeAnchors(Text.of(['a', '#@ editor {"visualizers":[{"kind":"spectrum"}]}', 'c', '#@ editor {"visualizers":[{"kind":"oscilloscope"}]}']))).toEqual([3]);
  });
});
