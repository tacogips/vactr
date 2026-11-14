import { ChangeSet, EditorState, Text } from '@codemirror/state';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DocumentSync } from '../../src/code/sync';
import { CodeSurface } from '../../src/code/surface';
import { Utf8Index } from '../../src/protocol/utf8';
import { LineBytes, LineTable } from '../../src/code/line-bytes';
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

  it('pins the current revision from line starts without a text read or table build', () => {
    const { sync, state } = setup(Array.from({ length: 200 }, (_, i) => `line ${i}`).join('\n'));
    const builds = LineTable.builds, indexBuilds = Utf8Index.builds;
    const tableBytes = sync.history.indexBytes;
    const toString = vi.spyOn(Text.prototype, 'toString');
    expect(sync.pin('eval', sync.revision)).toBe(true);
    expect(LineTable.builds).toBe(builds);
    expect(Utf8Index.builds).toBe(indexBuilds);
    expect(sync.history.indexBytes).toBe(tableBytes);
    expect(toString).not.toHaveBeenCalled();
    sync.unpin('eval');
    expect(state().doc.lines).toBe(200);
  });

  it('keeps LineBytes line starts equal to LineTable after 50 edits', () => {
    let text = Text.of(['a', '日本', '😀', '', 'tail']);
    const bytes = new LineBytes(text);
    for (let i = 0; i < 50; i += 1) {
      const line = text.line((i % text.lines) + 1);
      const at = line.from + Math.min(line.length, i % 3);
      const changes = ChangeSet.of({ from: at, insert: i % 2 ? 'x' : '\n' }, text.length);
      const next = changes.apply(text);
      bytes.update(changes, text, next); text = next;
      const starts = bytes.starts(), table = LineTable.build(text);
      expect(starts.length).toBe(text.lines + 1);
      for (let n = 1; n <= text.lines; n += 1) expect(starts[n - 1]).toBe(table.toByte(text.line(n).from));
      expect(starts[starts.length - 1]).toBe(table.byteLength);
    }
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

  it('matches Utf8Index for seeded Unicode edits, multi-change transactions, undo and redo', () => {
    const initialText = Text.of(['ASCII 日本 🎹', '', 'z'.repeat(100_000), 'tail']);
    const transport = new RecordingTransport();
    const client = new Client(transport, { store: new Store(), now: () => Date.now() });
    const sync = new DocumentSync(client.document('random.vact'), initialText);
    const surface = new CodeSurface({ sync });
    let latest: { changes: import('@codemirror/state').ChangeSet; doc: Text } | null = null;
    surface.subscribe((update) => { if (update.docChanged) latest = { changes: update.changes, doc: update.state.doc }; });
    let seed = 0x51a7;
    const random = (max: number): number => { seed = (1664525 * seed + 1013904223) >>> 0; return seed % max; };
    const inserts = ['', 'x', '日本', '🎹', '\n', 'é\n尾'];
    const assertLastChange = (base: Text, changes: import('@codemirror/state').ChangeSet) => {
      const edits: { from: number; to: number; insert: string }[] = [];
      changes.iterChanges((from, to, _fromB, _toB, inserted) => edits.push({ from, to, insert: inserted.toString() }));
      const expected = new Utf8Index(base.toString()).changes(edits);
      vi.advanceTimersByTime(DOC_DEBOUNCE_MS);
      const event = transport.of('doc-changed').at(-1);
      expect(event?.body.changes).toEqual(expected.changes);
      expect(event?.body.dirty).toEqual(expected.dirty);
    };

    for (let i = 0; i < 200; i += 1) {
      const base = surface.state.doc;
      const first = random(base.length + 1);
      const specs: { from: number; to: number; insert: string }[] = [{
        from: first,
        to: i % 4 === 0 ? Math.min(base.length, first + 1 + random(4)) : first,
        insert: inserts[random(inserts.length)]!,
      }];
      if (i % 9 === 0 && base.length > 4) {
        const second = Math.max(0, Math.min(base.length - 1, first > base.length / 2 ? first - 3 : first + 3));
        if (Math.abs(second - first) > 1) specs.push({ from: second, to: second, insert: inserts[random(inserts.length)]! });
      }
      specs.sort((left, right) => left.from - right.from);
      const tr = surface.state.update({ changes: specs });
      if (!tr.docChanged) continue;
      surface.dispatch(tr);
      assertLastChange(base, tr.changes);
    }

    for (const operation of ['undo', 'redo'] as const) {
      const base = surface.state.doc;
      latest = null;
      expect(surface[operation]()).toBe(true);
      expect(latest).not.toBeNull();
      assertLastChange(base, latest!.changes);
    }
    surface.dispose();
  }, 30_000);

  it('rebuilds a stale Text identity without whole-document string or oversized slice work', () => {
    const initial = Text.of(['a'.repeat(100_000)]);
    const replacement = Text.of(['b'.repeat(100_000)]);
    const transport = new RecordingTransport();
    const client = new Client(transport, { store: new Store(), now: () => Date.now() });
    const sync = new DocumentSync(client.document('reset.vact'), initial);
    const state = EditorState.create({ doc: replacement });
    const tr = state.update({ changes: { from: 50_000, to: 50_001, insert: '日本🎹' } });
    const sourceProto = Object.getPrototypeOf(replacement) as { toString: () => string; sliceString: (from: number, to?: number) => string };
    const stringify = vi.spyOn(sourceProto, 'toString');
    const slices = vi.spyOn(sourceProto, 'sliceString');
    sync.apply(tr);
    expect(stringify.mock.contexts.some((doc) => (doc as Text).length > 65_536)).toBe(false);
    expect(slices.mock.calls.every(([from, to]) => (to ?? Infinity) - from <= 65_536)).toBe(true);
    vi.advanceTimersByTime(DOC_DEBOUNCE_MS);
    expect(transport.of('doc-changed')[0]?.body.changes).toEqual([{ from: 50_000, to: 50_001, insert_len: 10 }]);
    stringify.mockRestore();
    slices.mockRestore();
  });

  it('maps an edit endpoint inside a surrogate pair to the pair start byte', () => {
    const base = 'a🎹b';
    const { transport, sync, change } = setup(base);
    expect(new Utf8Index(base).toByte(2)).toBe(1);
    change({ from: 2, insert: 'x' });
    vi.advanceTimersByTime(DOC_DEBOUNCE_MS);
    expect(transport.of('doc-changed')[0]?.body.changes).toEqual([{ from: 1, to: 1, insert_len: 1 }]);
    expect(sync.revision).toBe(2);
  });
});
