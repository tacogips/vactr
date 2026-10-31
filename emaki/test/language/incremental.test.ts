import { ChangeSet, Text } from '@codemirror/state';
import { describe, expect, it } from 'vitest';
import { IncrementalHighlighter } from '../../src/language/incremental';
import { jsonLanguage } from '../../src/language/json';
import type { LineLanguage } from '../../src/types';
const makeDoc = (count: number) => Text.of(Array.from({ length: count }, (_, i) => `"key${i}": ${i},`));
function makeHighlighter(doc: Text, options = {}) { const highlighter = new IncrementalHighlighter(jsonLanguage, { now: () => 0, ...options }); highlighter.setDoc(doc); return highlighter; }
describe('incremental highlighter', () => {
  it('tokenizes only requested initial lines', () => {
    const h = makeHighlighter(makeDoc(10000)); h.tokens(0, 50); expect(h.stats.linesTokenized).toBeLessThanOrEqual(50);
  });
  it('converges at the next checkpoint after an edit', () => {
    const doc = makeDoc(10000); const h = makeHighlighter(doc); h.tokens(0, 6000);
    const pos = doc.line(5001).from + 1; const changes = ChangeSet.of([{ from: pos, to: pos + 1, insert: 'z' }], doc.length);
    h.applyChanges(changes, changes.apply(doc)); const before = h.stats.linesTokenized; h.tokens(4990, 5010);
    expect(h.stats.linesTokenized - before).toBeLessThanOrEqual(64);
    h.tokens(5010, 5040); expect(h.stats.checkpointsValid).toBeGreaterThan(180);
  });
  it('bounds cold work and catches up to exact sequential results', () => {
    const doc = makeDoc(10000); const h = makeHighlighter(doc, { maxLinesPerFrame: 2000 });
    h.tokens(9000, 9050); expect(h.approximate).toBe(true);
    while (h.catchUp()) { /* bounded slices */ }
    const actual = h.tokens(9000, 9050); expect(h.approximate).toBe(false);
    const full = makeHighlighter(doc); expect(actual).toEqual(full.tokens(0, 9050).filter(t => t.from >= doc.line(9001).from));
  });
  it('recomputes changed downstream state lazily', () => {
    const lang: LineLanguage<number> = { kind: 'lines', name: 'count', startState: () => 0, sameState: (a,b) => a === b, tokenizeLine: (text, state) => ({ tokens: [{ from: 0, to: text.length, tag: String(state) }], state: state + (text === '{' ? 1 : 0) }) };
    const doc = Text.of(Array(200).fill('x')); const h = new IncrementalHighlighter(lang, { now: () => 0 }); h.setDoc(doc); h.tokens(0, 200);
    const changes = ChangeSet.of([{ from: doc.line(21).from, to: doc.line(21).to, insert: '{' }], doc.length); h.applyChanges(changes, changes.apply(doc));
    expect(h.tokens(100, 101)[0].tag).toBe('1');
  });
  it('does not converge before a later changed range', () => {
    const lang: LineLanguage<number> = { kind: 'lines', name: 'count', startState: () => 0, sameState: (a,b) => a === b, tokenizeLine: (text, state) => ({ tokens: [{ from: 0, to: text.length, tag: String(state) }], state: state + (text === '{' ? 1 : 0) }) };
    const doc = Text.of(Array(300).fill('x')); const h = new IncrementalHighlighter(lang, { now: () => 0 }); h.setDoc(doc); h.tokens(0, 300);
    const changes = ChangeSet.of([{ from: doc.line(11).from, to: doc.line(11).to, insert: 'y' }, { from: doc.line(201).from, to: doc.line(201).to, insert: '{' }], doc.length);
    h.applyChanges(changes, changes.apply(doc)); expect(h.tokens(250, 251)[0].tag).toBe('1');
  });
  it('does equal tokenization work after edits at different document sizes', () => {
    const work = (size: number) => { const doc = makeDoc(size); const h = makeHighlighter(doc); h.tokens(0, 600); const pos = doc.line(501).from; const changes = ChangeSet.of([{ from: pos, insert: 'z' }], doc.length); h.applyChanges(changes, changes.apply(doc)); const before = h.stats.linesTokenized; h.tokens(490, 510); return h.stats.linesTokenized - before; };
    expect(work(1000)).toBe(work(100000));
  });
});
