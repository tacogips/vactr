import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, EDITORS, setup, site, spanOf } from './fixtures';
import { fromUnit, toUnit } from '../../src/bind/write';

afterEach(cleanup);

describe('momentary bound-site control', () => {
  it('sends a smoothed ParamMeta target without editing the document and glides on release', () => {
    const text = 's [:bd] > lpf 800 > d1';
    const h = setup(text, { editors: EDITORS });
    const call = { name: 'lpf', head: spanOf(text, 'lpf'), ordinal: 1, arg: 0, param: 'cutoff' };
    h.evalResult([site(text, '800', 1, { call })]);
    const gesture = h.view.momentaryDrag({ pos: text.indexOf('800') + 1, clientY: 200, kind: 'mouse' });
    expect(gesture).not.toBeNull();
    gesture!.move(100);
    const target = h.transport.of('momentary').at(-1)?.body;
    const liveTarget = Number(target?.target);
    const meta = EDITORS[0]!.params[0]!;
    expect(target).toMatchObject({ target: Math.round(fromUnit(toUnit(800, meta) + 0.5, meta)), ramp_ms: 30 });
    expect(h.view.animatedRows(performance.now())[0]?.label).toBe(` ~ ${String(Number(liveTarget.toPrecision(3)))}`);
    expect(h.text()).toBe(text);
    gesture!.end(false);
    expect(h.transport.of('momentary').at(-1)?.body).toMatchObject({ target: null, ramp_ms: 1000 });
    const glide = Number(h.view.animatedRows(performance.now() + 500)[0]?.label?.slice(3));
    expect(glide).toBeGreaterThan(800); expect(glide).toBeLessThan(liveTarget);
  });

  it('uses relative scaling, integer rounding, Shift snap and ignores a sub-slop click', () => {
    const text = 's [:bd] > pan 2 > gain 4 > d1';
    const h = setup(text);
    h.evalResult([site(text, '2', 1), site(text, '4', 2)]);
    const click = h.view.momentaryDrag({ pos: text.indexOf('2'), clientY: 50, kind: 'mouse' })!;
    click.move(48); click.end(false);
    expect(h.transport.of('momentary')).toHaveLength(0);
    const gesture = h.view.momentaryDrag({ pos: text.indexOf('2'), clientY: 100, kind: 'mouse' })!;
    gesture.move(50);
    expect(h.transport.of('momentary').at(-1)?.body.target).toBe(3);
    gesture.end(true);
    expect(h.transport.of('momentary').at(-1)?.body).toMatchObject({ target: null, ramp_ms: 0 });
    expect(h.text()).toBe(text);
  });

  it('rejects non-Direct sites and rebases a held target onto the refreshed site id', () => {
    const text = 's [:bd] > gain 0.5 > d1';
    const h = setup(text);
    h.evalResult([site(text, '0.5', 1, { tier: 'reeval' })]);
    expect(h.view.momentaryDrag({ pos: text.indexOf('0.5'), clientY: 50, kind: 'mouse' })).toBeNull();
    expect(h.area.notices).toContain('momentary tweak needs a live site (re-evaluate or use a direct literal)');
    h.evalResult([site(text, '0.5', 1)]);
    const gesture = h.view.momentaryDrag({ pos: text.indexOf('0.5'), clientY: 100, kind: 'mouse' })!;
    gesture.move(50);
    h.evalResult([site(text, '0.5', 9, { form_gen: 7, value: 0.25 })]);
    expect(h.transport.of('momentary').at(-1)?.body).toMatchObject({ id: 9, form_gen: 7, target: 0.75 });
    gesture.end(false);
    expect(h.text()).toBe(text);
  });

  it('releases the last bound identity after an edit invalidates a held site', () => {
    const text = 's [:bd] > gain 0.5 > d1';
    const h = setup(text);
    h.evalResult([site(text, '0.5', 1)]);

    const releaseAfterEdit = (literal: string, next: string, id: number, snap: boolean) => {
      const current = h.text();
      h.evalResult([site(current, literal, id)]);
      const gesture = h.view.momentaryDrag({ pos: current.indexOf(literal), clientY: 100, kind: 'mouse' })!;
      gesture.move(50);
      const drag = h.transport.of('momentary').at(-1)!.body;
      const countBeforeEdit = h.transport.of('momentary').length;
      h.edit(literal, next);
      expect(h.area.table.byId(id)?.state).toBe('stale');
      gesture.move(40);
      expect(h.transport.of('momentary')).toHaveLength(countBeforeEdit);
      gesture.end(snap);
      expect(h.transport.of('momentary').at(-1)?.body).toMatchObject({
        id: drag.id, form_gen: drag.form_gen, target: null, ramp_ms: snap ? 0 : 1000,
      });
      expect(h.transport.of('momentary').filter(message => message.body.target === null)).toHaveLength(snap ? 2 : 1);
    };

    releaseAfterEdit('0.5', '0.6', 1, false);
    releaseAfterEdit('0.6', '0.7', 2, true);
    expect(h.text()).toBe('s [:bd] > gain 0.7 > d1');
  });

  it('keeps a moved gesture releasable when refresh makes its site unbound', () => {
    const text = 's [:bd] > gain 0.5 > d1';
    const h = setup(text);
    h.evalResult([site(text, '0.5', 1)]);
    const gesture = h.view.momentaryDrag({ pos: text.indexOf('0.5'), clientY: 100, kind: 'mouse' })!;
    gesture.move(50);
    const drag = h.transport.of('momentary').at(-1)!.body;

    h.evalResult([]);
    expect(h.area.table.byId(1)?.state).toBe('unbound');
    const countBeforeRelease = h.transport.of('momentary').length;
    gesture.end(false);
    expect(h.transport.of('momentary')).toHaveLength(countBeforeRelease + 1);
    expect(h.transport.of('momentary').at(-1)?.body).toMatchObject({
      id: drag.id, form_gen: drag.form_gen, target: null, ramp_ms: 1000,
    });
    expect(h.transport.of('momentary').slice(countBeforeRelease).every(message => message.body.target === null)).toBe(true);
  });

  it('rejects Manual and unbound sites with a notice and caps active gestures at 16', () => {
    const text = 's [:bd] > gain 0.5 > pan 0.2 > d1';
    const h = setup(text);
    h.evalResult([site(text, '0.5', 1, { tier: 'manual' }), site(text, '0.2', 2)]);
    expect(h.view.momentaryDrag({ pos: text.indexOf('0.5'), clientY: 10, kind: 'mouse' })).toBeNull();
    expect(h.area.notices).toHaveLength(1);
    const unbound = h.area.table.byId(2)!; unbound.state = 'unbound';
    expect(h.view.momentaryDrag({ pos: text.indexOf('0.2'), clientY: 10, kind: 'mouse' })).toBeNull();
    expect(h.area.notices).toHaveLength(2);

    const many = `s ${Array.from({ length: 17 }, () => '1').join(' ')}`;
    const manyHarness = setup(many);
    manyHarness.evalResult(Array.from({ length: 17 }, (_, i) => site(many, '1', i + 1, {}, i)));
    // Find each digit position without relying on SiteTable internals.
    const indexes: number[] = []; let at = -1; for (let i = 0; i < 17; i++) { at = many.indexOf('1', at + 1); indexes.push(at); }
    for (const pos of indexes.slice(0, 16)) expect(manyHarness.view.momentaryDrag({ pos, clientY: 10, kind: 'touch' })).not.toBeNull();
    expect(manyHarness.view.momentaryDrag({ pos: indexes[16]!, clientY: 10, kind: 'touch' })).toBeNull();
  });
});
