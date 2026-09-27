// Criterion 8: every 14.5.5 batch shape repaints the affected value
// displays and slider rows exactly once, leaves unrelated rows alone, and
// never renders a provisional value.

import { afterEach, describe, expect, it } from 'vitest';
import type { BindingsBody, WireDirectives, WireLabel } from '../../src/protocol/types';
import { BATCH_SHAPES, cleanup, noDirectives, q, setup, site, type BatchShape } from './fixtures';

afterEach(cleanup);

const enc = new TextEncoder();

function namesOf(shape: BatchShape): string[] {
  const all = new Set<string>();
  for (const b of [...shape.seed, shape.batch]) {
    for (const c of b.changed) all.add(c.name);
    for (const s of b.states) all.add(s.name);
  }
  return [...all, 'unrelated'];
}

const affectedBy = (b: BindingsBody): Set<string> =>
  new Set([...b.changed.map((c) => c.name), ...b.states.map((s) => s.name)]);

/** One `let <name> 0` line per name; each literal is a site owned by its name. */
function rig(names: string[]) {
  const text = names.map((n) => `let ${n} 0`).join('\n');
  const h = setup(text);
  const labels: WireLabel[] = [];
  let at = 0;
  for (const n of names) {
    const start = enc.encode(text.slice(0, at)).length + 4;
    labels.push({ name: n, spans: [{ start, end: start + n.length }], ambiguous: false });
    at += `let ${n} 0`.length + 1;
  }
  const directives: WireDirectives = { ...noDirectives(), labels };
  h.evalResult(
    names.map((n, i) => site(text, `${n} 0`, i + 1, { origin: 'binding' })).map((s) => ({
      ...s,
      span: { start: s.span.end - 1, end: s.span.end },
      value: 0,
    })),
    { directives },
  );
  const bindingOf = (i: number): string => h.area.table.byId(i + 1)?.bindingId ?? '';
  return { h, bindingOf };
}

