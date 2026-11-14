import { EditorState, Text } from '@codemirror/state';
import { describe, expect, it } from 'vitest';
import { UnappliedEdits } from '../../src/code/unapplied';
import { DocumentSync } from '../../src/code/sync';
import { Client } from '../../src/protocol/client';
import { RecordingTransport } from '../support/recording';
import type { EvalResultBody } from '../../src/protocol/types';

function setup(text: string) {
  let state = EditorState.create({ doc: text });
  const edits = new UnappliedEdits(state.doc);
  const sync = new DocumentSync(new Client(new RecordingTransport()).document('main.vact'), state.doc);
  return { edits, sync, rows: () => edits.annotations(state.doc), change(from: number, to: number, insert: string) {
    const tr = state.update({ changes: { from, to, insert } }); state = tr.state; sync.apply(tr);
    edits.update({ state, changes: tr.changes, docChanged: tr.docChanged, selectionSet: false, userEvent: null }, sync.revision);
  } };
}
const result = (revision: number): EvalResultBody => ({ file: 'main.vact', doc_revision: revision, forms: [], diagnostics: [], sites: [],
  directives: { file_level: {}, entries: [] } });

describe('unapplied edit classification', () => {
  it('distinguishes inserted text, replacements and zero-width deletion markers', () => {
    const env = setup('abc def ghi');
    env.change(8, 11, ''); env.change(4, 7, 'xyz'); env.change(0, 0, '+');
    expect(env.rows()).toEqual([
      { kind: 'unapplied', editKind: 'addition', from: 0, to: 1 },
      { kind: 'unapplied', editKind: 'modification', from: 5, to: 8 },
      { kind: 'unapplied', editKind: 'deletion', from: 9, to: 9 },
    ]);
  });

  it('removes markers when text is restored to its applied baseline', () => {
    const env = setup('abc');
    env.change(1, 2, 'x'); env.change(1, 2, 'b');
    expect(env.rows()).toEqual([]);
    env.change(1, 2, ''); env.change(1, 1, 'b');
    expect(env.rows()).toEqual([]);
    env.change(1, 1, '+'); env.change(1, 2, '');
    expect(env.rows()).toEqual([]);
  });

  it('clears submitted edits on success while retaining newer edits', () => {
    const env = setup('abc');
    env.change(0, 1, 'x');
    const ids = env.edits.snapshot({ from: 0, to: 3 });
    const revision = env.sync.revision;
    env.change(3, 3, '+');
    env.edits.acknowledge(ids, result(revision), env.sync);
    expect(env.rows()).toEqual([{ kind: 'unapplied', editKind: 'addition', from: 3, to: 4 }]);
  });

  it('retains failed changes and clears a successfully submitted deletion', () => {
    const env = setup('abc'); env.change(1, 2, '');
    const ids = env.edits.snapshot({ from: 0, to: 2 });
    env.edits.acknowledge(ids, { ...result(env.sync.revision), diagnostics: [
      { severity: 'error', message: 'failed', code: 'eval', file: 'main.vact', span: { start: 0, end: 2 } },
    ] }, env.sync);
    expect(env.rows()[0]?.editKind).toBe('deletion');
    env.edits.acknowledge(ids, result(env.sync.revision), env.sync);
    expect(env.edits.annotations(Text.of(['ac']))).toEqual([]);
  });

  it('clears only the successful form from a mixed evaluation result', () => {
    const env = setup('let a 1\nlet b 2');
    env.change(6, 7, '3'); env.change(14, 15, '4');
    const ids = env.edits.snapshot({ from: 0, to: 15 });
    const failure = { severity: 'error' as const, message: 'failed', code: 'eval', file: 'main.vact',
      span: env.sync.toWireSpan(8, 15) };
    env.edits.acknowledge(ids, { ...result(env.sync.revision), forms: [
      { span: env.sync.toWireSpan(0, 7), form_gen: 1 },
      { span: env.sync.toWireSpan(8, 15), form_gen: 2, failure },
    ] }, env.sync);
    expect(env.rows()).toEqual([{ kind: 'unapplied', editKind: 'modification', from: 14, to: 15 }]);
  });
});
