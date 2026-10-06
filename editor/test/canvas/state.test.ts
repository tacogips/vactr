import { EditorState, StateEffect, StateField, Text, Transaction } from '@codemirror/state';
import { isolateHistory, invertedEffects, undo } from '@codemirror/commands';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { RevisionHistory, HISTORY_LIMIT, HISTORY_UNDO_BYTES, INDEX_BYTES } from '../../src/code/history';
import { tokenizerSpans } from '../../src/code/language';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { RecordingTransport } from '../support/recording';

function setup(source: string, historyByteLimit?: number, extensions: import('@codemirror/state').Extension = []) {
  const transport = new RecordingTransport();
  const client = new Client(transport, { store: new Store(), now: () => Date.now() });
  const sync = new DocumentSync(client.document('main.vact'), Text.of(source.split('\n')));
  const surface = new CodeSurface({ sync, historyByteLimit, extensions });
  return { surface, sync, client, transport };
}
const isolated = isolateHistory.of('full');

describe('headless canvas state authority', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('records a multi-change transaction once before subscribers flush a protocol write', () => {
    const { surface, sync, client, transport } = setup('日本😀 abc');
    const old = surface.state;
    const stop = surface.subscribe((u) => {
      if (!u.docChanged) return;
      expect(sync.revision).toBe(2);
      expect(sync.history.current).toBe(2);
      expect(sync.doc.epoch).toBe(1);
      client.setTweak('main.vact', 3, 1, 440);
    });
    surface.dispatch({ changes: [{ from: 0, to: 1, insert: '語' }, { from: 5, to: 8, insert: 'é' }] });
    expect(old.doc.toString()).toBe('日本😀 abc');
    expect(surface.state.doc.toString()).toBe('語本😀 é');
    expect(transport.kinds()).toEqual(['doc-changed', 'set-tweak']);
    expect(transport.of('doc-changed')[0]?.body.changes).toEqual([{ from: 0, to: 3, insert_len: 3 }, { from: 11, to: 14, insert_len: 2 }]);
    vi.advanceTimersByTime(200);
    expect(transport.of('doc-changed')).toHaveLength(1);
    stop(); surface.dispose();
  });

  it('selection and immutable owner annotations never revise source; observer cleanup works', () => {
    const { surface, sync } = setup('abc');
    const cb = vi.fn();
    const stop = sync.bind(surface, cb);
    surface.dispatch({ selection: { anchor: 2 } });
    const input = [{ from: 0, to: 1, kind: 'playing' as const }];
    surface.annotate('audio', input);
    input[0]!.to = 3;
    expect(surface.annotationRanges()).toEqual([{ from: 0, to: 1, kind: 'playing' }]);
    expect(sync.revision).toBe(1);
    expect(cb).not.toHaveBeenCalled();
    surface.dispatch({ changes: { from: 3, insert: 'd' } });
    expect(cb).toHaveBeenCalledExactlyOnceWith(2);
    stop(); surface.dispatch({ changes: { from: 4, insert: 'e' } });
    expect(cb).toHaveBeenCalledTimes(1);
    surface.dispose(); expect(() => surface.dispatch({ selection: { anchor: 0 } })).toThrow('disposed');
  });

  it('round trips Japanese, emoji, tabs and CRLF normalized CodeMirror documents', () => {
    const { surface, sync } = setup('日本😀\t\r\n語');
    const text = surface.state.doc.toString();
    for (const pos of [0, 1, 2, 4, 5, 6, text.length]) {
      const span = sync.toWireSpan(pos, pos);
      expect(sync.mapWireSpan(span, 1)).toEqual({ from: pos, to: pos });
    }
    surface.dispose();
  });

  it('clipboard replacement, undo and redo all pass through the same revision pipeline', () => {
    const { surface, sync } = setup('日本😀 abc');
    surface.dispatch({ changes: { from: 0, to: 4, insert: 'paste' }, selection: { anchor: 5 }, userEvent: 'input.paste' });
    expect(surface.state.doc.toString()).toBe('paste abc');
    expect(surface.undo()).toBe(true);
    expect(surface.state.doc.toString()).toBe('日本😀 abc');
    expect(surface.redo()).toBe(true);
    expect(surface.state.doc.toString()).toBe('paste abc');
    expect(sync.revision).toBe(4);
    expect(sync.doc.epoch).toBe(3);
    surface.dispose();
  });

  it('keeps adjacent typing in one undo group and excludes feedback', () => {
    const { surface } = setup('');
    surface.dispatch({ changes: { from: 0, insert: 'a' }, userEvent: 'input.type' });
    vi.advanceTimersByTime(10);
    surface.annotate('cursor', []);
    surface.dispatch({ changes: { from: 1, insert: 'b' }, userEvent: 'input.type' });
    expect(surface.retentionStatus.undoDepth).toBe(1);
    surface.undo(); expect(surface.state.doc.toString()).toBe('');
    surface.redo(); expect(surface.state.doc.toString()).toBe('ab');
    surface.dispose();
  });

  it('rejects replayed transactions and maps deletion boundaries while dropping touched spans', () => {
    const { surface, sync } = setup('abcdef');
    const tr = surface.state.update({ changes: { from: 0, to: 2 } });
    surface.dispatch(tr);
    expect(() => surface.dispatch(tr)).toThrow('Stale');
    expect(sync.mapWireSpan({ start: 2, end: 4 }, 1)).toEqual({ from: 0, to: 2 });
    expect(sync.mapWireSpan({ start: 1, end: 3 }, 1)).toBeNull();
    expect(sync.history.mapSpan({ from: -1, to: 2 }, 1)).toBeNull();
    surface.dispose();
  });

  it('bounds combined history/undo and evicts complete groups without mutating earlier snapshots', () => {
    const { surface, sync } = setup('x'.repeat(100), 2200);
    for (let i = 0; i < 8; i++) surface.dispatch({ changes: { from: surface.state.doc.length, insert: 'a'.repeat(100) }, annotations: isolated });
    const snapshot = surface.state;
    const before = snapshot.doc.toString();
    expect(surface.retentionStatus.reducedDepth).toBe(true);
    expect(surface.retentionStatus.historyBytes + surface.retentionStatus.undoBytes).toBeLessThanOrEqual(2200);
    expect(sync.mapWireSpan({ start: 0, end: 1 }, 1)).toBeNull();
    let groups = 0;
    while (surface.undo()) { groups++; expect(surface.state.doc.length % 100).toBe(0); }
    expect(groups).toBeGreaterThan(0);
    expect(groups).toBeLessThan(8);
    expect(snapshot.doc.toString()).toBe(before);
    expect(surface.retentionStatus.historyBytes + surface.retentionStatus.undoBytes).toBeLessThanOrEqual(2200);
    expect(surface.redo()).toBe(true);
    surface.dispose();
  });

  it('preserves non-history state fields and inverted effects when trimming', () => {
    const effect = StateEffect.define<number>();
    const field = StateField.define({ create: () => 0, update: (v, tr) => { for (const e of tr.effects) if (e.is(effect)) v = e.value; return v; } });
    const extension = [field, invertedEffects.of((tr) => tr.effects.some((e) => e.is(effect)) ? [effect.of(tr.startState.field(field))] : [])];
    const { surface } = setup('', 3500, extension);
    for (let i = 1; i <= 8; i++) surface.dispatch({ changes: { from: surface.state.doc.length, insert: 'x'.repeat(100) }, effects: effect.of(i), annotations: isolated });
    expect(surface.retentionStatus.reducedDepth).toBe(true);
    expect(surface.state.field(field)).toBe(8);
    expect(surface.undo()).toBe(true);
    expect(surface.state.field(field)).toBe(7);
    expect(surface.redo()).toBe(true);
    expect(surface.state.field(field)).toBe(8);
    surface.dispose();
  });

  it('maps undo through a source edit excluded from undo history', () => {
    const { surface } = setup('abc');
    surface.dispatch({ changes: { from: 3, insert: 'X' }, annotations: isolated });
    surface.dispatch({ changes: { from: 0, insert: 'Y' }, annotations: Transaction.addToHistory.of(false) });
    surface.undo(); expect(surface.state.doc.toString()).toBe('Yabc');
    surface.redo(); expect(surface.state.doc.toString()).toBe('YabcX');
    surface.dispose();
  });

  it('keeps grouping metadata and previous snapshot history after eviction', () => {
    const { surface } = setup('', 2200);
    for (let i = 0; i < 8; i++) surface.dispatch({ changes: { from: surface.state.doc.length, insert: 'x'.repeat(100) }, userEvent: 'input.paste' });
    surface.dispatch({ changes: { from: surface.state.doc.length, insert: 'a' }, userEvent: 'input.type' });
    const snapshot = surface.state;
    const depth = surface.retentionStatus.undoDepth;
    vi.advanceTimersByTime(10);
    surface.dispatch({ changes: { from: surface.state.doc.length, insert: 'b' }, userEvent: 'input.type' });
    expect(surface.retentionStatus.undoDepth).toBe(depth);
    expect(snapshot.doc.toString().endsWith('a')).toBe(true);
    let previousUndo: Transaction | undefined;
    // Headless public commands still work with the retained immutable snapshot.
    undo({ state: snapshot, dispatch: (tr) => { previousUndo = tr; } });
    expect(previousUndo?.newDoc.toString().endsWith('a')).toBe(false);
    surface.undo(); expect(surface.state.doc.toString().endsWith('ab')).toBe(false);
    surface.dispose();
  });

  it('internal history eviction cannot be rewritten by user transaction filters', () => {
    const filter = EditorState.transactionFilter.of((tr) => tr.reconfigured ? { changes: { from: 0, insert: 'BAD' } } : tr);
    const { surface, sync } = setup('', 100, filter);
    surface.dispatch({ changes: { from: 0, insert: 'source' } });
    expect(surface.state.doc.toString()).toBe('source');
    expect(sync.history.text(sync.revision)?.toString()).toBe('source');
    expect(sync.revision).toBe(2);
    surface.dispose();
  });

  it('debounces at 200ms and maps or invalidates annotations on edits', () => {
    const { surface, transport } = setup('abc');
    surface.annotate('playing', [{ from: 1, to: 2, kind: 'playing' }]);
    surface.dispatch({ changes: { from: 0, insert: 'x' } });
    expect(surface.annotationRanges()).toEqual([{ from: 2, to: 3, kind: 'playing' }]);
    vi.advanceTimersByTime(199); expect(transport.sent).toHaveLength(0);
    vi.advanceTimersByTime(1); expect(transport.of('doc-changed')).toHaveLength(1);
    surface.dispatch({ changes: { from: 2, to: 3, insert: 'z' } });
    expect(surface.annotationRanges()).toEqual([]);
    surface.dispose();
  });

  it('keeps the current document editable when a single group exceeds the ceiling', () => {
    const { surface } = setup('a', 100);
    surface.dispatch({ changes: { from: 0, to: 1, insert: 'x'.repeat(1000) } });
    expect(surface.state.doc.length).toBe(1000);
    expect(surface.retentionStatus.undoBytes).toBe(0);
    expect(surface.undo()).toBe(false);
    surface.dispatch({ changes: { from: 1000, insert: 'y' } });
    expect(surface.state.doc.length).toBe(1001);
    surface.dispose();
  });

  it('defers overlapping composition writes and cleans pointer subscriptions and pending timers', () => {
    const { surface, transport } = setup('abc');
    const write = vi.fn(); const pointer = vi.fn(); const stop = surface.onPointer(pointer);
    surface.setCompositionRange({ from: 1, to: 2 });
    surface.deferSourceWrite({ from: 1, to: 2 }, write);
    expect(write).not.toHaveBeenCalled();
    surface.setCompositionRange(null); expect(write).toHaveBeenCalledTimes(1);
    surface.notifyPointer({} as PointerEvent); stop(); surface.notifyPointer({} as PointerEvent);
    expect(pointer).toHaveBeenCalledTimes(1);
    surface.dispatch({ changes: { from: 3, insert: 'd' } });
    surface.dispose(); vi.advanceTimersByTime(200);
    expect(transport.sent).toHaveLength(0);
  });
});