describe('reactive displays (criterion 8)', () => {
  it('a one-site bindings batch preserves neighboring Solid row and value nodes', () => {
    const { h, bindingOf } = rig(['x', 'y']);
    h.emit({ kind: 'bindings', body: {
      pass: 0,
      changed: [{ name: 'x', value: '0', form_gen: 1 }, { name: 'y', value: '0', form_gen: 1 }],
      sites: [], states: [],
    } });
    const panel = h.area.panel;
    const xId = bindingOf(0);
    const yId = bindingOf(1);
    const xRow = panel.row(xId)!;
    const yRow = panel.row(yId)!;
    const xName = panel.nameRow('x')!;
    const yName = panel.nameRow('y')!;
    const xValue = xRow.querySelector('.bind-value');
    const yValue = yRow.querySelector('.bind-value');
    const xDisplay = xName.querySelector('.bind-name-value');
    const yDisplay = yName.querySelector('.bind-name-value');
    const xCount = panel.renderCount(`binding:${xId}`);
    const yCount = panel.renderCount(`binding:${yId}`);
    const xNameCount = panel.renderCount('name:x');
    const yNameCount = panel.renderCount('name:y');

    h.emit({ kind: 'bindings', body: {
      pass: 1,
      changed: [{ name: 'x', value: '3', form_gen: 2 }],
      sites: [{ ...site(h.text(), '0', 11, { origin: 'binding', form_gen: 2, value: 3 }), value: 3 }],
      states: [{ name: 'x', state: 'ok', value: '3' }],
    } });

    expect(panel.renderCount(`binding:${xId}`) - xCount).toBe(1);
    expect(panel.renderCount(`binding:${yId}`) - yCount).toBe(0);
    expect(panel.renderCount('name:x') - xNameCount).toBe(1);
    expect(panel.renderCount('name:y') - yNameCount).toBe(0);
    expect(panel.row(xId)).toBe(xRow);
    expect(panel.row(yId)).toBe(yRow);
    expect(xRow.querySelector('.bind-value')).toBe(xValue);
    expect(yRow.querySelector('.bind-value')).toBe(yValue);
    expect(panel.nameRow('x')).toBe(xName);
    expect(panel.nameRow('y')).toBe(yName);
    expect(xName.querySelector('.bind-name-value')).toBe(xDisplay);
    expect(yName.querySelector('.bind-name-value')).toBe(yDisplay);
    expect(q(xRow, '.bind-value')).toBe('3');
    expect(q(yRow, '.bind-value')).toBe('0');
    expect(q(xName, '.bind-name-value')).toBe('3');
  });

  for (const shape of BATCH_SHAPES) {
    it(`${shape.name}: one repaint per affected row, none elsewhere, no provisional value`, () => {
      const names = namesOf(shape);
      const { h, bindingOf } = rig(names);
      h.emit({ kind: 'bindings', body: { pass: 0, changed: [{ name: 'unrelated', value: '9', form_gen: 1 }], sites: [], states: [] } });
      for (const s of shape.seed) h.emit({ kind: 'bindings', body: s });
      const panel = h.area.panel;
      const before = new Map<string, number>();
      names.forEach((n, i) => {
        before.set(`name:${n}`, panel.renderCount(`name:${n}`));
        before.set(`binding:${bindingOf(i)}`, panel.renderCount(`binding:${bindingOf(i)}`));
      });
      const control = h.area.control.renderCount;

      h.emit({ kind: 'bindings', body: shape.batch });

      const affected = affectedBy(shape.batch);
      names.forEach((n, i) => {
        const want = affected.has(n) ? 1 : 0;
        expect(panel.renderCount(`name:${n}`) - (before.get(`name:${n}`) ?? 0), `name ${n}`).toBe(want);
        const key = `binding:${bindingOf(i)}`;
        expect(panel.renderCount(key) - (before.get(key) ?? 0), `slider of ${n}`).toBe(want);
      });
      expect(h.area.control.renderCount).toBe(control);
      for (const [n, [value, badge]] of Object.entries(shape.expect)) {
        const row = panel.nameRow(n);
        if (!row) throw new Error(`no row ${n}`);
        expect(q(row, '.bind-name-value'), `${n} value`).toBe(value);
        if (badge === '') expect(q(row, '.bind-name-badge'), `${n} badge`).toBe('');
        else expect(q(row, '.bind-name-badge'), `${n} badge`).toContain(badge);
        const i = names.indexOf(n);
        if (badge === '') expect(q(h.row(i + 1), '.bind-form')).toBe('');
        else expect(q(h.row(i + 1), '.bind-form')).toContain(badge);
      }
      for (const [n, bad] of Object.entries(shape.never ?? {})) {
        const shown = h.renders.filter(([k]) => k === `name:${n}`).map(([, v]) => v);
        expect(shown.length).toBeGreaterThan(0);
        for (const v of bad) expect(shown, `${n} never shows ${v}`).not.toContain(v);
      }
    });
  }

  it('a batch that re-keys a site and changes its owner repaints that slider once and nothing else', () => {
    const { h, bindingOf } = rig(['x', 'y']);
    const panel = h.area.panel;
    const bx = bindingOf(0);
    const by = bindingOf(1);
    const nx = panel.renderCount(`binding:${bx}`);
    const ny = panel.renderCount(`binding:${by}`);
    const text = h.text();
    const fresh = { ...site(text, '0', 11, { origin: 'binding', form_gen: 2, value: 3 }), value: 3 };
    h.emit({
      kind: 'bindings',
      body: { pass: 1, changed: [{ name: 'x', value: '3', form_gen: 2 }], sites: [fresh], states: [{ name: 'x', state: 'ok', value: '3' }] },
    });
    expect(h.area.table.byId(11)?.bindingId).toBe(bx);
    expect(panel.renderCount(`binding:${bx}`) - nx).toBe(1);
    expect(panel.renderCount(`binding:${by}`) - ny).toBe(0);
    expect(q(h.row(11), '.bind-value')).toBe('3');
  });
});
