import { ChangeSet, EditorState, Text } from '@codemirror/state';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TextLayout } from '../../src/code/layout';
import { CHECK_DEBOUNCE_MS, DiagnosticsController } from '../../src/code/diagnostics';
import { PhaseTimer } from '../../src/code/frame';
import { InputController, type InputPresentation } from '../../src/code/input';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { LineTable } from '../../src/code/line-bytes';
import { Utf8Index } from '../../src/protocol/utf8';
import { wordMove } from '../../src/code/keyboard';
import { boundary, INPUT_WINDOW_LIMIT, surroundingWindow } from '../../src/code/accessibility';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { RecordingTransport } from '../support/recording';
import { SyntaxSpans, WorkerSyntaxSpans, type ParsedVact, type SyntaxWorkerPort, type VactSyntax } from '../../src/code/syntax';
import type { SyntaxWorkerReply, SyntaxWorkerRequest } from '../../src/code/syntax-worker-core';

const cleanups: (() => void)[] = [];
const lines = Array.from({ length: 20_000 }, (_, n) => `const value${n} = alpha beta gamma${' '.repeat(22)}`);
function spySlices(...docs: Text[]) {
  const prototypes = new Set(docs.map(doc => Object.getPrototypeOf(doc)));
  return [...prototypes].map(prototype => vi.spyOn(prototype as Text & { sliceString: Text['sliceString'] }, 'sliceString'));
}
function setup(inputLines = lines, onPresentation?: (presentation: InputPresentation) => void) {
  const transport = new RecordingTransport(), store = new Store();
  const client = new Client(transport, { store, now: () => Date.now() });
  const sync = new DocumentSync(client.document('main.vact'), Text.of(inputLines));
  const surface = new CodeSurface({ sync }), container = document.createElement('div'); document.body.append(container);
  const input = new InputController(surface, container, { onPresentation });
  cleanups.push(() => { input.dispose(); surface.dispose(); client.close(); store.dispose(); container.remove(); });
  return { surface, input, container, sync, client, store };
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
  it('keeps a 20,000-line edit check out of the keystroke and frame phases', () => {
    vi.useFakeTimers();
    const { surface, input, sync, client } = setup();
    const frameCallbacks: (() => void)[] = [], tasks: (() => void)[] = [];
    let phase = 'idle';
    const check = vi.fn(() => { expect(phase).toBe('task'); return []; });
    const diagnostics = new DiagnosticsController({ client, sync, tier: 'browser', core: { check }, text: () => surface.state.doc.toString(),
      afterPresent(callback) {
        let cancelled = false;
        const frame = (): void => { if (!cancelled) tasks.push(() => { phase = 'task'; if (!cancelled) callback(); }); };
        frameCallbacks.push(frame);
        return () => { cancelled = true; };
      } });
    const stop = surface.subscribe((update) => { if (update.docChanged) diagnostics.noteInput(); });
    cleanups.push(stop, () => diagnostics.dispose());
    const checksDuringKeystroke = check.mock.calls.length;
    phase = 'keystroke'; input.replaceSelection('x', 'input.type');
    expect(check.mock.calls.length - checksDuringKeystroke).toBe(0);
    expect(diagnostics.stats.checks).toBe(0);
    vi.advanceTimersByTime(CHECK_DEBOUNCE_MS);
    phase = 'frame'; for (const frame of frameCallbacks.splice(0)) frame();
    expect(check.mock.calls.length).toBe(0);
    expect(diagnostics.stats.checks).toBe(0);
    tasks.shift()?.();
    expect(check).toHaveBeenCalledTimes(1);
    expect(diagnostics.stats.checks).toBe(1);
  });

  it('keeps a 20,000-line ASCII edit bounded across layout and deferred syntax', () => {
    const metrics = { font: '', measureText: (text: string) => ({ width: text.length * 8 }) };
    const layout = new TextLayout(metrics, { font: '16px mono', lineHeight: 20, baseline: 16 });
    let presented: Text | null = null;
    const { surface, input } = setup(lines, (presentation) => {
      const documentChanged = presented === null || presentation.changes !== undefined;
      presented = presentation.doc;
      if (documentChanged) layout.setText(presentation.doc, presentation.changes);
    });
    const parsed = (): ParsedVact => ({ captures: () => [], changedRanges: () => [], delete: () => {} });
    const afterFrameCallbacks: (() => void)[] = [];
    const syntax = new SyntaxSpans({ parse: () => parsed(), parseDoc: () => parsed(), edit: () => {} } satisfies VactSyntax,
      () => {}, (callback) => { afterFrameCallbacks.push(callback); return () => { const index = afterFrameCallbacks.indexOf(callback); if (index >= 0) afterFrameCallbacks.splice(index, 1); }; });
    syntax.spans(surface.state, 0, 64, 32); syntax.flush(); syntax.stats.deferredParses = 0;
    const stop = surface.subscribe((update) => { if (update.docChanged) syntax.noteChanges(update.changes, update.state); });
    cleanups.push(stop, () => syntax.dispose());

    const line = surface.state.doc.line(10_001);
    surface.dispatch({ selection: { anchor: line.from + 12 } });
    layout.shape(10_000);
    const measures = layout.stats.measuredTextCalls, builds = layout.stats.builds;
    const toString = vi.spyOn(Text.prototype, 'toString');
    input.replaceSelection('x', 'input.type');
    layout.shape(10_000);

    expect(layout.stats.measuredTextCalls - measures).toBe(0);
    expect(layout.stats.builds - builds).toBeLessThanOrEqual(1);
    expect(toString.mock.contexts.filter((context) => (context as Text).length >= 64 * 1024)).toHaveLength(0);
    expect(syntax.stats.syncParses).toBe(0);
    expect(syntax.stats.deferredParses).toBe(0);
    afterFrameCallbacks.shift()?.();
    expect(syntax.stats.deferredParses).toBe(1);
  });

  it('posts one bounded Worker edit without main-thread captures or large stringification', () => {
    const messages: SyntaxWorkerRequest[] = [], listeners = new Map<string, EventListener[]>();
    const port: SyntaxWorkerPort = {
      postMessage(message) { messages.push(message); }, terminate() {},
      addEventListener(type, listener) { listeners.set(type, [...(listeners.get(type) ?? []), listener]); },
    };
    const provider = new WorkerSyntaxSpans(port, 'http://test/');
    const state = EditorState.create({ doc: lines.join('\n') });
    provider.spans(state, 0, 64, 32);
    const reset = messages.find((message) => message.type === 'reset');
    for (const listener of listeners.get('message') ?? []) listener(new MessageEvent('message', { data: { type: 'ready' } satisfies SyntaxWorkerReply }));
    for (const listener of listeners.get('message') ?? []) listener(new MessageEvent('message', { data: { type: 'spans', seq: reset?.type === 'reset' ? reset.seq : 0, lines: new Uint32Array(), spans: new Uint32Array(), truncated: false } satisfies SyntaxWorkerReply }));
    const before = messages.length, next = state.update({ changes: { from: 0, insert: 'x' } });
    const stringify = vi.spyOn(Text.prototype, 'toString'); provider.noteChanges(next.changes, next.state);
    expect(messages).toHaveLength(before + 1); expect(messages.at(-1)?.type).toBe('edit');
    expect(provider.stats.captures).toBe(0);
    expect(stringify.mock.contexts.filter((doc) => (doc as Text).length >= 64 * 1024)).toHaveLength(0);
    stringify.mockRestore(); provider.dispose();
  });

  it('defers textarea edits, then patches only the changed window without a layout read', () => {
    const { surface, input } = setup();
    const textarea = input.accessibility.textarea;
    const valueSetter = vi.spyOn(HTMLTextAreaElement.prototype, 'value', 'set');
    const setRangeText = vi.spyOn(textarea, 'setRangeText');
    const rect = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect');
    input.replaceSelection('x', 'input.type');
    expect(valueSetter).not.toHaveBeenCalled();
    expect(rect).not.toHaveBeenCalled();
    input.flushBridge();
    expect(valueSetter).not.toHaveBeenCalled();
    expect(setRangeText).toHaveBeenCalledTimes(1);
    expect(rect).not.toHaveBeenCalled();
  });

  it('flushes a pending edit before beforeinput reads the textarea and avoids caret-only value writes', () => {
    const { surface, input } = setup(lines.slice(0, 40));
    const textarea = input.accessibility.textarea;
    const valueSetter = vi.spyOn(HTMLTextAreaElement.prototype, 'value', 'set');
    const setRangeText = vi.spyOn(textarea, 'setRangeText');
    input.replaceSelection('x', 'input.type');
    const pendingValue = surroundingWindow(surface).value;
    expect(textarea.value).not.toBe(pendingValue);
    let observed = '';
    textarea.addEventListener('beforeinput', () => { observed = textarea.value; });
    textarea.dispatchEvent(new InputEvent('beforeinput', { inputType: 'insertFromPaste', cancelable: true }));
    expect(observed).toBe(pendingValue);
    valueSetter.mockClear(); setRangeText.mockClear();
    surface.dispatch({ selection: { anchor: 2 } });
    input.flushBridge();
    expect(valueSetter).not.toHaveBeenCalled();
    expect(setRangeText).not.toHaveBeenCalled();
  });

  it('caches presentation identity and keeps a one-character edit free of whole-document strings', () => {
    const { surface, input } = setup();
    expect(input.presentation.doc).toBe(surface.state.doc);
    const before = input.presentation; expect(input.presentation).toBe(before);
    const toString = vi.spyOn(Text.prototype, 'toString');
    const indexBuilds = Utf8Index.builds, tableBuilds = LineTable.builds;
    const sliceString = spySlices(surface.state.doc);
    surface.dispatch({ selection: { anchor: surface.state.doc.line(10_001).from + 12 } });
    input.replaceSelection('x', 'input.type');
    expect(toString.mock.contexts.every(context => (context as Text).length < 64 * 1024)).toBe(true);
    expect(Utf8Index.builds).toBe(indexBuilds);
    expect(LineTable.builds).toBe(tableBuilds);
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
    input.flushBridge();
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
    const changes = ChangeSet.of({ from: doc.line(10_001).from, insert: 'x' }, doc.length);
    const changed = changes.apply(doc);
    layout.setText(changed, changes); layout.shape(9_999);
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
    const changes = ChangeSet.of({ from: 0, insert: 'x' }, doc.length);
    const changed = changes.apply(doc);
    layout.setText(changed, changes);
    const reused = layout.shape(lineNumber);
    expect(layout.stats.builds).toBe(builds);
    expect(reused).not.toBe(shaped);
    expect(reused.from).toBe(shapedFrom + 1);
    expect(reused.to).toBe(shapedTo + 1);
    expect(reused.runs[0]).toBe(firstRun);
    expect(reused.runs[0]?.from).toBe(runFrom + 1);
    expect(reused.runs[0]?.to).toBe(runTo + 1);
  });

  it('keeps earlier line identity and renumbers cached lines after an Enter change set', () => {
    const doc = Text.of(lines), layout = new TextLayout({ font: '', measureText: text => ({ width: text.length * 8 }) },
      { font: '16px mono', lineHeight: 20, baseline: 16 });
    layout.setText(doc);
    const before = layout.shape(9_998), shifted = layout.shape(15_000), run = shifted.runs[0]!;
    const changes = ChangeSet.of({ from: doc.line(10_001).from, insert: '\n' }, doc.length);
    layout.setText(changes.apply(doc), changes);
    expect(layout.shape(9_998)).toBe(before);
    const renumbered = layout.shape(15_001);
    expect(renumbered.runs[0]).toBe(run);
    expect(renumbered.from).toBe(shifted.from + 1);
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
