import { ChangeSet, Text } from '@codemirror/state';
import { describe, expect, it } from 'vitest';
import { HISTORY_LIMIT, RevisionHistory } from '../../src/code/history';
import { LineTable } from '../../src/code/line-bytes';

/** Applies one change spec and records it as the next revision. */
function edit(h: RevisionHistory, text: Text, spec: { from: number; to?: number; insert?: string }): Text {
  const cs = ChangeSet.of(spec, text.length);
  const next = cs.apply(text);
  h.record(h.current + 1, cs, next);
  return next;
}

const doc = (s: string): Text => Text.of(s.split('\n'));

describe('RevisionHistory', () => {
  const base = 'let a 1\nd1 "bd sd"\n';
  // "bd sd" string literal span in revision 1 (UTF-16).
  const lit = { from: base.indexOf('"bd'), to: base.indexOf('"\n') + 1 };

  it('maps across an insertion above', () => {
    const h = new RevisionHistory(doc(base));
    let t = doc(base);
    t = edit(h, t, { from: 0, insert: '# über\n' });
    const r = h.mapSpan(lit, 1);
    expect(r).toEqual({ from: lit.from + 7, to: lit.to + 7 });
    expect(t.sliceString(r?.from ?? 0, r?.to ?? 0)).toBe('"bd sd"');
  });

  it('maps across a deletion above', () => {
    const h = new RevisionHistory(doc(base));
    let t = doc(base);
    t = edit(h, t, { from: 0, to: 8 });
    const r = h.mapSpan(lit, 1);
    expect(t.sliceString(r?.from ?? 0, r?.to ?? 0)).toBe('"bd sd"');
    // An edit that only abuts the span keeps it.
    t = edit(h, t, { from: r?.to ?? 0, insert: ' # x' });
    expect(h.mapSpan(lit, 1)).toEqual(r);
  });

  it('returns null for an edit inside the span', () => {
    const h = new RevisionHistory(doc(base));
    const t = doc(base);
    edit(h, t, { from: lit.from + 2, insert: '*2' });
    expect(h.mapSpan(lit, 1)).toBeNull();
    // Later revisions still map their own spans.
    expect(h.mapSpan({ from: 0, to: 3 }, 2)).toEqual({ from: 0, to: 3 });
  });

  it('returns null once the revision overflows the history', () => {
    const h = new RevisionHistory(doc(base));
    let t = doc(base);
    for (let i = 0; i < HISTORY_LIMIT; i += 1) t = edit(h, t, { from: t.length, insert: 'x' });
    expect(h.current).toBe(HISTORY_LIMIT + 1);
    expect(h.mapSpan(lit, 1)).toBeNull();
    expect(h.text(1)).toBeNull();
    // The oldest kept revision still maps.
    expect(h.mapSpan(lit, h.oldest)).toEqual(lit);
    expect(h.oldest).toBe(h.current - HISTORY_LIMIT + 1);
  });

  it('maps wire byte spans of an older revision with non-ASCII text', () => {
    const src = 'let é 1\nd1 "ü"\n';
    const h = new RevisionHistory(doc(src));
    let t = doc(src);
    const enc = new TextEncoder();
    const start = enc.encode(src.slice(0, src.indexOf('"'))).length;
    const end = start + enc.encode('"ü"').length;
    t = edit(h, t, { from: 0, insert: '# 日本\n' });
    const r = h.mapWireSpan({ start, end }, 1);
    expect(t.sliceString(r?.from ?? 0, r?.to ?? 0)).toBe('"ü"');
    // Out of range byte spans and unknown revisions are null.
    expect(h.mapWireSpan({ start: 0, end: 999 }, 1)).toBeNull();
    expect(h.mapWireSpan({ start: 0, end: 1 }, 7)).toBeNull();
  });

  it('keeps a pinned evaluated span mappable after 300 edits while the unpinned control expires', () => {
    const initial = Text.of(['playing-span', ...Array.from({ length: 19_998 }, (_, i) => `line ${i}`), 'x']);
    const span = { start: 0, end: new TextEncoder().encode('playing-span').length };
    const pinned = new RevisionHistory(initial, 1, 999);
    const unpinned = new RevisionHistory(initial, 1, 999);
    expect(pinned.pin('eval', 1)).toBe(true);
    let pinnedText = initial, unpinnedText = initial;
    let prior = 'x';
    const lineLength = initial.line(initial.lines).length;
    const ceiling = 300 * (512 + 2 * (lineLength + 1) + 64 + 2);
    for (let i = 0; i < 300; i += 1) {
      const next = prior === 'x' ? 'y' : 'x';
      const from = pinnedText.line(pinnedText.lines).from;
      const cs = ChangeSet.of({ from, to: from + 1, insert: next }, pinnedText.length);
      pinnedText = cs.apply(pinnedText); pinned.record(pinned.current + 1, cs, pinnedText);
      const otherFrom = unpinnedText.line(unpinnedText.lines).from;
      const other = ChangeSet.of({ from: otherFrom, to: otherFrom + 1, insert: next }, unpinnedText.length);
      unpinnedText = other.apply(unpinnedText); unpinned.record(unpinned.current + 1, other, unpinnedText);
      prior = next;
    }
    expect(pinned.mapWireSpan(span, 1)).toEqual({ from: 0, to: 'playing-span'.length });
    expect(unpinned.mapWireSpan(span, 1)).toBeNull();
    expect(pinned.retainedBytes).toBeLessThanOrEqual(ceiling);
  });

  it('shares pins by revision, preserves the eval owner through runtime churn, and evicts the LRU at owner 33', () => {
    const makeHistory = (): RevisionHistory => {
      let text = doc(base);
      const h = new RevisionHistory(text);
      text = edit(h, text, { from: text.length, insert: 'x' });
      expect(h.current).toBe(2);
      expect(h.pin('eval', 1)).toBe(true);
      for (let i = 0; i < 8; i += 1) expect(h.pin(`playing:${i}`, 2)).toBe(true);
      for (let i = 0; i < 16; i += 1) expect(h.pin(`diag:runtime:${i}`, 2)).toBe(true);
      // Twenty-four owners share revision 2; eval remains separately pinned at revision 1.
      expect([...h.pinnedRevisions()].sort()).toEqual([1, 2]);
      return h;
    };

    const evictEval = makeHistory();
    for (let i = 25; i < 32; i += 1) expect(evictEval.pin(`owner:${i}`, 2)).toBe(true);
    expect(evictEval.pin('owner:32', 2)).toBe(true);
    expect(evictEval.stats.pinsEvicted).toBe(1);
    expect([...evictEval.pinnedRevisions()].sort()).toEqual([2]);

    const preserveEval = makeHistory();
    for (let i = 25; i < 32; i += 1) expect(preserveEval.pin(`owner:${i}`, 2)).toBe(true);
    expect(preserveEval.pin('eval', 1)).toBe(true);
    expect(preserveEval.pin('owner:32', 2)).toBe(true);
    expect(preserveEval.stats.pinsEvicted).toBe(1);
    expect([...preserveEval.pinnedRevisions()].sort()).toEqual([1, 2]);
  });

  it('drops all pins on a revision gap and maps a cancelling edit through the composed set', () => {
    const h = new RevisionHistory(doc(base));
    expect(h.pin('eval', 1)).toBe(true);
    let text = doc(base);
    const range = { from: lit.from, to: lit.to };
    text = edit(h, text, { from: lit.from + 1, insert: 'x' });
    text = edit(h, text, { from: lit.from + 1, to: lit.from + 2, insert: '' });
    expect(h.mapWireSpan({ start: new TextEncoder().encode(base.slice(0, lit.from)).length, end: new TextEncoder().encode(base.slice(0, lit.to)).length }, 1)).toEqual(range);
    h.record(h.current + 2, ChangeSet.empty(text.length), text);
    expect(h.pinnedRevisions()).toEqual([]);
  });

  it('enforces shared table bytes, refusal counts and on-demand eviction before refusing a pin', () => {
    const huge = Text.of(Array(1_048_576).fill(''));
    const revise = (h: RevisionHistory, text: Text, value: string): Text => edit(h, text, { from: text.length, insert: value });
    let text = huge;
    const h = new RevisionHistory(text);
    text = revise(h, text, 'a');
    expect(h.pin('a', 1)).toBe(true);
    const bytes = h.indexBytes;
    expect(h.pin('b', 1)).toBe(true);
    expect(h.indexBytes).toBe(bytes);
    expect(h.pin('c', 2)).toBe(false);
    expect(h.stats.pinsRefused).toBe(1);
    expect(h.indexBytes).toBeLessThanOrEqual(8 * 1024 * 1024);
    h.unpin('a'); h.unpin('b');
    expect(h.pin('c', 2)).toBe(true);

    let text2 = huge;
    const cached = new RevisionHistory(text2);
    text2 = revise(cached, text2, 'a');
    text2 = revise(cached, text2, 'b');
    expect(cached.lineTable(2)).not.toBeNull();
    expect(cached.pin('a', 1)).toBe(true);
    expect(cached.indexCount).toBe(1);
    expect(cached.stats.pinsRefused).toBe(0);
    expect(cached.lineTable(2)).not.toBeNull();
    expect(cached.indexCount).toBe(1);
    expect(cached.pinnedRevisions()).toEqual([1]);
  });
});
