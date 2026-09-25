// Criterion 10, the bindings half: edits above and inside bound forms keep
// bindings attached or drop them cleanly (non-ASCII text included),
// duplicate literals bind independently, a delayed CC after an edit sends
// `doc-changed` first and shows STALE on `edit-invalidated`, and write-back
// declines on a text mismatch.

import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fakeMidi, FILE, noDirectives, q, settle, setup, site, spanOf } from './fixtures';

afterEach(cleanup);

const TEXT = 'd1 s [:bd] > gain 0.5\nd2 s [:sd] > gain 0.7';

describe('binding reconciliation (criterion 10, bindings half)', () => {
  it('an insertion above keeps bindings attached and source-edits land on the moved literal', async () => {
    const h = setup(TEXT);
    h.evalResult([site(TEXT, '0.5', 1), site(TEXT, '0.7', 2)]);
    h.edit('d1', '# ünïcode 日本\nd1');
    expect(q(h.row(1), '.bind-state')).toBe('');
    h.area.writer.setMode(h.area.table.byId(2)?.bindingId ?? '', 'source-edit');
    h.transport.clear();
    h.deps.bind?.writeSite(2, 0.9);
    const next = '# ünïcode 日本\nd1 s [:bd] > gain 0.5\nd2 s [:sd] > gain 0.9';
    expect(h.text()).toBe(next);
    expect(h.transport.kinds()).toEqual(['doc-changed', 'eval']);
    expect(h.transport.of('eval')[0]?.body.span).toEqual(spanOf(next, 'd2 s [:sd] > gain 0.9'));
    // The reply re-keys both bindings at the new byte spans.
    h.evalResult([site(next, '0.5', 11), site(next, '0.9', 12)], { re: h.lastSeq('eval') });
    await settle();
    expect(h.area.table.byId(11)?.state).toBe('bound');
    expect(h.area.table.byId(12)?.literalText).toBe('0.9');
  });

  it('a deletion above and an insertion inside the form keep the bindings', () => {
    const h = setup(TEXT);
    h.evalResult([site(TEXT, '0.5', 1), site(TEXT, '0.7', 2)]);
    const second = h.area.table.byId(2)?.bindingId;
    h.edit('d1 s [:bd] > gain 0.5\n', '');
    h.edit('[:sd]', '[:sd :hh]');
    const next = h.text();
    expect(next).toBe('d2 s [:sd :hh] > gain 0.7');
    expect(h.area.table.get(second ?? '')?.state).toBe('bound');
    h.evalResult([site(next, '0.7', 5)]);
    expect(h.area.table.byId(5)?.bindingId).toBe(second);
    h.transport.clear();
    h.deps.bind?.writeSite(5, 0.8);
    expect(h.transport.of('set-tweak').map((e) => e.body.id)).toEqual([5]);
  });

  it('deleting a bound literal drops its binding cleanly', () => {
    const h = setup(TEXT);
    h.evalResult([site(TEXT, '0.5', 1), site(TEXT, '0.7', 2)]);
    const gone = h.area.table.byId(1)?.bindingId ?? '';
    h.edit('d1 s [:bd] > gain 0.5\n', '');
    expect(h.area.table.get(gone)?.state).toBe('stale');
    expect(h.area.panel.row(gone)?.dataset.state).toBe('stale');
    const next = h.text();
    h.evalResult([site(next, '0.7', 3)]);
    h.evalResult([site(next, '0.7', 4)]);
    expect(h.area.table.get(gone)).toBeUndefined();
    expect(h.area.panel.row(gone)).toBeUndefined();
    expect(h.area.table.all()).toHaveLength(1);
  });

  it('reordering forms keeps the untouched binding and re-binds the moved literal as new', () => {
    const h = setup(TEXT);
    h.evalResult([site(TEXT, '0.5', 1), site(TEXT, '0.7', 2)]);
    const stay = h.area.table.byId(1)?.bindingId;
    const moved = h.area.table.byId(2)?.bindingId ?? '';
    // Move line 2 above line 1.
    h.view.dispatch({ changes: [{ from: 0, insert: 'd2 s [:sd] > gain 0.7\n' }, { from: TEXT.indexOf('\nd2'), to: TEXT.length }] });
    const next = h.text();
    expect(next).toBe('d2 s [:sd] > gain 0.7\nd1 s [:bd] > gain 0.5');
    h.evalResult([site(next, '0.7', 21), site(next, '0.5', 22)]);
    expect(h.area.table.byId(22)?.bindingId).toBe(stay);
    expect(h.area.table.byId(21)?.bindingId).not.toBe(moved);
    expect(h.area.table.get(moved)?.state).toBe('stale');
  });

  it('duplicate literals bind independently', () => {
    const text = 's [:bd] > gain 0.5 > pan 0.5 > d1';
    const h = setup(text);
    h.evalResult([site(text, '0.5', 1), site(text, '0.5', 2, {}, 1)]);
    h.deps.bind?.writeSite(1, 0.25);
    expect(h.transport.of('set-tweak').map((e) => e.body.id)).toEqual([1]);
    h.area.writer.setMode(h.area.table.byId(2)?.bindingId ?? '', 'source-edit');
    h.deps.bind?.writeSite(2, 0.9);
    expect(h.text()).toBe('s [:bd] > gain 0.5 > pan 0.9 > d1');
  });

  it('an edit without re-eval then a delayed CC: doc-changed first, then STALE on edit-invalidated', () => {
    const h = setup(TEXT);
    const midi = fakeMidi();
    h.deps.midi = midi;
    h.area.router.sync();
    const d = noDirectives();
    d.bindings = [{ span: spanOf(TEXT, 'gain 0.5'), param: 'amp', cc: 21, directive: { start: 0, end: 0 } }];
    const call = { name: 'gain', head: spanOf(TEXT, 'gain'), ordinal: 1, arg: 0, param: 'amp' };
    h.evalResult([site(TEXT, '0.5', 1, { call }), site(TEXT, '0.7', 2)], { directives: d });
    h.edit('[:sd]', '[:sd :cp]'); // not re-evaluated
    h.transport.clear();
    midi.cc(21, 127, 1);
    expect(h.transport.kinds()).toEqual(['doc-changed', 'set-tweak']);
    const [changed] = h.transport.of('doc-changed');
    const [tweak] = h.transport.of('set-tweak');
    expect(tweak?.body.edit_epoch).toBe(changed?.body.edit_epoch);
    h.emit({ kind: 'stale-binding', body: { target: 1, reason: 'edit-invalidated' } });
    expect(q(h.row(1), '.bind-state')).toBe('STALE');
    h.transport.clear();
    midi.cc(21, 0, 1);
    expect(h.transport.sent).toEqual([]);
  });

  it('write-back declines on a text mismatch (directive-edit and literal)', async () => {
    const h = setup(TEXT);
    h.evalResult([site(TEXT, '0.5', 1)]);
    const e = h.area.table.byId(1);
    if (!e) throw new Error('no binding');
    // A directive-edit whose expected text no longer matches.
    h.edit('gain 0.5', 'gain 0.5 ');
    expect(
      h.area.router.applyDirectiveEdit({
        file: FILE,
        doc_revision: 1,
        span: spanOf(TEXT, 'd1'),
        expected: 'd3',
        text: 'd4',
      }),
    ).toBe(false);
    expect(h.text()).toBe('d1 s [:bd] > gain 0.5 \nd2 s [:sd] > gain 0.7');
    // A literal whose last-known text differs.
    h.area.writer.setMode(e.bindingId, 'source-edit');
    e.literalText = '0.6';
    h.transport.clear();
    h.deps.bind?.writeSite(1, 0.1);
    await settle();
    expect(h.transport.sent).toEqual([]);
    expect(h.text()).toBe('d1 s [:bd] > gain 0.5 \nd2 s [:sd] > gain 0.7');
  });
});
