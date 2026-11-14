// Criterion 10, the highlight half: playing highlights survive edits at
// their mapped positions, drop when their text is edited, follow their
// text through reorders, stay independent for duplicate literals, and
// resolve against the event's own (earlier) doc_revision.

import { moveLineDown } from '@codemirror/commands';
import { Text } from '@codemirror/state';
import { afterEach, describe, expect, it } from 'vitest';
import { HighlightScheduler } from '../../src/code/highlight';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { Client } from '../../src/protocol/client';
import type { WirePlaying } from '../../src/protocol/types';
import { MockClock } from '../support/clock';
import { RecordingTransport } from '../support/recording';

const enc = new TextEncoder();
const views: CodeSurface[] = [];
afterEach(() => {
  for (const v of views.splice(0)) v.dispose();
});
const playingRanges = (surface: CodeSurface) => surface.annotationRanges().filter((range) => range.kind === 'playing');

function setup(text: string) {
  const clock = new MockClock(0);
  const client = new Client(new RecordingTransport());
  const initial = Text.of(text.split('\n'));
  const sync = new DocumentSync(client.document('main.vact'), initial);
  const view = new CodeSurface({ sync });
  views.push(view);
  const sched = new HighlightScheduler({
    clock,
    file: 'main.vact',
    map: (span, rev) => sync.mapWireSpan(span, rev),
    tempo: () => ({ bpm: 120, beats_per_cycle: 4, cycle: [0, 1] }),
    apply: (ranges) => view.annotate('playing', ranges.map((range) => ({ ...range, kind: 'playing' }))),
  });
  const shown = (): string[] => {
    sched.tick();
    return playingRanges(view).map((r) => view.state.doc.sliceString(r.from, r.to));
  };
  const positions = (): number[] => playingRanges(view).map((r) => r.from);
  return { clock, sync, view, sched, shown, positions };
}

/** An event for the `occurrence`-th `literal` of `text` (revision `rev`), active over [0, 2). */
function ev(text: string, literal: string, rev: number, occurrence = 0, slot = 'd1'): WirePlaying {
  let at = -1;
  for (let i = 0; i <= occurrence; i += 1) at = text.indexOf(literal, at + 1);
  if (at < 0) throw new Error(`no ${literal}`);
  const start = enc.encode(text.slice(0, at)).length;
  return {
    slot,
    beat: [0, 1],
    time: 0,
    dur: [1, 1],
    src: { file: 'main.vact', span: { start, end: start + enc.encode(literal).length }, doc_revision: rev, form_gen: 1 },
  };
}

