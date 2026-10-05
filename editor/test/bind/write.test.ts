// Criterion 2: the one site-write path in both modes, tiers, re-keying,
// the in-flight eval rule, and declines on text drift.

import { afterEach, describe, expect, it } from 'vitest';
import { formatLiteral } from '../../src/bind/write';
import { cleanup, FILE, q, settle, setup, site, spanOf } from './fixtures';

afterEach(cleanup);

const TEXT = 's [:bd] > gain 0.5 > d1';

function slide(row: HTMLElement, value: number): void {
  const input = row.querySelector('input.bind-slider') as HTMLInputElement;
  input.value = String(value);
  input.dispatchEvent(new Event('input'));
}

describe('overlay mode (criterion 2)', () => {
  it('sends exactly one set-tweak with form_gen and the current epoch, flushing doc-changed first; the text is unchanged', () => {
    const h = setup(TEXT);
    h.evalResult([site(TEXT, '0.5', 1, { form_gen: 3 })]);
    h.edit('d1', 'd1 '); // an edit elsewhere: epoch 1, doc-changed pending
    const before = h.text();
    h.transport.clear();
    h.deps.bind?.writeSite(1, 0.75);
    expect(h.transport.kinds()).toEqual(['doc-changed', 'set-tweak']);
    expect(h.transport.of('set-tweak')[0]?.body).toEqual({ file: FILE, id: 1, form_gen: 3, value: 0.75, edit_epoch: 1 });
    expect(h.text()).toBe(before);
    expect(h.view.annotationRanges().filter((r) => r.kind === 'binding').map((r) => ({ pos: r.from, text: r.label }))).toEqual([{ pos: TEXT.indexOf('0.5') + 3, text: ' = 0.75' }]);
    expect(q(h.row(1), '.bind-value')).toBe('0.75');
  });

  it('commit makes a verified edit, then one eval of the owning form', () => {
    const h = setup(TEXT);
    h.evalResult([site(TEXT, '0.5', 1)]);
    h.deps.bind?.writeSite(1, 0.75);
    h.transport.clear();
    h.area.writer.commit(1);
    const next = 's [:bd] > gain 0.75 > d1';
    expect(h.text()).toBe(next);
    expect(h.transport.kinds()).toEqual(['doc-changed', 'eval']);
    const ev = h.transport.of('eval')[0]?.body;
    expect(ev?.span).toEqual(spanOf(next, next));
    expect(ev?.code).toBe(next);
    expect(h.view.annotationRanges().filter((r) => r.kind === 'binding')).toEqual([]);
  });

  it('a reeval site shows "next cycle"; a bindings batch re-keys it and the next move targets the new id', () => {
    const h = setup(TEXT);
    h.evalResult([site(TEXT, '0.5', 1, { tier: 'reeval' })]);
    expect(q(h.row(1), '.bind-tier')).toBe('next cycle');
    h.emit({
      kind: 'bindings',
      body: { pass: 1, changed: [], sites: [site(TEXT, '0.5', 7, { tier: 'reeval', form_gen: 2 })], states: [] },
    });
    h.transport.clear();
    slide(h.row(7), 0.6);
    expect(h.transport.of('set-tweak').map((e) => [e.body.id, e.body.form_gen, e.body.value])).toEqual([[7, 2, 0.6]]);
  });

  it('a manual site shows the re-evaluate badge', () => {
    const h = setup(TEXT);
    h.evalResult([site(TEXT, '0.5', 1, { tier: 'manual' })]);
    expect(q(h.row(1), '.bind-tier')).toBe('re-evaluate to hear');
  });
});

