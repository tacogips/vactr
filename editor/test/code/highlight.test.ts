import { EditorState, Text } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { afterEach, describe, expect, it } from 'vitest';
import {
  HighlightScheduler,
  TimeAnchor,
  durSeconds,
  highlightExtension,
  playingRanges,
  setPlaying,
} from '../../src/code/highlight';
import { DocumentSync } from '../../src/code/sync';
import { Client } from '../../src/protocol/client';
import type { TempoBody, WirePlaying } from '../../src/protocol/types';
import { MockClock } from '../support/clock';
import { RecordingTransport } from '../support/recording';

/**
 * The runtime's default scheduling lookahead (`SchedConfig::default()`,
 * `lookahead: 0.120` s in src/sched/runtime.rs): criterion 1 asks for the
 * highlight within one such window of the event time.
 */
const RUNTIME_LOOKAHEAD_S = 0.12;
const FRAME_S = 1 / 60;

const TEMPO: TempoBody = { bpm: 120, beats_per_cycle: 4, cycle: [0, 1] };
const enc = new TextEncoder();

const views: EditorView[] = [];
afterEach(() => {
  for (const v of views.splice(0)) v.destroy();
});

function setup(text: string, clock = new MockClock()) {
  const client = new Client(new RecordingTransport());
  const initial = Text.of(text.split('\n'));
  const sync = new DocumentSync(client.document('main.vact'), initial);
  const view = new EditorView({
    parent: document.body,
    state: EditorState.create({ doc: initial, extensions: [sync.extension(), highlightExtension()] }),
  });
  views.push(view);
  const sched = new HighlightScheduler({
    clock,
    file: 'main.vact',
    map: (span, rev) => sync.mapWireSpan(span, rev),
    tempo: () => TEMPO,
    apply: (ranges) => view.dispatch({ effects: setPlaying.of(ranges) }),
  });
  return { clock, sync, view, sched };
}

function event(text: string, literal: string, time: number, dur: [number, number], rev = 1, occurrence = 0): WirePlaying {
  let at = -1;
  for (let i = 0; i <= occurrence; i += 1) at = text.indexOf(literal, at + 1);
  const start = enc.encode(text.slice(0, at)).length;
  return {
    slot: 'd1',
    beat: [0, 1],
    time,
    dur,
    src: { file: 'main.vact', span: { start, end: start + enc.encode(literal).length }, doc_revision: rev, form_gen: 1 },
  };
}

describe('HighlightScheduler (criterion 1, mock clock)', () => {
  it('computes dur_seconds from the latest tempo', () => {
    // `dur` is in beats on the wire (src/session/publish.rs dur_beats).
    expect(durSeconds([1, 1], TEMPO)).toBeCloseTo(0.5);
    expect(durSeconds([3, 1], { bpm: 60, beats_per_cycle: 3 })).toBeCloseTo(3);
    // The runtime defaults before any tempo.
    expect(durSeconds([1, 1], null)).toBeCloseTo(0.5);
  });

  it('is active exactly over [time, time + dur)', () => {
    const text = 'd1 "bd sd"';
    const { clock, view, sched } = setup(text);
    sched.onPlaying([event(text, 'bd', 1.0, [1, 1])]); // one beat = 0.5 s at 120 bpm
    const at = (t: number) => {
      clock.set(t);
      sched.tick();
      return playingRanges(view).map((r) => view.state.doc.sliceString(r.from, r.to));
    };
    expect(at(0.99)).toEqual([]);
    expect(at(1.0)).toEqual(['bd']);
    expect(at(1.49)).toEqual(['bd']);
    expect(at(1.5)).toEqual([]);
    expect(sched.size).toBe(0);
  });

  it('applies the decoration within one lookahead window at a 1/60 s tick cadence', () => {
    const text = 'd1 "bd sd"';
    // Worst case: the event lands just after a tick.
    for (const phase of [0, 0.001, FRAME_S / 2, FRAME_S - 1e-6]) {
      const { clock, view, sched } = setup(text, new MockClock(0));
      const time = 1.0 + phase;
      sched.onPlaying([event(text, 'sd', time, [1, 8])]);
      let applied: number | null = null;
      for (let k = 0; k < 600 && applied === null; k += 1) {
        clock.set(k * FRAME_S);
        sched.tick();
        if (playingRanges(view).length > 0) applied = clock.now();
      }
      expect(applied).not.toBeNull();
      expect((applied ?? 0) - time).toBeGreaterThanOrEqual(0);
      expect((applied ?? 0) - time).toBeLessThanOrEqual(RUNTIME_LOOKAHEAD_S);
      expect((applied ?? 0) - time).toBeLessThanOrEqual(FRAME_S);
    }
  });

  it('drops events without src or of another file', () => {
    const text = 'd1 "bd"';
    const { clock, view, sched } = setup(text);
    const noSrc: WirePlaying = { slot: 'd1', beat: [0, 1], time: 0, dur: [1, 1] };
    const other = event(text, 'bd', 0, [1, 1]);
    if (other.src) other.src.file = 'other.vact';
    sched.onPlaying([noSrc, other]);
    expect(sched.size).toBe(0);
    clock.set(0.1);
    sched.tick();
    expect(playingRanges(view)).toEqual([]);
  });

  it('clears on hush', () => {
    const text = 'd1 "bd"';
    const { clock, view, sched } = setup(text);
    sched.onPlaying([event(text, 'bd', 0, [1, 1])]);
    clock.set(0.1);
    sched.tick();
    expect(playingRanges(view)).toHaveLength(1);
    sched.clear();
    sched.tick();
    expect(playingRanges(view)).toEqual([]);
  });

  it('anchors native-tier host times at the first batch receipt', () => {
    const clock = new MockClock(50);
    const anchor = new TimeAnchor(clock, 'receipt');
    anchor.observe([1000.5, 1000.25]);
    expect(anchor.local(1000.25)).toBeCloseTo(50);
    expect(anchor.local(1001.25)).toBeCloseTo(51);
    // Later batches keep the first anchor.
    clock.set(70);
    anchor.observe([2000]);
    expect(anchor.local(1000.25)).toBeCloseTo(50);
    expect(new TimeAnchor(clock, 'audio').local(3)).toBe(3);
  });
});
