import { ChangeSet, Text } from '@codemirror/state';
import { describe, expect, it } from 'vitest';
import { HISTORY_LIMIT, RevisionHistory } from '../../src/code/history';

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
});