describe('source-edit mode (criterion 2)', () => {
  it('edits the text and evals the form, one eval in flight, the latest value applied after the reply', async () => {
    const h = setup(TEXT);
    h.evalResult([site(TEXT, '0.5', 1)]);
    (h.row(1).querySelector('.bind-mode') as HTMLButtonElement).click();
    expect(h.deps.bind?.mode(1)).toBe('source-edit');
    h.transport.clear();
    h.deps.bind?.writeSite(1, 0.6);
    expect(h.text()).toBe('s [:bd] > gain 0.6 > d1');
    expect(h.transport.kinds()).toEqual(['doc-changed', 'eval']);
    h.deps.bind?.writeSite(1, 0.7);
    h.deps.bind?.writeSite(1, 0.8);
    expect(h.transport.kinds()).toEqual(['doc-changed', 'eval']);
    expect(h.text()).toBe('s [:bd] > gain 0.6 > d1');
    // The reply: fresh sites at the edited revision.
    h.evalResult([site(h.text(), '0.6', 2, { form_gen: 2 })], { re: h.lastSeq('eval') });
    await settle();
    expect(h.text()).toBe('s [:bd] > gain 0.8 > d1');
    expect(h.transport.kinds()).toEqual(['doc-changed', 'eval', 'doc-changed', 'eval']);
    expect(h.transport.of('set-tweak')).toEqual([]);
  });

  it('formats integers as integers and floats with at most 6 decimals', () => {
    expect(formatLiteral(801.6, '800')).toBe('802');
    expect(formatLiteral(0.1234567, '0.5')).toBe('0.123457');
    expect(formatLiteral(2, '0.5')).toBe('2.0');
    expect(formatLiteral(-0.0000001, '0.5')).toBe('0.0');
    expect(formatLiteral(3.4, '0.5', { name: 'hits', range: [0, 16], curve: 'stepped', unit: 'none', group: 0 })).toBe('3');
  });

  it('declines on text drift at the span: no edit and no message', () => {
    const h = setup(TEXT);
    h.evalResult([site(TEXT, '0.5', 1)]);
    h.area.writer.setMode(h.area.table.byId(1)?.bindingId ?? '', 'source-edit');
    // The user edits the literal itself: the binding is STALE.
    h.edit('0.5', '0.55');
    expect(h.row(1).dataset.state).toBe('stale');
    h.transport.clear();
    h.deps.bind?.writeSite(1, 0.9);
    expect(h.transport.sent).toEqual([]);
    expect(h.text()).toBe('s [:bd] > gain 0.55 > d1');
    expect(h.area.notices.at(-1)).toMatch(/stale/);
  });

  it('declines when the text at a cleanly mapped span no longer spells the literal', () => {
    const h = setup(TEXT);
    h.evalResult([site(TEXT, '0.5', 1)]);
    const e = h.area.table.byId(1);
    if (!e) throw new Error('no binding');
    h.area.writer.setMode(e.bindingId, 'source-edit');
    e.literalText = '0.4'; // the last-known literal differs from the text
    h.transport.clear();
    h.deps.bind?.writeSite(1, 0.9);
    expect(h.transport.sent).toEqual([]);
    expect(h.text()).toBe(TEXT);
    expect(h.area.notices.at(-1)).toMatch(/declined/);
  });
});

describe('stale-binding replies', () => {
  it('stale-form-gen keeps the value and re-sends it to the re-keyed site', () => {
    const h = setup(TEXT);
    h.evalResult([site(TEXT, '0.5', 1)]);
    h.deps.bind?.writeSite(1, 0.9);
    h.emit({ kind: 'stale-binding', body: { target: 1, reason: 'stale-form-gen', current_form_gen: 2 } });
    h.transport.clear();
    h.emit({ kind: 'bindings', body: { pass: 2, changed: [], sites: [site(TEXT, '0.5', 4, { form_gen: 2 })], states: [] } });
    expect(h.transport.of('set-tweak').map((e) => [e.body.id, e.body.form_gen, e.body.value])).toEqual([[4, 2, 0.9]]);
  });

  it('edit-invalidated marks STALE until the next eval-result; superseded-definition drops the pending write', () => {
    const h = setup(TEXT);
    h.evalResult([site(TEXT, '0.5', 1)]);
    h.deps.bind?.writeSite(1, 0.9);
    h.emit({ kind: 'stale-binding', body: { target: 1, reason: 'edit-invalidated' } });
    expect(q(h.row(1), '.bind-state')).toBe('STALE');
    h.transport.clear();
    h.deps.bind?.writeSite(1, 0.8);
    expect(h.transport.sent).toEqual([]);
    h.evalResult([site(TEXT, '0.5', 2)]);
    expect(q(h.row(2), '.bind-state')).toBe('');
    h.deps.bind?.writeSite(2, 0.3);
    h.emit({ kind: 'stale-binding', body: { target: 2, reason: 'stale-form-gen' } });
    h.emit({ kind: 'stale-binding', body: { target: 2, reason: 'superseded-definition' } });
    h.transport.clear();
    h.evalResult([site(TEXT, '0.5', 3)]);
    expect(h.transport.of('set-tweak')).toEqual([]);
  });
});
