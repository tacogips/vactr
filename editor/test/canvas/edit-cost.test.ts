import { Text } from '@codemirror/state';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TextLayout } from '../../src/code/layout';
import { PhaseTimer } from '../../src/code/frame';
import { InputController } from '../../src/code/input';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { wordMove } from '../../src/code/keyboard';
import { boundary, INPUT_WINDOW_LIMIT, surroundingWindow } from '../../src/code/accessibility';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { RecordingTransport } from '../support/recording';

const cleanups: (() => void)[] = [];
const lines = Array.from({ length: 20_000 }, (_, n) => `const value${n} = alpha beta gamma${' '.repeat(22)}`);
function spySlices(...docs: Text[]) {
  const prototypes = new Set(docs.map(doc => Object.getPrototypeOf(doc)));
  return [...prototypes].map(prototype => vi.spyOn(prototype as Text & { sliceString: Text['sliceString'] }, 'sliceString'));
}
function setup(inputLines = lines) {
  const transport = new RecordingTransport(), store = new Store();
  const client = new Client(transport, { store, now: () => Date.now() });
  const sync = new DocumentSync(client.document('main.vact'), Text.of(inputLines));
  const surface = new CodeSurface({ sync }), container = document.createElement('div'); document.body.append(container);
  const input = new InputController(surface, container);
  cleanups.push(() => { input.dispose(); surface.dispose(); client.close(); store.dispose(); container.remove(); });
  return { surface, input, container };
}
function oldWindow(text: string, head: number, anchor: number): { start: number; end: number; value: string; anchor: number; head: number; outside: boolean } {
  let start = boundary(text, Math.max(0, head - INPUT_WINDOW_LIMIT / 2), 1);
  let end = boundary(text, Math.min(text.length, start + INPUT_WINDOW_LIMIT), -1);
  if (end === text.length) start = boundary(text, Math.max(0, end - INPUT_WINDOW_LIMIT), 1);
  if (end < start || head < start || head > end) start = end = boundary(text, head, -1);
  return { start, end, value: text.slice(start, end), anchor, head, outside: anchor < start || anchor > end };
}
afterEach(() => { for (const cleanup of cleanups.splice(0).reverse()) cleanup(); vi.restoreAllMocks(); document.body.replaceChildren(); });

