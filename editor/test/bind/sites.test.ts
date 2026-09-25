// The site table: re-keying by span and by key, key migration, and the
// stale and unbound states.

import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, setup, site, spanOf } from './fixtures';

afterEach(cleanup);

describe('SiteTable', () => {
  it('re-keys a tracked binding to the fresh TweakId whose span matches', () => {
    const text = 's [:bd] > gain 0.5 > pan 0.2 > d1';
    const h = setup(text);
    h.evalResult([site(text, '0.5', 1), site(text, '0.2', 2)]);
    const a = h.area.table.byId(1)?.bindingId;
    const b = h.area.table.byId(2)?.bindingId;
    h.edit('d1', 'd2'); // does not touch either literal
    h.evalResult([site(h.text(), '0.2', 12), site(h.text(), '0.5', 11)]);
    expect(h.area.table.byId(11)?.bindingId).toBe(a);
    expect(h.area.table.byId(12)?.bindingId).toBe(b);
    expect(h.area.table.byId(11)?.state).toBe('bound');
    expect(h.area.table.byId(1)).toBeUndefined();
  });

  it('re-keys a keyed binding by key when its span was touched', () => {
    const text = 's [:hh] > lpf 800 > d1  #@ hats: lpf';
    const h = setup(text);
    h.evalResult([site(text, '800', 1, { key: 'hats.lpf.1.cutoff' })]);
    const id = h.area.table.byId(1)?.bindingId;
    h.edit('800', '900');
    expect(h.area.table.byId(1)?.state).toBe('stale');
    h.evalResult([site(h.text(), '900', 5, { key: 'hats.lpf.1.cutoff' })]);
    const e = h.area.table.byId(5);
    expect(e?.bindingId).toBe(id);
    expect(e?.state).toBe('bound');
    expect(e?.literalText).toBe('900');
  });

  it('migrates a keyed binding whose span lands on a same-family key with a new ordinal', () => {
    const text = 's [:hh] > lpf 800 > d1';
    const h = setup(text);
    h.evalResult([site(text, '800', 1, { key: 'hats.lpf.1.cutoff' })]);
    const id = h.area.table.byId(1)?.bindingId;
    h.edit('s [:hh] > ', 's [:hh] > lpf 100 > ');
    const next = h.text();
    h.evalResult([
      site(next, '100', 2, { key: 'hats.lpf.1.cutoff' }),
      site(next, '800', 3, { key: 'hats.lpf.2.cutoff' }),
    ]);
    expect(h.area.table.byId(3)?.bindingId).toBe(id);
    expect(h.area.table.byId(3)?.key).toBe('hats.lpf.2.cutoff');
    expect(h.area.table.byId(2)?.bindingId).not.toBe(id);
  });

  it('a touched mapping is STALE; no match is unbound; a second miss drops the binding', () => {
    const text = 's [:bd] > gain 0.5 > pan 0.2 > d1';
    const h = setup(text);
    h.evalResult([site(text, '0.5', 1), site(text, '0.2', 2)]);
    const stale = h.area.table.byId(1)?.bindingId ?? '';
    const unbound = h.area.table.byId(2)?.bindingId ?? '';
    h.edit('0.5', '0.6');
    h.evalResult([]);
    expect(h.area.table.get(stale)?.state).toBe('stale');
    expect(h.area.table.get(unbound)?.state).toBe('unbound');
    expect(h.area.panel.row(unbound)?.dataset.state).toBe('unbound');
    h.evalResult([]);
    expect(h.area.table.get(stale)).toBeUndefined();
    expect(h.area.table.get(unbound)).toBeUndefined();
    expect(h.area.panel.row(unbound)).toBeUndefined();
  });

  it('duplicate literals are distinct bindings', () => {
    const text = 's [:bd] > gain 0.5 > pan 0.5 > d1';
    const h = setup(text);
    h.evalResult([site(text, '0.5', 1), site(text, '0.5', 2, {}, 1)]);
    const a = h.area.table.byId(1);
    const b = h.area.table.byId(2);
    expect(a?.bindingId).not.toBe(b?.bindingId);
    expect(a?.anchor.span).toEqual(spanOf(text, '0.5'));
    expect(b?.anchor.span).toEqual(spanOf(text, '0.5', 1));
  });

  it('keeps the known key when a bindings batch republishes the site without it', () => {
    const text = 's [:hh] > lpf 800 > d1';
    const h = setup(text);
    h.evalResult([site(text, '800', 1, { key: 'hats.lpf.1.cutoff' })]);
    h.emit({ kind: 'bindings', body: { pass: 1, changed: [], sites: [site(text, '800', 9, { form_gen: 2 })], states: [] } });
    expect(h.area.table.byId(9)?.key).toBe('hats.lpf.1.cutoff');
  });
});