describe('revision and line-table ceilings', () => {
  it('enforces 256 revisions and four cached line tables including stale rejection', () => {
    let state = EditorState.create({ doc: 'abc' });
    const h = new RevisionHistory(state.doc, 1, 999);
    for (let rev = 2; rev <= 300; rev++) {
      const tr = state.update({ changes: { from: state.doc.length, insert: 'x' } }); state = tr.state;
      h.record(rev, tr.changes, state.doc);
    }
    expect(h.current - h.oldest + 1).toBe(HISTORY_LIMIT);
    expect(h.mapWireSpan({ start: 0, end: 1 }, 1)).toBeNull();
    for (let rev = 295; rev <= 300; rev++) h.lineTable(rev);
    expect(h.indexCount).toBe(4);
    expect(h.retainedBytes).toBeLessThanOrEqual(HISTORY_UNDO_BYTES);
    expect(h.indexBytes).toBeLessThanOrEqual(INDEX_BYTES);
  });

  it('evicts byte-heavy old revisions and refuses to cache oversized line tables', () => {
    let state = EditorState.create({ doc: '日本😀'.repeat(20) });
    const h = new RevisionHistory(state.doc, 1, 256, 800, 128);
    for (let rev = 2; rev <= 5; rev++) {
      const tr = state.update({ changes: { from: state.doc.length, insert: '語' } }); state = tr.state;
      h.record(rev, tr.changes, state.doc); h.lineTable(rev);
    }
    expect(h.retainedBytes).toBeLessThanOrEqual(800);
    expect(h.indexBytes).toBeLessThanOrEqual(400);
    expect(h.indexCount).toBe(0);
    expect(h.text(5)?.toString()).toBe(state.doc.toString());
    expect(h.lineTable(5)?.toByte(4)).toBe(10);
    expect(h.mapWireSpan({ start: 0, end: 3 }, 1)).toBeNull();
  });

  it('enforces the real 8MiB index byte cap before the four-entry count cap', () => {
    let state = EditorState.create({ doc: Text.of(Array(1_048_576).fill('')) });
    const h = new RevisionHistory(state.doc);
    for (let rev = 2; rev <= 4; rev++) {
      h.lineTable(rev - 1);
      const tr = state.update({ changes: { from: state.doc.length, insert: 'x' } }); state = tr.state;
      h.record(rev, tr.changes, state.doc);
    }
    h.lineTable(4);
    expect(h.indexCount).toBe(1);
    expect(h.indexBytes).toBeLessThanOrEqual(INDEX_BYTES);
    expect(h.mapWireSpan({ start: 0, end: 1 }, 1)).toEqual({ from: 0, to: 1 });
  });

  it('exports presentation token spans with exact UTF-16 positions and multiline string state', () => {
    const text = Text.of(['let x "日本😀', '語" :foo 12', '#@ tempo 120']);
    const spans = tokenizerSpans(text);
    expect(spans.map((s) => [text.sliceString(s.from, s.to), s.type])).toEqual([
      ['let', 'head'], ['x', 'name'], ['"日本😀', 'string'], ['語"', 'string'], [':foo', 'keyword'], ['12', 'number'], ['#@ tempo 120', 'directive']
    ]);
    expect(spans.every((s) => s.className === `vact-tok-${s.type}`)).toBe(true);
  });
});