describe('highlight reconciliation (criterion 10)', () => {
  it('keeps the highlight of a playing form at its mapped position after an insertion above', () => {
    const text = 'let a 1\nd1 "bd sd"';
    const { view, sched, shown, positions } = setup(text);
    sched.onPlaying([ev(text, 'sd', 1)]);
    expect(shown()).toEqual(['sd']);
    const before = positions()[0] ?? -1;
    view.dispatch({ changes: { from: 0, insert: '# ünïcode 日本\n' } });
    expect(shown()).toEqual(['sd']);
    expect(positions()[0]).toBe(before + '# ünïcode 日本\n'.length);
  });

  it('drops the highlight when its literal is deleted', () => {
    const text = 'd1 "bd sd"';
    const { view, sched, shown } = setup(text);
    sched.onPlaying([ev(text, 'bd', 1)]);
    expect(shown()).toEqual(['bd']);
    const at = text.indexOf('bd');
    view.dispatch({ changes: { from: at, to: at + 3 } });
    expect(shown()).toEqual([]);
    expect(sched.size).toBe(0);
  });

  it('drops the highlight when its literal is edited inside', () => {
    const text = 'd1 "bd sd"';
    const { view, sched, shown } = setup(text);
    sched.onPlaying([ev(text, 'bd', 1)]);
    view.dispatch({ changes: { from: text.indexOf('bd') + 1, insert: 'x' } });
    expect(shown()).toEqual([]);
  });

  it('maps each highlight with its text when two forms are reordered (moveLineDown)', () => {
    const a = 'd1 "bd"';
    const b = 'd2 "hh"';
    const text = `${a}\n${b}`;
    const { view, sched, shown } = setup(text);
    sched.onPlaying([ev(text, 'bd', 1), ev(text, 'hh', 1, 0, 'd2')]);
    expect(shown()).toEqual(['bd', 'hh']);
    // The editor's own reorder: the cursor form moves below the next one.
    view.dispatch({ selection: { anchor: 1 } });
    const commandView = { get state() { return view.state; }, dispatch: (tr: Parameters<typeof view.dispatch>[0]) => view.dispatch(tr) };
    expect(moveLineDown(commandView as Parameters<typeof moveLineDown>[0])).toBe(true);
    expect(view.state.doc.toString()).toBe(`${b}\n${a}`);
    // The moved form keeps its highlight at its new position. The line it
    // jumped over is re-typed by the command (a change over its text), so
    // that highlight drops until the next event of the new revision.
    expect(shown()).toEqual(['bd']);
    expect(playingRanges(view)[0]?.from).toBe(b.length + 1 + a.indexOf('bd'));
    const rev = view.state.doc.toString();
    sched.onPlaying([ev(rev, 'hh', 2, 0, 'd2')]);
    expect(shown()).toEqual(['hh', 'bd']);
  });

  it('follows both forms when the reorder moves text around them', () => {
    const text = 'd1 "bd"\n\nd2 "hh"';
    const { view, sched, shown } = setup(text);
    sched.onPlaying([ev(text, 'bd', 1), ev(text, 'hh', 1, 0, 'd2')]);
    // Swap the blank separator for a comment and prepend a new form.
    view.dispatch({
      changes: [
        { from: 0, insert: 'd3 "cp"\n' },
        { from: 8, to: 9, insert: '# sep' },
      ],
    });
    expect(shown()).toEqual(['bd', 'hh']);
    const doc = view.state.doc.toString();
    expect(playingRanges(view).map((r) => r.from)).toEqual([doc.indexOf('bd'), doc.indexOf('hh')]);
  });

  it('highlights duplicate literals independently by span', () => {
    const text = 'd1 "bd bd bd"';
    const { view, sched, shown } = setup(text);
    sched.onPlaying([ev(text, 'bd', 1, 0), ev(text, 'bd', 1, 2)]);
    expect(shown()).toEqual(['bd', 'bd']);
    const ranges = playingRanges(view);
    expect(ranges.map((r) => r.from)).toEqual([text.indexOf('bd'), text.lastIndexOf('bd')]);
    // Editing the middle duplicate touches neither highlighted one.
    const mid = text.indexOf('bd', text.indexOf('bd') + 1);
    view.dispatch({ changes: { from: mid, to: mid + 2, insert: 'sn' } });
    expect(shown()).toEqual(['bd', 'bd']);
    // Editing the first drops only that one.
    view.dispatch({ changes: { from: text.indexOf('bd'), to: text.indexOf('bd') + 2, insert: 'cp' } });
    expect(shown()).toEqual(['bd']);
    expect(playingRanges(view)[0]?.from).toBe(text.lastIndexOf('bd'));
  });

  it('highlights an event tagged with an earlier doc_revision through its own src', () => {
    // Revision 1: the stored list was evaluated here.
    const v1 = 'let xs "bd sd"\nd1 xs';
    const { view, sync, sched, shown } = setup(v1);
    // Later edits: revisions 2 and 3.
    view.dispatch({ changes: { from: 0, insert: '# ö\n' } });
    view.dispatch({ changes: { from: view.state.doc.length, insert: '\nd2 "hh"' } });
    expect(sync.revision).toBe(3);
    const v3 = view.state.doc.toString();
    // An event of revision 1 (the stored list) and one of revision 3.
    sched.onPlaying([ev(v1, 'sd', 1), ev(v3, 'hh', 3, 0, 'd2')]);
    expect(shown()).toEqual(['sd', 'hh']);
    expect(playingRanges(view)[0]?.from).toBe(v3.indexOf('sd'));
    // A revision the history never saw highlights nothing.
    sched.onPlaying([ev(v1, 'bd', 9)]);
    expect(shown()).toEqual(['sd', 'hh']);
  });
});
