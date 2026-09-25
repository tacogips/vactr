import { EditorState, Text } from '@codemirror/state';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DocumentSync } from '../../src/code/sync';
import { Client } from '../../src/protocol/client';
import { DOC_DEBOUNCE_MS } from '../../src/protocol/document';
import { Store } from '../../src/protocol/store';
import { RecordingTransport } from '../support/recording';

const enc = new TextEncoder();
const bytes = (s: string): number => enc.encode(s).length;

function setup(initial: string) {
  const transport = new RecordingTransport();
  const client = new Client(transport, { store: new Store(), now: () => Date.now() });
  const text = Text.of(initial.split('\n'));
  const sync = new DocumentSync(client.document('main.vact'), text);
  let state = EditorState.create({ doc: text });
  const change = (spec: { from: number; to?: number; insert?: string }): void => {
    const tr = state.update({ changes: spec });
    state = tr.state;
    sync.apply(tr);
  };
  return { transport, client, sync, change, state: () => state };
}

describe('DocumentSync', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('sends doc-changed with UTF-8 byte changes and dirty spans for non-ASCII text', () => {
    const base = 'let ü 1\nd1 "日本"';
    const { transport, sync, change } = setup(base);
    // Type "é" after the Japanese text (UTF-16 offset before the closing quote).
    const at = base.lastIndexOf('"');
    change({ from: at, insert: 'é' });
    expect(transport.of('doc-changed')).toHaveLength(0);
    vi.advanceTimersByTime(DOC_DEBOUNCE_MS);
    const [dc] = transport.of('doc-changed');
    const b = bytes(base.slice(0, at));
    expect(dc?.body).toEqual({
      file: 'main.vact',
      doc_revision: 2,
      base_revision: 1,
      changes: [{ from: b, to: b, insert_len: 2 }],
      dirty: [{ start: b, end: b + 2 }],
      edit_epoch: 1,
    });
    expect(sync.revision).toBe(2);
  });

  it('gives a zero-length dirty span at a pure deletion of multi-byte text', () => {
    const base = 'd1 "日本語"';
    const { transport, change } = setup(base);
    const from = base.indexOf('本');
    change({ from, to: from + 1 });
    vi.advanceTimersByTime(DOC_DEBOUNCE_MS);
    const b = bytes(base.slice(0, from));
    expect(transport.of('doc-changed')[0]?.body.changes).toEqual([{ from: b, to: b + 3, insert_len: 0 }]);
    expect(transport.of('doc-changed')[0]?.body.dirty).toEqual([{ start: b, end: b }]);
  });

  it('increments the epoch synchronously, before the debounce fires', () => {
    const { transport, client, change } = setup('a');
    const doc = client.document('main.vact');
    expect(doc.epoch).toBe(0);
    change({ from: 1, insert: 'b' });
    expect(doc.epoch).toBe(1);
    change({ from: 2, insert: 'c' });
    expect(doc.epoch).toBe(2);
    expect(doc.revision).toBe(3);
    expect(transport.sent).toHaveLength(0);
    vi.advanceTimersByTime(DOC_DEBOUNCE_MS);
    const [dc] = transport.of('doc-changed');
    expect(dc?.body.edit_epoch).toBe(2);
    expect(dc?.body.changes).toEqual([{ from: 1, to: 1, insert_len: 2 }]);
  });

  it('flushes doc-changed before a set-tweak issued inside the debounce window', () => {
    const { transport, client, change } = setup('lpf 800');
    change({ from: 0, insert: '# ö\n' });
    vi.advanceTimersByTime(DOC_DEBOUNCE_MS / 2);
    client.setTweak('main.vact', 3, 1, 440);
    expect(transport.kinds()).toEqual(['doc-changed', 'set-tweak']);
    const [dc] = transport.of('doc-changed');
    const [tw] = transport.of('set-tweak');
    expect(dc?.body.doc_revision).toBe(2);
    expect(tw?.body.edit_epoch).toBe(dc?.body.edit_epoch);
    // The debounce does not send it a second time.
    vi.advanceTimersByTime(DOC_DEBOUNCE_MS);
    expect(transport.of('doc-changed')).toHaveLength(1);
  });

  it('records each revision in the history and maps wire spans to the current text', () => {
    const base = 'd1 "bd"';
    const { sync, change, state } = setup(base);
    const lit = { start: bytes('d1 '), end: bytes('d1 "bd"') };
    change({ from: 0, insert: 'let ä 1\n' });
    expect(sync.history.current).toBe(2);
    const r = sync.mapWireSpan(lit, 1);
    expect(state().doc.sliceString(r?.from ?? 0, r?.to ?? 0)).toBe('"bd"');
    expect(sync.toWireSpan(r?.from ?? 0, r?.to ?? 0)).toEqual({ start: bytes('let ä 1\nd1 '), end: bytes('let ä 1\nd1 "bd"') });
  });

  it('ignores transactions that do not change the document', () => {
    const { transport, sync, state } = setup('x');
    sync.apply(state().update({ selection: { anchor: 1 } }));
    vi.advanceTimersByTime(DOC_DEBOUNCE_MS);
    expect(transport.sent).toHaveLength(0);
    expect(sync.revision).toBe(1);
  });
});
