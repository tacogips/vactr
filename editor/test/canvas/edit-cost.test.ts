import { Text } from '@codemirror/state';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TextLayout } from '../../src/code/layout';
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
});
