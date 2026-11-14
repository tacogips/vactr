// Criterion 5: the directive-backed control panel from the 13.5 examples,
// learn write-back through a verified `directive-edit`, a declined
// mismatch, the diagnostic marker, and switching to ExternalFile with the
// text byte-identical and the panel preserved.

import { afterEach, describe, expect, it } from 'vitest';
import type { Diagnostic } from '../../src/protocol/types';
import { cleanup, DIRECTIVE_DOC, directiveSites, directiveTable, fakeMidi, FILE, settle, setup, spanOf } from './fixtures';

afterEach(cleanup);

function open(diagnostics: Diagnostic[] = []) {
  const h = setup(DIRECTIVE_DOC);
  h.evalResult(directiveSites(DIRECTIVE_DOC), { directives: directiveTable(DIRECTIVE_DOC), diagnostics });
  return h;
}

const texts = (root: Element, sel: string): string[] =>
  [...root.querySelectorAll(sel)].map((e) => e.textContent ?? '');

describe('directive control panel (criterion 5)', () => {
  it('shows the declared panel grouped by label or line', () => {
    const h = open();
    const panel = h.area.control.el;
    expect([...panel.querySelectorAll('.bind-ctl-group')].map((g) => (g as HTMLElement).dataset.group)).toEqual([
      'file',
      'line 1',
      'bass-filter',
      'line 4',
      'snare',
      'line 6',
    ]);
    expect(texts(panel, '[data-group="bass-filter"] .bind-ctl-binding')).toEqual([
      'bass-filter.lpf.1.cutoff  cc 74 ch 1',
      'bass-filter.lpf.1.q  cc 71 ch 1',
    ]);
    expect(texts(panel, '[data-group="snare"] .bind-ctl-binding')).toEqual([
      'snare.hpf.1.cutoff  cc 9 ch 1',
      'snare.hpf.1.q  panel',
    ]);
    expect(texts(panel, '[data-group="snare"] .bind-ctl-text')).toEqual(['#@ snare.hpf cc: 9']);
    expect(texts(panel, '.bind-ctl-file')).toEqual(['midi ch 1']);
  });

  it('learning a mapped parameter sends learn and applies the verified directive-edit', async () => {
    const h = open();
    const midi = fakeMidi();
    h.deps.midi = midi;
    const learning = h.deps.bind?.learn(3);
    midi.cc(12, 100, 1);
    await settle();
    expect(h.transport.of('learn').map((e) => e.body)).toEqual([
      { file: FILE, binding: 'snare.hpf.1.cutoff', cc: 12, ch: 1, edit_epoch: 0 },
    ]);
    const nine = spanOf(DIRECTIVE_DOC, '9', 0);
    h.emit({
      kind: 'directive-edit',
      re: h.lastSeq('learn'),
      body: { file: FILE, doc_revision: 1, span: nine, expected: '9', text: '12' },
    });
    await learning;
    h.transport.clear();
    expect(h.text()).toBe(DIRECTIVE_DOC.replace('#@ snare.hpf cc: 9', '#@ snare.hpf cc: 12'));
    // The code sync reports the edit before any later write.
    h.deps.bind?.writeSite(3, 400);
    expect(h.transport.kinds()).toEqual(['doc-changed', 'set-tweak']);
  });

  it('declines a directive-edit whose expected text does not match', async () => {
    const h = open();
    const midi = fakeMidi();
    h.deps.midi = midi;
    const learning = h.deps.bind?.learn(3);
    midi.cc(12, 100, 1);
    await settle();
    h.emit({
      kind: 'directive-edit',
      re: h.lastSeq('learn'),
      body: { file: FILE, doc_revision: 1, span: spanOf(DIRECTIVE_DOC, '9', 0), expected: '8', text: '12' },
    });
    await learning;
    expect(h.text()).toBe(DIRECTIVE_DOC);
    expect(h.area.notices.at(-1)).toMatch(/declined/);
    expect(h.area.router.learnedOf(h.area.table.byId(3)?.bindingId ?? '')).toBeUndefined();
  });

  it('never applies a directive-edit in ExternalFile mode', () => {
    const h = open();
    h.area.setMode('external-file');
    h.emit({
      kind: 'directive-edit',
      body: { file: FILE, doc_revision: 1, span: spanOf(DIRECTIVE_DOC, '9', 0), expected: '9', text: '12' },
    });
    expect(h.text()).toBe(DIRECTIVE_DOC);
    expect(h.area.notices.at(-1)).toMatch(/external file/);
  });

  it('marks the entry of a directive diagnostic', () => {
    const span = spanOf(DIRECTIVE_DOC, '#@ ghost.lpf cc: 3');
    const h = open([
      { code: 'unknown-label', severity: 'warning', message: 'unknown label `ghost`', span, file: FILE },
    ]);
    const entry = h.area.control.el.querySelector(`[data-directive="${span.start}:${span.end}"]`) as HTMLElement;
    expect(entry.dataset.diagnostic).toBe('unknown-label');
    expect(entry.querySelector('.bind-ctl-marker')?.textContent).toBe('unknown-label');
    expect(h.area.control.el.querySelectorAll('.bind-ctl-marker')).toHaveLength(1);
  });

  it('switching to ExternalFile leaves the text byte-identical and preserves the panel', () => {
    const h = open();
    const rows = h.area.table.all().length;
    h.transport.clear();
    (h.area.control.el.querySelector('select.bind-persistence') as HTMLSelectElement).value = 'external-file';
    h.area.control.el.querySelector('select.bind-persistence')?.dispatchEvent(new Event('change'));
    expect(h.area.persistence.mode).toBe('external-file');
    expect(h.text()).toBe(DIRECTIVE_DOC);
    expect(h.transport.sent).toEqual([]);
    expect(h.area.table.all()).toHaveLength(rows);
    expect(texts(h.area.control.el, '.bind-ctl-binding')).toEqual([
      'bass-filter.lpf.1.cutoff  cc 74 ch 1',
      'bass-filter.lpf.1.q  cc 71 ch 1',
      'snare.hpf.1.cutoff  cc 9 ch 1',
      'snare.hpf.1.q  panel',
    ]);
    // The mappings keep working from the set.
    const midi = fakeMidi();
    h.deps.midi = midi;
    h.area.router.sync();
    midi.cc(9, 127, 1);
    expect(h.transport.of('set-tweak').map((e) => e.body.id)).toEqual([3]);
  });
});
