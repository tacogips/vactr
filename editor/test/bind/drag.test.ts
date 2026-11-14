// Mouse drag on literals: the same write as the slider, nothing on a
// non-site number, and ParamMeta range/curve scaling.

import { afterEach, describe, expect, it } from 'vitest';
import { DRAG_PX } from '../../src/bind/drag';
import { fromUnit, toUnit } from '../../src/bind/write';
import { cleanup, EDITORS, setup, site, spanOf } from './fixtures';

afterEach(cleanup);

const tweaks = (h: ReturnType<typeof setup>) =>
  h.transport.of('set-tweak').map((e) => ({ id: e.body.id, form_gen: e.body.form_gen, value: e.body.value }));

describe('drag on literals', () => {
  it('a drag on a site literal produces the same set-tweak as the slider', () => {
    const text = 's [:bd] > gain 0.5 > d1';
    const bySlider = setup(text);
    bySlider.evalResult([site(text, '0.5', 1, { form_gen: 2 })]);
    const input = bySlider.row(1).querySelector('input.bind-slider') as HTMLInputElement;
    input.value = '0.6';
    input.dispatchEvent(new Event('input'));

    const byDrag = setup(text);
    byDrag.evalResult([site(text, '0.5', 1, { form_gen: 2 })]);
    expect(byDrag.area.drag.begin(text.indexOf('0.5') + 1, 100)).toBe(true);
    byDrag.area.drag.move(90); // 10px up: +10% of max(|0.5|, 1)
    expect(byDrag.area.drag.end()).toBe(true);

    expect(tweaks(bySlider)).toEqual([{ id: 1, form_gen: 2, value: 0.6 }]);
    expect(tweaks(byDrag)).toEqual(tweaks(bySlider));
    expect(byDrag.text()).toBe(text);
  });

  it('a drag on a number that is not a site does nothing', () => {
    const text = 's [:bd] > gain 0.5 > delay 42 > d1';
    const h = setup(text);
    h.evalResult([site(text, '0.5', 1)]);
    expect(h.area.drag.begin(text.indexOf('42') + 1, 100)).toBe(false);
    h.area.drag.move(50);
    expect(h.transport.sent).toEqual([]);
  });

  it('a click without movement writes nothing', () => {
    const text = 's [:bd] > gain 0.5 > d1';
    const h = setup(text);
    h.evalResult([site(text, '0.5', 1)]);
    expect(h.area.drag.begin(text.indexOf('0.5'), 100)).toBe(true);
    h.area.drag.move(101);
    expect(h.area.drag.end()).toBe(false);
    expect(h.transport.sent).toEqual([]);
  });

  it('scales by the ParamMeta range and curve of site.call', () => {
    const text = 's [:bd] > lpf 800 > d1';
    const h = setup(text, { editors: EDITORS });
    const call = { name: 'lpf', head: spanOf(text, 'lpf'), ordinal: 1, arg: 0, param: 'cutoff' };
    h.evalResult([site(text, '800', 1, { call })]);
    const meta = EDITORS[0]?.params[0];
    if (!meta) throw new Error('no meta');
    const pos = text.indexOf('800') + 1;
    h.area.drag.begin(pos, 300);
    h.area.drag.move(300 - DRAG_PX / 4);
    h.area.drag.move(300 + DRAG_PX * 10);
    h.area.drag.end();
    const expected = Math.round(fromUnit(toUnit(800, meta) + 0.25, meta));
    expect(tweaks(h).map((t) => t.value)).toEqual([expected, 20]);
    // Log curve: a quarter of the travel multiplies the frequency by 1000^0.25.
    expect(expected).toBe(Math.round(800 * 1000 ** 0.25));
  });

  it('without ParamMeta the step is 1% of max(|value|, 1) per pixel', () => {
    const text = 's [:bd] > gain 0.5 > pan 300 > d1';
    const h = setup(text);
    h.evalResult([site(text, '0.5', 1), site(text, '300', 2)]);
    h.area.drag.begin(text.indexOf('300') + 1, 100);
    h.area.drag.move(90);
    h.area.drag.end();
    h.area.drag.begin(text.indexOf('0.5') + 1, 100);
    h.area.drag.move(110);
    h.area.drag.end();
    expect(tweaks(h).map((t) => [t.id, t.value])).toEqual([
      [2, 330],
      [1, 0.4],
    ]);
  });

  it('in source-edit mode the drag edits the literal through the same write path', () => {
    const text = 's [:bd] > gain 0.5 > d1';
    const h = setup(text);
    h.evalResult([site(text, '0.5', 1)]);
    h.area.writer.setMode(h.area.table.byId(1)?.bindingId ?? '', 'source-edit');
    h.area.drag.begin(text.indexOf('0.5') + 1, 100);
    h.area.drag.move(80);
    h.area.drag.end();
    expect(h.text()).toBe('s [:bd] > gain 0.7 > d1');
    expect(h.transport.kinds()).toEqual(['doc-changed', 'eval']);
  });
});