describe('bounded document edit costs', () => {
  it('caches presentation identity and keeps a one-character edit free of whole-document strings', () => {
    const { surface, input } = setup();
    expect(input.presentation.doc).toBe(surface.state.doc);
    const before = input.presentation; expect(input.presentation).toBe(before);
    const toString = vi.spyOn(Text.prototype, 'toString');
    const sliceString = spySlices(surface.state.doc);
    surface.dispatch({ selection: { anchor: surface.state.doc.line(10_001).from + 12 } });
    input.replaceSelection('x', 'input.type');
    expect(toString.mock.contexts.every(context => (context as Text).length < 64 * 1024)).toBe(true);
    expect(sliceString.flatMap(spy => spy.mock.calls).some(([from, to]) => (to ?? Infinity) - from > 64 * 1024)).toBe(false);
  });

  it('matches the old accessibility window for a huge line and a 20,000-line document', () => {
    const hugeLine = ['x'.repeat(100_000)];
    const one = setup(hugeLine); one.surface.dispatch({ selection: { anchor: 51_237 } });
    const expectedOne = oldWindow(one.surface.state.doc.toString(), 51_237, 51_237);
    const multi = setup(); const head = multi.surface.state.doc.line(10_001).from + 23;
    multi.surface.dispatch({ selection: { anchor: head } });
    const expectedMulti = oldWindow(multi.surface.state.doc.toString(), head, head);
    const toString = vi.spyOn(Text.prototype, 'toString');
    const sliceString = spySlices(one.surface.state.doc, multi.surface.state.doc);
    expect(surroundingWindow(one.surface)).toEqual(expectedOne);
    expect(surroundingWindow(multi.surface)).toEqual(expectedMulti);
    expect(toString.mock.contexts.every(context => (context as Text).length < 64 * 1024)).toBe(true);
    expect(sliceString.flatMap(spy => spy.mock.calls).every(([from, to]) => (to ?? Infinity) - from <= 2 * INPUT_WINDOW_LIMIT + 8)).toBe(true);
  });

  it('maps forward and backward textarea selections like the former whole-string boundary algorithm', () => {
    const { surface, input } = setup(); const head = surface.state.doc.line(10_001).from + 20;
    surface.dispatch({ selection: { anchor: head } });
    const text = surface.state.doc.toString(), window = input.accessibility.window, textarea = input.accessibility.textarea;
    const expected = (start: number, end: number, backward: boolean) => ({
      anchor: boundary(text, window.start + (backward ? end : start), backward ? 1 : -1),
      head: boundary(text, window.start + (backward ? start : end), backward ? -1 : 1),
    });
    const toString = vi.spyOn(Text.prototype, 'toString'), sliceString = spySlices(surface.state.doc);
    textarea.setSelectionRange(5, 12, 'forward');
    expect(input.accessibility.readSelection()).toEqual(expected(5, 12, false));
    textarea.setSelectionRange(5, 12, 'backward');
    expect(input.accessibility.readSelection()).toEqual(expected(5, 12, true));
    expect(toString).not.toHaveBeenCalled();
    expect(sliceString.flatMap(spy => spy.mock.calls).every(([from, to]) => (to ?? Infinity) - from <= 16)).toBe(true);
  });

  it('matches whole-document word movement and deletion while slicing only involved lines', () => {
    const { surface, input } = setup();
    const doc = surface.state.doc, line = doc.line(10_001), position = line.from + 17;
    const reference = doc.toString();
    const moved = wordMove(reference, position, -1);
    surface.dispatch({ selection: { anchor: position } });
    const toString = vi.spyOn(Text.prototype, 'toString');
    const sliceString = spySlices(doc);
    input.keyboard.handle(new KeyboardEvent('keydown', { key: 'ArrowLeft', ctrlKey: true, cancelable: true }));
    expect(surface.state.selection.main.head).toBe(moved);
    expect(toString).not.toHaveBeenCalled();
    expect(sliceString.flatMap(spy => spy.mock.calls).every(([from, to]) => (to ?? Infinity) - from <= 2 * INPUT_WINDOW_LIMIT + 8)).toBe(true);
    toString.mockClear(); for (const spy of sliceString) spy.mockClear();
    surface.dispatch({ selection: { anchor: position } });
    const deletionEnd = wordMove(reference, position, -1);
    input.keyboard.delete(-1, true);
    expect(surface.state.doc.length).toBe(reference.length - (position - deletionEnd));
    expect(toString).not.toHaveBeenCalled();
    expect(sliceString.flatMap(spy => spy.mock.calls).every(([from, to]) => (to ?? Infinity) - from <= 2 * INPUT_WINDOW_LIMIT + 8)).toBe(true);
  });

  it('retains shaped lines before an edit at line 10,000', () => {
    const doc = Text.of(lines), layout = new TextLayout({ font: '', measureText: text => ({ width: text.length * 8 }) },
      { font: '16px mono', lineHeight: 20, baseline: 16 });
    layout.setText(doc);
    for (let n = 0; n < 10_000; n++) layout.shape(n);
    const builds = layout.stats.builds;
    const changed = doc.replace(doc.line(10_001).from, doc.line(10_001).from, Text.of(['x']));
    layout.setText(changed); layout.shape(9_999);
    expect(layout.stats.builds).toBe(builds);
  });

  it('reuses unchanged shaped lines after a prefix edit shifts their document offsets', () => {
    const doc = Text.of(lines), layout = new TextLayout({ font: '', measureText: text => ({ width: text.length * 8 }) },
      { font: '16px mono', lineHeight: 20, baseline: 16 });
    layout.setText(doc);
    const lineNumber = 15_000, shaped = layout.shape(lineNumber);
    const shapedFrom = shaped.from, shapedTo = shaped.to, firstRun = shaped.runs[0]!;
    const runFrom = firstRun.from, runTo = firstRun.to;
    const builds = layout.stats.builds;
    const changed = doc.replace(0, 0, Text.of(['x']));
    layout.setText(changed);
    const reused = layout.shape(lineNumber);
    expect(layout.stats.builds).toBe(builds);
    expect(reused).not.toBe(shaped);
    expect(reused.from).toBe(shapedFrom + 1);
    expect(reused.to).toBe(shapedTo + 1);
    expect(reused.runs[0]).toBe(firstRun);
    expect(reused.runs[0]?.from).toBe(runFrom + 1);
    expect(reused.runs[0]?.to).toBe(runTo + 1);
  });

  it('sets the canvas metrics font once and only updates it after a font change', () => {
    let currentFont = '', assignments = 0;
    const metrics = {
      get font() { return currentFont; },
      set font(value: string) { currentFont = value; assignments++; },
      measureText(text: string) { return { width: text.length * 8 }; },
    };
    const layout = new TextLayout(metrics, { font: '13px mono', lineHeight: 18, baseline: 14 });
    layout.setText(Text.of(lines.slice(0, 100)));
    layout.shape(0); layout.shape(1); layout.shape(2);
    expect(assignments).toBe(1);
    layout.setFont({ ...layout.font, font: '14px mono' });
    expect(assignments).toBe(2);
    layout.shape(0);
    expect(assignments).toBe(2);
  });
  it('bounds long-line measurements without splitting grapheme clusters', () => {
    const text = `${'x'.repeat(1546)}e\u0301${'y'.repeat(300)}`;
    const measured: string[] = [];
    const segment = vi.spyOn(Intl.Segmenter.prototype, 'segment');
    const layout = new TextLayout({ font: '', measureText: value => { measured.push(value); return { width: value.length * 8 }; } },
      { font: '13px mono', lineHeight: 18, baseline: 14 });
    layout.phases = new PhaseTimer(() => 0);
    layout.setDocument(text);
    const line = layout.shape(0);
    expect(segment).toHaveBeenCalledTimes(1);
    expect(line.runs.map(run => run.text).join('')).toBe(text);
    expect(line.runs).toHaveLength(1);
    expect(measured.every(part => part.length <= 256)).toBe(true);
    expect(measured.filter(part => part.includes('e\u0301'))).toHaveLength(1);
    expect(measured.some(part => part.endsWith('e'))).toBe(false);
    expect(line.width).toBe(text.length * 8);
    expect(layout.stats.maxMeasuredTextLength).toBeLessThanOrEqual(256);
    expect(layout.stats.measuredTextCalls).toBe(measured.length);
    const calls = layout.stats.measuredTextCalls;
    const midpoint = layout.offsetInRun(line.runs[0]!, line.width / 2, 1);
    expect(midpoint).toBeGreaterThanOrEqual(922);
    expect(midpoint).toBeLessThanOrEqual(924);
    expect(layout.stats.measuredTextCalls).toBeGreaterThan(calls);
    const indexedCalls = layout.stats.measuredTextCalls;
    expect(layout.offsetInRun(line.runs[0]!, line.width / 2, 1)).toBe(midpoint);
    expect(layout.stats.measuredTextCalls - indexedCalls).toBe(0);
    const beforeAdvance = layout.stats.measuredTextCalls;
    expect(layout.advance(line, line.from + midpoint)).toBe(midpoint * 8);
    const afterAdvance = layout.stats.measuredTextCalls;
    expect(layout.advance(line, line.from + midpoint)).toBe(midpoint * 8);
    expect(layout.stats.measuredTextCalls).toBe(afterAdvance);
    expect(afterAdvance - beforeAdvance).toBeLessThanOrEqual(1);
    expect(layout.stats.maxMeasuredTextLength).toBeLessThanOrEqual(256);
    expect(segment).toHaveBeenCalledTimes(1);
    layout.invalidate();
    layout.shape(0);
    expect(segment).toHaveBeenCalledTimes(2);
  });
  it('caches short-run prefix widths shared by caret and tile position lookups', () => {
    const measured: string[] = [];
    const layout = new TextLayout({ font: '', measureText: text => { measured.push(text); return { width: text.length * 8 }; } },
      { font: '13px mono', lineHeight: 18, baseline: 14 });
    layout.phases = new PhaseTimer(() => 0);
    layout.setDocument('abcdefghij');
    const line = layout.shape(0), run = line.runs[0]!;
    const before = layout.stats.measuredTextCalls;
    const position = layout.offsetInRun(run, 32, 1);
    const afterFirstLookup = layout.stats.measuredTextCalls;
    expect(position).toBe(run.from + 4);
    expect(afterFirstLookup).toBeGreaterThan(before);
    expect(layout.offsetInRun(run, 32, 1)).toBe(position);
    expect(layout.advance(line, position)).toBe(32);
    expect(layout.stats.measuredTextCalls).toBe(afterFirstLookup);
    expect(measured.every(text => text.length <= 256)).toBe(true);
  });
});
