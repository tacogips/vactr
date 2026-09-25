import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DOC_DEBOUNCE_MS, DocSync, composeChanges } from '../../src/protocol/document';
import type { ByteChange, ClientMsg, DocChangedBody } from '../../src/protocol/types';

function sync(): { doc: DocSync; sent: ClientMsg[] } {
  const sent: ClientMsg[] = [];
  return { doc: new DocSync('a.vact', (m) => sent.push(m)), sent };
}

function body(m: ClientMsg | undefined): DocChangedBody {
  if (m?.kind !== 'doc-changed') throw new Error('not doc-changed');
  return m.body;
}

/** Applies a change list (base offsets) whose inserted bytes are taken from `next`. */
function apply(base: string, next: string, changes: readonly ByteChange[]): string {
  let out = '';
  let pos = 0;
  let delta = 0;
  for (const c of changes) {
    out += base.slice(pos, c.from);
    const start = c.from + delta;
    out += next.slice(start, start + c.insert_len);
    pos = c.to;
    delta += c.insert_len - (c.to - c.from);
  }
  return out + base.slice(pos);
}

/** Applies one edit to an ASCII text, returning the new text and its change. */
function splice(text: string, from: number, to: number, insert: string): [string, ByteChange] {
  return [text.slice(0, from) + insert + text.slice(to), { from, to, insert_len: insert.length }];
}

describe('DocSync', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('bumps epoch and revision synchronously, before the debounce', () => {
    const { doc, sent } = sync();
    expect([doc.revision, doc.epoch]).toEqual([1, 0]);
    doc.edit([{ from: 0, to: 0, insert_len: 1 }], [{ start: 0, end: 1 }]);
    expect([doc.revision, doc.epoch]).toEqual([2, 1]);
    expect(doc.stamp()).toEqual({ doc_revision: 2, edit_epoch: 1 });
    expect(sent).toHaveLength(0);
  });

  it('fires the debounce at 200 ms and re-arms on each edit', () => {
    const { doc, sent } = sync();
    doc.edit([{ from: 0, to: 0, insert_len: 1 }], [{ start: 0, end: 1 }]);
    vi.advanceTimersByTime(DOC_DEBOUNCE_MS - 50);
    doc.edit([{ from: 1, to: 1, insert_len: 1 }], [{ start: 1, end: 2 }]);
    vi.advanceTimersByTime(DOC_DEBOUNCE_MS - 1);
    expect(sent).toHaveLength(0);
    vi.advanceTimersByTime(1);
    expect(sent).toHaveLength(1);
    expect(body(sent[0])).toEqual({
      file: 'a.vact',
      doc_revision: 3,
      base_revision: 1,
      changes: [{ from: 0, to: 0, insert_len: 2 }],
      dirty: [{ start: 0, end: 2 }],
      edit_epoch: 2,
    });
    vi.advanceTimersByTime(1000);
    expect(sent).toHaveLength(1);
  });

  it('composes two edits into base-revision offsets', () => {
    const { doc, sent } = sync();
    const base = 'hello world, goodbye';
    const [mid, c1] = splice(base, 6, 11, 'there'); // hello there, goodbye
    const [next, c2] = splice(mid, 13, 20, 'ciao'); // hello there, ciao
    doc.edit([c1], [{ start: 6, end: 11 }]);
    doc.edit([c2], [{ start: 13, end: 17 }]);
    expect(doc.flush()).toBe(true);
    const b = body(sent[0]);
    expect(b.changes).toEqual([
      { from: 6, to: 11, insert_len: 5 },
      { from: 13, to: 20, insert_len: 4 },
    ]);
    expect(b.dirty).toEqual([
      { start: 6, end: 11 },
      { start: 13, end: 17 },
    ]);
    expect(apply(base, next, b.changes)).toBe(next);
  });

  it('merges overlapping edits and maps earlier dirty spans', () => {
    const { doc, sent } = sync();
    const base = 'abcdef';
    const [mid, c1] = splice(base, 2, 2, 'XYZ'); // abXYZcdef
    const [next, c2] = splice(mid, 0, 3, ''); // YZcdef
    doc.edit([c1], [{ start: 2, end: 5 }]);
    doc.edit([c2], [{ start: 0, end: 0 }]);
    doc.flush();
    const b = body(sent[0]);
    expect(apply(base, next, b.changes)).toBe(next);
    expect(b.dirty).toEqual([{ start: 0, end: 2 }]);
  });

  it('tracks base and new revisions across flushes', () => {
    const { doc, sent } = sync();
    doc.edit([{ from: 0, to: 0, insert_len: 1 }], [{ start: 0, end: 1 }]);
    doc.edit([{ from: 0, to: 1, insert_len: 0 }], [{ start: 0, end: 0 }]);
    doc.flush();
    expect(body(sent[0])).toMatchObject({ base_revision: 1, doc_revision: 3 });
    expect(doc.flush()).toBe(false);
    doc.edit([{ from: 0, to: 0, insert_len: 2 }], [{ start: 0, end: 2 }]);
    doc.flush();
    expect(body(sent[1])).toMatchObject({ base_revision: 3, doc_revision: 4, edit_epoch: 3 });
    expect(doc.baseRevision).toBe(4);
  });

  it('composes random edit sequences consistently', () => {
    let seed = 7;
    const rnd = (n: number): number => {
      seed = (seed * 1103515245 + 12345) & 0x7fffffff;
      return seed % n;
    };
    for (let round = 0; round < 200; round += 1) {
      const base = 'the quick brown fox jumps over the lazy dog';
      let text = base;
      let composed: ByteChange[] = [];
      for (let k = 0; k < 1 + rnd(6); k += 1) {
        const from = rnd(text.length + 1);
        const to = from + rnd(Math.min(6, text.length - from) + 1);
        const ins = 'QRSTUVW'.slice(0, rnd(5));
        const [next, c] = splice(text, from, to, ins);
        composed = composed.length === 0 ? [c] : composeChanges(composed, [c]);
        text = next;
        expect(apply(base, text, composed)).toBe(text);
      }
    }
  });
});
