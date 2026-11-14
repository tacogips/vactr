// Criterion 6 (mode-scoped saving), two tests: Directive mode saves the
// buffer with its `#@` comments and no sidecar; ExternalFile mode saves the
// buffer free of editor-written binding text plus the sidecar. In both, an
// overlay reaches the text only through commit.

import { afterEach, describe, expect, it } from 'vitest';
import { EditorBindingSet } from '../../src/bind/persistence';
import { cleanup, fakeMidi, FILE, settle, setup, site } from './fixtures';

afterEach(cleanup);

const TEXT = 's [:hh] > lpf 800 > d1  #@ hats: lpf\ns [:bd] > gain 0.5 > d2';

function open() {
  const h = setup(TEXT, { docName: 'song.vact' });
  h.evalResult([site(TEXT, '800', 1, { key: 'hats.lpf.1.cutoff' }), site(TEXT, '0.5', 2)]);
  const midi = fakeMidi();
  h.deps.midi = midi;
  return { h, midi };
}

describe('mode-scoped saving (criterion 6)', () => {
  it('Directive mode: the saved .vact is the buffer with the learned #@ comment, and there is no sidecar', async () => {
    const { h, midi } = open();
    const learning = h.deps.bind?.learn(1);
    midi.cc(21, 1, 1);
    await settle();
    const at = TEXT.indexOf('#@ hats: lpf') + '#@ hats: lpf'.length;
    h.emit({
      kind: 'directive-edit',
      re: h.lastSeq('learn'),
      body: { file: FILE, doc_revision: 1, span: { start: at, end: at }, expected: '', text: ' cc: 21' },
    });
    await learning;
    // An active overlay is not in the text.
    h.deps.bind?.writeSite(2, 0.75);
    const saved = await h.area.save();
    expect(saved).toEqual({ sidecar: false });
    expect(h.files.files.get('song.vact')).toBe(h.text());
    expect(h.files.files.get('song.vact')).toContain('#@ hats: lpf cc: 21');
    expect(h.files.files.get('song.vact')).not.toContain('0.75');
    expect(h.files.files.has('song.bindings.json')).toBe(false);
    // Only commit writes the value.
    h.area.writer.commit(2);
    await h.area.save();
    expect(h.files.files.get('song.vact')).toContain('gain 0.75');
  });

  it('ExternalFile mode: the saved .vact has no editor-written binding text and the sidecar holds the bindings', async () => {
    const { h, midi } = open();
    h.area.setMode('external-file');
    const learning = h.deps.bind?.learn(1);
    midi.cc(22, 1, 3);
    await learning;
    expect(h.transport.of('learn')).toEqual([]);
    h.deps.bind?.writeSite(2, 0.75);
    await h.area.save();
    expect(h.files.files.get('song.vact')).toBe(TEXT);
    expect(h.files.files.get('song.vact')).not.toContain('0.75');
    const sidecar = h.files.files.get('song.bindings.json') ?? '';
    const back = EditorBindingSet.fromJson(sidecar);
    expect(back.notices).toEqual([]);
    expect(back.set.get('hats.lpf.1.cutoff')).toEqual({ key: 'hats.lpf.1.cutoff', panel: true, midi: { cc: 22, ch: 3 } });
    const gain = back.set.entries().find((e) => e.key === undefined);
    expect(gain).toMatchObject({ param: 'value', panel: true, overlay: 0.75 });
    // Commit is the only path into the text.
    h.area.writer.commit(2);
    await h.area.save();
    expect(h.files.files.get('song.vact')).toBe(TEXT.replace('gain 0.5', 'gain 0.75'));
  });
});
