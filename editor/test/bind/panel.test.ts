// Criterion 3 (slider panel, learn): an eval-result with a pattern literal,
// a top-level `let` number and an `inst` parameter default lists all three
// under their origin groups; the inst-default slider in overlay sends
// set-tweak with no eval; learn maps a CC to it and later CCs move it.

import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fakeMidi, FILE, q, settle, setup, site, spanOf } from './fixtures';

afterEach(cleanup);

const TEXT = ['d1 s [:bd] > gain 0.8', 'let cut 1200', 'inst pad [amp 0.3] (saw 220)'].join('\n');

function sites() {
  return [
    site(TEXT, '0.8', 1, { origin: 'pattern-literal' }),
    site(TEXT, '1200', 2, { origin: 'binding' }),
    site(TEXT, '0.3', 3, { origin: 'inst-default' }),
  ];
}

describe('slider panel (criterion 3)', () => {
  it('lists the three sites under their origin groups with labels and values', () => {
    const h = setup(TEXT);
    h.evalResult(sites());
    const group = (origin: string) => h.root.querySelector(`.bind-group[data-origin="${origin}"]`) as HTMLElement;
    expect(group('pattern-literal').contains(h.row(1))).toBe(true);
    expect(group('binding').contains(h.row(2))).toBe(true);
    expect(group('inst-default').contains(h.row(3))).toBe(true);
    expect(q(h.row(2), '.bind-label')).toBe('let cut 1200');
    expect(q(h.row(3), '.bind-value')).toBe('0.3');
    expect(q(h.row(1), '.bind-mode')).toBe('overlay');
    // The panel lives in the right pane.
    expect(h.root.querySelector('[data-pane="right"] .bind-panel')).not.toBeNull();
  });

  it('the inst-default slider in overlay sends set-tweak and no eval', () => {
    const h = setup(TEXT);
    h.evalResult(sites());
    h.transport.clear();
    const input = h.row(3).querySelector('input.bind-slider') as HTMLInputElement;
    input.value = '0.45';
    input.dispatchEvent(new Event('input'));
    expect(h.transport.kinds()).toEqual(['set-tweak']);
    expect(h.transport.of('set-tweak')[0]?.body).toMatchObject({ id: 3, value: 0.45 });
    expect(h.text()).toBe(TEXT);
  });

  it('learn maps a CC to the inst-default slider and later CC events move it', async () => {
    const h = setup(TEXT);
    h.evalResult(sites());
    const midi = fakeMidi();
    h.deps.midi = midi;
    (h.row(3).querySelector('.bind-learn') as HTMLButtonElement).click();
    expect(midi.learning).toBe(true);
    midi.cc(21, 64, 1); // the learn capture
    await settle();
    const learn = h.transport.of('learn');
    expect(learn.map((e) => e.body)).toEqual([{ file: FILE, binding: 3, cc: 21, ch: 1, edit_epoch: 0 }]);
    // The session answers with the directive text edit (13.5 write-back).
    const end = spanOf(TEXT, '(saw 220)').end;
    h.emit({
      kind: 'directive-edit',
      re: h.lastSeq('learn'),
      body: { file: FILE, doc_revision: 1, span: { start: end, end }, expected: '', text: '  #@ amp cc: 21' },
    });
    await settle();
    expect(h.text()).toBe(`${TEXT}  #@ amp cc: 21`);
    expect(q(h.row(3), '.bind-midi')).toBe('cc 21 ch 1');
    h.transport.clear();
    midi.cc(21, 127, 1);
    midi.cc(21, 0, 1);
    midi.cc(21, 127, 2); // another channel: ignored
    expect(h.transport.of('set-tweak').map((e) => [e.body.id, e.body.value])).toEqual([
      [3, 1],
      [3, 0],
    ]);
    expect(h.transport.kinds()[0]).toBe('doc-changed');
  });

  it('learn without MIDI enabled reports it and sends nothing', async () => {
    const h = setup(TEXT);
    h.evalResult(sites());
    h.transport.clear();
    await h.deps.bind?.learn(3);
    expect(h.transport.sent).toEqual([]);
    expect(h.area.notices.at(-1)).toMatch(/MIDI/);
  });

  it('toggles the mode per slider and shows commit only for an active overlay', () => {
    const h = setup(TEXT);
    h.evalResult(sites());
    const commit = h.row(1).querySelector('.bind-commit') as HTMLButtonElement;
    expect(commit.hidden).toBe(true);
    h.deps.bind?.writeSite(1, 0.9);
    expect(commit.hidden).toBe(false);
    (h.row(1).querySelector('.bind-mode') as HTMLButtonElement).click();
    expect(h.deps.bind?.mode(1)).toBe('source-edit');
    expect(h.deps.bind?.mode(2)).toBe('overlay');
    expect(commit.hidden).toBe(true);
  });
});
