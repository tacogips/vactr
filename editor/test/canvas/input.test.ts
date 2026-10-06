import { Text } from '@codemirror/state';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { InputController } from '../../src/code/input';
import { boundary, INPUT_WINDOW_LIMIT } from '../../src/code/accessibility';
import { PointerController } from '../../src/code/pointer';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { RecordingTransport } from '../support/recording';

const cleanup: (() => void)[] = [];
function setup(text = '日本😀 abc') {
  const transport = new RecordingTransport();
  const client = new Client(transport, { store: new Store(), now: () => Date.now() });
  const sync = new DocumentSync(client.document('main.vact'), Text.of(text.split('\n')));
  const surface = new CodeSurface({ sync });
  const container = document.createElement('div'); document.body.append(container);
  const evalSelection = vi.fn(), evalAll = vi.fn(), hush = vi.fn(), scrollCaret = vi.fn(), onError = vi.fn();
  const input = new InputController(surface, container, { evalSelection, evalAll, hush, scrollCaret, onError });
  cleanup.push(() => { input.dispose(); surface.dispose(); container.remove(); });
  return { surface, sync, input, container, el: input.accessibility.textarea, transport, evalSelection, evalAll, hush, scrollCaret, onError };
}
function composition(el: HTMLElement, kind: string, data = '') { el.dispatchEvent(new CompositionEvent(kind, { data, bubbles: true })); }
function before(el: HTMLElement, inputType: string, data: string | null = null, isComposing = false) {
  const event = new InputEvent('beforeinput', { inputType, data, isComposing, bubbles: true, cancelable: true }); el.dispatchEvent(event); return event;
}
function key(el: HTMLElement, k: string, options: KeyboardEventInit = {}) {
  const event = new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true, ...options }); el.dispatchEvent(event); return event;
}
function clip(el: HTMLElement, kind: string, data: { getData?: (kind: string) => string; setData?: (kind: string, value: string) => void } | null) {
  const event = new Event(kind, { bubbles: true, cancelable: true }); Object.defineProperty(event, 'clipboardData', { value: data }); el.dispatchEvent(event); return event;
}
function pointerSetup(text = 'alpha beta\ngamma') {
  const env = setup(text), element = document.createElement('canvas'); env.container.append(element);
  const capture = new Set<number>();
  element.setPointerCapture = vi.fn((id) => { capture.add(id); });
  element.hasPointerCapture = (id) => capture.has(id);
  element.releasePointerCapture = vi.fn((id) => { capture.delete(id); });
  element.getBoundingClientRect = () => ({ left: 0, right: 100, top: 0, bottom: 40, x: 0, y: 0, width: 100, height: 40, toJSON: () => ({}) });
  const scrollBy = vi.fn(), onHandles = vi.fn(), focus = vi.fn(), numericDrag = vi.fn();
  const detach = env.surface.attachBridge({ focus, posAtCoords: ({ x }) => Math.max(0, Math.min(env.surface.state.doc.length, Math.round(x / 10))),
    coordsAtPos: (pos) => ({ left: pos * 10, right: pos * 10 + 1, top: 0, bottom: 20 }) });
  const pointer = new PointerController(env.surface, element, { scrollBy, onHandles, focus, numericDrag });
  cleanup.push(() => { pointer.dispose(); detach(); });
  function fire(type: string, props: Partial<PointerEvent> = {}) {
    const event = new Event(type, { bubbles: true, cancelable: true });
    for (const [name, value] of Object.entries({ pointerId: 1, pointerType: 'mouse', button: 0, isPrimary: true, clientX: 30, clientY: 10, detail: 1, ...props })) Object.defineProperty(event, name, { value });
    element.dispatchEvent(event); return event;
  }
  return { ...env, element, capture, scrollBy, onHandles, focus, numericDrag, pointer, fire };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => { for (const fn of cleanup.splice(0).reverse()) fn(); vi.useRealTimers(); });

describe('canvas input bridge', () => {
  it('keeps preedit on presentation only and commits Japanese exactly once with one undo group', () => {
    const { surface, sync, el, input } = setup('abc'); surface.dispatch({ selection: { anchor: 1, head: 2 } });
    composition(el, 'compositionstart');
    const beforePreedit = input.presentation;
    composition(el, 'compositionupdate', 'に');
    const firstPreedit = input.presentation;
    expect(firstPreedit.changes?.apply(beforePreedit.doc).eq(firstPreedit.doc)).toBe(true);
    before(el, 'insertCompositionText', '日本', true);
    const secondPreedit = input.presentation;
    expect(secondPreedit.changes?.apply(firstPreedit.doc).eq(secondPreedit.doc)).toBe(true);
    expect(surface.state.doc.toString()).toBe('abc'); expect(sync.revision).toBe(1);
    expect(input.presentation.text).toBe('a日本c');
    expect(input.presentation.doc.toString()).toBe('a日本c');
    expect(input.presentation.cursor).toBe(3);
    expect(input.presentation.annotations).toEqual([{ from: 1, to: 3, kind: 'composition' }]);
    composition(el, 'compositionend', '日本'); before(el, 'insertFromComposition', '日本', true);
    el.dispatchEvent(new InputEvent('input', { inputType: 'insertFromComposition', data: '日本' }));
    expect(surface.state.doc.toString()).toBe('a日本c'); expect(sync.revision).toBe(2);
    expect(surface.retentionStatus.undoDepth).toBe(1); surface.undo(); expect(surface.state.doc.toString()).toBe('abc');
    surface.redo(); expect(surface.state.doc.toString()).toBe('a日本c');
  });
  it('supports final beforeinput occurring before compositionend', () => {
    const { el, surface, sync } = setup(''); composition(el, 'compositionstart');
    before(el, 'insertFromComposition', '語'); composition(el, 'compositionend', '語');
    expect(surface.state.doc.toString()).toBe('語'); expect(sync.revision).toBe(2);
  });
  it('cancels without any document revision and suppresses late cancelled input', () => {
    const { el, surface, sync } = setup('abc'); surface.dispatch({ selection: { anchor: 0, head: 2 } });
    composition(el, 'compositionstart'); composition(el, 'compositionupdate', '日本'); composition(el, 'compositionend');
    before(el, 'insertFromComposition', ''); expect(surface.state.doc.toString()).toBe('abc'); expect(sync.revision).toBe(1);
    expect(surface.compositionRange).toBeNull();
  });
  it('Escape and blur cancel preedit, release deferral, and retain original selection', () => {
    const { el, input, surface } = setup('abc'); surface.dispatch({ selection: { anchor: 1, head: 2 } });
    composition(el, 'compositionstart'); key(el, 'Escape'); expect(input.isComposing).toBe(false);
    expect(surface.state.selection.main.from).toBe(1); expect(surface.state.selection.main.to).toBe(2);
    composition(el, 'compositionstart'); el.dispatchEvent(new Event('blur')); expect(surface.compositionRange).toBeNull();
  });
  it('freezes bridge and suppresses evaluation/history/navigation during composition', () => {
    const { el, surface, evalAll, evalSelection, hush } = setup(); composition(el, 'compositionstart');
    el.value = 'native preedit'; surface.annotate('feedback', []);
    key(el, 'Enter', { ctrlKey: true }); key(el, 'Enter', { metaKey: true, shiftKey: true }); key(el, '.', { ctrlKey: true }); key(el, 'a', { ctrlKey: true });
    expect(el.value).toBe('native preedit'); expect(evalAll).not.toHaveBeenCalled(); expect(evalSelection).not.toHaveBeenCalled(); expect(hush).not.toHaveBeenCalled();
    expect(surface.state.selection.main.empty).toBe(true);
  });
  it('drains overlapping source writes after commit so they revalidate expected text', () => {
    const { el, surface } = setup('abc'); surface.dispatch({ selection: { anchor: 1, head: 2 } }); composition(el, 'compositionstart');
    const write = vi.fn(() => { if (surface.state.doc.sliceString(1, 2) === 'b') surface.dispatch({ changes: { from: 1, to: 2, insert: 'X' } }); });
    surface.deferSourceWrite({ from: 1, to: 2 }, write); expect(write).not.toHaveBeenCalled();
    composition(el, 'compositionend', '語'); expect(write).toHaveBeenCalledOnce(); expect(surface.state.doc.toString()).toBe('a語c');
  });
  it('maps retained composition across an independent edit and rejects a conflicting direct write', () => {
    const { el, surface, input, onError } = setup('abc'); surface.dispatch({ selection: { anchor: 1, head: 2 } }); composition(el, 'compositionstart');
    surface.dispatch({ changes: { from: 0, insert: 'X' } }); input.updateComposition('語'); expect(input.presentation.text).toBe('Xa語c');
    surface.dispatch({ changes: { from: 2, to: 3, insert: 'Y' } }); composition(el, 'compositionend', '語');
    expect(surface.state.doc.toString()).toBe('XaYc'); expect(onError).toHaveBeenCalledWith('Composition cancelled: source changed');
  });
  it('isolates composition from adjacent ordinary typing', () => {
    const { el, surface } = setup(''); before(el, 'insertText', 'a'); composition(el, 'compositionstart'); composition(el, 'compositionend', '語');
    before(el, 'insertText', 'b'); surface.undo(); expect(surface.state.doc.toString()).toBe('a語'); surface.undo(); expect(surface.state.doc.toString()).toBe('a');
  });
  it('reconciles normal beforeinput plus redundant input without duplication', () => {
    const { el, surface, sync } = setup(''); expect(before(el, 'insertText', '😀').defaultPrevented).toBe(true);
    el.dispatchEvent(new InputEvent('input', { inputType: 'insertText', data: '😀' }));
    expect(surface.state.doc.toString()).toBe('😀'); expect(sync.revision).toBe(2);
  });
  it('reconciles native fallback replacement containing surrogate and combining clusters', () => {
    const { el, surface, input } = setup('A😀e\u0301Z'); el.value = 'A😁e\u0301Z'; el.dispatchEvent(new InputEvent('input', { inputType: 'insertReplacementText' }));
    input.flushBridge(); expect(el.value).toBe('A😁e\u0301Z');
    expect(surface.state.doc.toString()).toBe('A😁e\u0301Z'); surface.undo(); expect(surface.state.doc.toString()).toBe('A😀e\u0301Z');
  });
  it('bounds accessibility windows and navigates across a window boundary', () => {
    const { surface, input, el } = setup('日本😀e\u0301'.repeat(5000)); surface.dispatch({ selection: { anchor: 10000 } });
    input.flushBridge();
    const oldStart = input.accessibility.window.start; expect(el.value.length).toBeLessThanOrEqual(INPUT_WINDOW_LIMIT);
    expect(boundary(surface.state.doc.toString(), oldStart)).toBe(oldStart);
    el.setSelectionRange(0, 0); el.dispatchEvent(new Event('select')); expect(surface.state.selection.main.head).toBe(oldStart);
    key(el, 'ArrowLeft'); expect(surface.state.selection.main.head).toBeLessThan(oldStart); expect(input.accessibility.window.start).toBeLessThan(oldStart);
    expect(el.style.opacity).toBe('0'); expect(el.getAttribute('aria-label')).toBe('Code editor');
  });
  it('preserves full-document selection despite a bounded projected select event and copies all source', () => {
    const { surface, el, input } = setup('日本😀'.repeat(5000)); key(el, 'a', { metaKey: true });
    el.dispatchEvent(new Event('select')); expect(surface.state.selection.main.to).toBe(20000);
    expect(input.accessibility.window.outside).toBe(true); const setData = vi.fn(); clip(el, 'copy', { setData });
    expect(setData).toHaveBeenCalledWith('text/plain', '日本😀'.repeat(5000));
    before(el, 'insertText', '語'); expect(surface.state.doc.toString()).toBe('語'); surface.undo(); expect(surface.state.doc.length).toBe(20000);
  });
  it('cuts and pastes document selections through history, and reports clipboard denial without deleting', () => {
    const { surface, el, onError, input } = setup('日本😀 abc'); surface.dispatch({ selection: { anchor: 0, head: 4 } });
    clip(el, 'cut', null); expect(surface.state.doc.toString()).toBe('日本😀 abc'); expect(onError).toHaveBeenCalledWith('Clipboard operation denied');
    expect(input.accessibility.status.textContent).toBe('Clipboard operation denied');
    expect(input.accessibility.status.style.clipPath).toBe('none');
    const setData = vi.fn(); clip(el, 'cut', { setData }); expect(setData).toHaveBeenCalledWith('text/plain', '日本😀'); expect(surface.state.doc.toString()).toBe(' abc');
    clip(el, 'paste', { getData: () => '語' }); expect(surface.state.doc.toString()).toBe('語 abc');
    surface.undo(); expect(surface.state.doc.toString()).toBe(' abc'); surface.undo(); expect(surface.state.doc.toString()).toBe('日本😀 abc');
  });
  it('pastes CRLF and tabs using normalized document offsets and round-trips undo', () => {
    const { surface, el, onError } = setup(''); clip(el, 'paste', { getData: () => '日本\r\n\t😀' });
    expect(surface.state.doc.toString()).toBe('日本\n\t😀'); expect(surface.state.selection.main.head).toBe(6);
    expect(onError).not.toHaveBeenCalled(); surface.undo(); expect(surface.state.doc.toString()).toBe('');
    surface.redo(); expect(surface.state.doc.toString()).toBe('日本\n\t😀');
  });
  it('keeps an oversized combining cluster outside the bounded bridge without splitting it', () => {
    const text = 'a' + '\u0301'.repeat(10000);
    const { input, surface, el } = setup(text); surface.dispatch({ selection: { anchor: text.length } });
    expect(el.value.length).toBeLessThanOrEqual(INPUT_WINDOW_LIMIT);
    expect(boundary(text, input.accessibility.window.start)).toBe(input.accessibility.window.start);
    expect(boundary(text, input.accessibility.window.end)).toBe(input.accessibility.window.end);
  });
  it('uses document selection for fallback input when selection extends beyond the bridge window', () => {
    const { surface, input, el } = setup('x'.repeat(20000)); key(el, 'a', { ctrlKey: true }); input.flushBridge(); el.value = 'paste';
    el.dispatchEvent(new InputEvent('input', { inputType: 'insertText' })); expect(surface.state.doc.toString()).toBe('paste');
  });
  it.each([
    ['shared prefix', 'a', 'abc'],
    ['shared suffix', 'c', 'abc'],
    ['shared prefix and suffix', 'a', 'aba'],
  ])('preserves full native replacement with %s outside the window', (_name, original, replacement) => {
    const { surface, input, el, sync } = setup(original.repeat(20000)); key(el, 'a', { ctrlKey: true }); input.flushBridge();
    el.value = replacement; el.dispatchEvent(new InputEvent('input', { inputType: 'insertReplacementText' }));
    expect(surface.state.doc.toString()).toBe(replacement); expect(surface.state.selection.main.head).toBe(replacement.length);
    expect(sync.revision).toBe(2); surface.undo(); expect(surface.state.doc.toString()).toBe(original.repeat(20000));
  });
  it.each([false, true])('preserves unselected context for partially clipped native selection (backward=%s)', (backward) => {
    const { surface, el, input } = setup('a'.repeat(24000));
    const from = backward ? 10000 : 2000, to = backward ? 22000 : 14000;
    surface.dispatch({ selection: { anchor: backward ? to : from, head: backward ? from : to } });
    input.flushBridge();
    const old = input.accessibility.window;
    expect(old.outside).toBe(true);
    const a = Math.max(0, Math.min(old.value.length, from - old.start));
    const b = Math.max(0, Math.min(old.value.length, to - old.start));
    el.value = old.value.slice(0, a) + 'aba' + old.value.slice(b);
    el.dispatchEvent(new InputEvent('input', { inputType: 'insertReplacementText' }));
    expect(surface.state.doc.toString()).toBe('a'.repeat(from) + 'aba' + 'a'.repeat(24000 - to));
    expect(surface.state.selection.main.head).toBe(from + 3); surface.undo(); expect(surface.state.doc.length).toBe(24000);
  });
  it('reconciles replacement even when its text equals the entire projected selection', () => {
    const { surface, input, el } = setup('a'.repeat(20000)); key(el, 'a', { ctrlKey: true }); input.flushBridge();
    const replacement = el.value;
    el.dispatchEvent(new InputEvent('input', { inputType: 'insertReplacementText' }));
    expect(surface.state.doc.toString()).toBe(replacement); expect(surface.state.doc.length).toBe(INPUT_WINDOW_LIMIT);
  });
  it('deletes a partially clipped selection without removing its unselected context', () => {
    const { surface, el, input } = setup('a'.repeat(24000)); surface.dispatch({ selection: { anchor: 2000, head: 14000 } }); input.flushBridge();
    const old = input.accessibility.window;
    const end = Math.max(0, Math.min(old.value.length, 14000 - old.start));
    el.value = old.value.slice(end); el.dispatchEvent(new InputEvent('input', { inputType: 'deleteContentBackward' }));
    expect(surface.state.doc.toString()).toBe('a'.repeat(12000)); expect(surface.state.selection.main.head).toBe(2000);
  });
  it('keeps accessible status bounded and releases nodes/listeners on dispose', () => {
    const { input, el, surface } = setup('abc'); input.accessibility.announce('x'.repeat(1000)); expect(input.accessibility.status.textContent?.length).toBe(256);
    input.dispose(); before(el, 'insertText', 'X'); expect(surface.state.doc.toString()).toBe('abc'); expect(el.isConnected).toBe(false);
  });
});

describe('headless keyboard', () => {
  it('moves and deletes entire emoji, combining graphemes and CRLF', () => {
    const { surface, el } = setup('😀e\u0301X'); surface.dispatch({ selection: { anchor: 4 } }); key(el, 'ArrowLeft'); expect(surface.state.selection.main.head).toBe(2);
    key(el, 'Backspace'); expect(surface.state.doc.toString()).toBe('e\u0301X'); key(el, 'Delete'); expect(surface.state.doc.toString()).toBe('X');
    expect(boundary('a\r\nb', 2, -1)).toBe(1); expect(boundary('a\r\nb', 2, 1)).toBe(3);
  });
  it('supports word, line, document movement and shift extension with caret scroll', () => {
    const { surface, el, scrollCaret } = setup('alpha beta\ngamma'); key(el, 'ArrowRight', { ctrlKey: true }); expect(surface.state.selection.main.head).toBe(5);
    key(el, 'End', { shiftKey: true }); expect(surface.state.selection.main.from).toBe(5); expect(surface.state.selection.main.to).toBe(10);
    key(el, 'Home', { ctrlKey: true }); expect(surface.state.selection.main.head).toBe(0);
    key(el, 'ArrowDown'); expect(surface.state.selection.main.head).toBe(11);
    key(el, 'End', { ctrlKey: true, shiftKey: true }); expect(surface.state.selection.main.to).toBe(16); expect(scrollCaret).toHaveBeenCalled();
  });
  it('bounds punctuation-only word movement to 65,536 UTF-16 units and returns a line start', () => {
    const { surface, el } = setup(Array.from({ length: 70_000 }, () => '!').join('\n'));
    const end = surface.state.doc.length;
    surface.dispatch({ selection: { anchor: end } });
    key(el, 'ArrowLeft', { ctrlKey: true });
    const head = surface.state.selection.main.head;
    expect(head).toBe(surface.state.doc.lineAt(head).from);
    expect(end - head).toBeLessThanOrEqual(65_536);
  });
  it('dispatches eval/hush and keyboard history only outside composition', () => {
    const { surface, el, evalSelection, evalAll, hush } = setup('');
    key(el, 'Enter', { ctrlKey: true }); key(el, 'Enter', { metaKey: true, shiftKey: true }); key(el, '.', { metaKey: true });
    expect(evalSelection).toHaveBeenCalledOnce(); expect(evalAll).toHaveBeenCalledOnce(); expect(hush).toHaveBeenCalledOnce();
    before(el, 'insertText', 'a'); key(el, 'z', { ctrlKey: true }); expect(surface.state.doc.toString()).toBe('');
    key(el, 'z', { metaKey: true, shiftKey: true }); expect(surface.state.doc.toString()).toBe('a');
  });
});

describe('canvas pointer contracts', () => {
  it('places caret, selects word/line and extends a drag; releases capture on blur', () => {
    const { surface, fire, capture } = pointerSetup(); fire('pointerdown'); expect(surface.state.selection.main.head).toBe(3);
    fire('pointermove', { clientX: 80 }); expect(surface.state.selection.main.to).toBe(8); window.dispatchEvent(new Event('blur')); expect(capture.size).toBe(0);
    fire('pointerdown', { detail: 2 }); expect(surface.state.selection.main.from).toBe(0); expect(surface.state.selection.main.to).toBe(5); fire('pointerup');
    fire('pointerdown', { detail: 3 }); expect(surface.state.selection.main.to).toBe(11); fire('pointerup');
  });
  it('selects and drags on a distant line without serializing the whole document', () => {
    const text = Array.from({ length: 20_000 }, (_, n) => `const value${n} = alpha beta gamma`).join('\n');
    const { surface, fire } = pointerSetup(text);
    const line = surface.state.doc.line(10_000);
    surface.attachBridge({ focus() {}, posAtCoords: ({ x }) => line.from + Math.min(line.length, Math.round(x / 10)),
      coordsAtPos: (pos) => ({ left: pos * 10, right: pos * 10 + 1, top: 0, bottom: 20 }) });
    const toString = vi.spyOn(Text.prototype, 'toString');
    fire('pointerdown', { detail: 2 }); fire('pointerup');
    fire('pointerdown'); fire('pointermove', { clientX: 80 });
    expect(toString).not.toHaveBeenCalled();
  });
  it('counts real pointerdown detail zero for repeated word/line selection and numeric refusal', () => {
    const { surface, fire, numericDrag } = pointerSetup();
    fire('pointerdown', { detail: 0 }); fire('pointerup');
    expect(numericDrag).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(50); fire('pointerdown', { detail: 0 });
    expect(surface.state.selection.main.from).toBe(0); expect(surface.state.selection.main.to).toBe(5); fire('pointerup');
    vi.advanceTimersByTime(50); fire('pointerdown', { detail: 0 });
    expect(surface.state.selection.main.to).toBe(11); expect(numericDrag).toHaveBeenCalledTimes(1); fire('pointerup');
  });
  it('preserves caret and native scrolling when touch moves before long press', () => {
    const { surface, fire, capture } = pointerSetup(); surface.dispatch({ selection: { anchor: 1 } });
    fire('pointerdown', { pointerType: 'touch' }); const move = fire('pointermove', { pointerType: 'touch', clientX: 60 }); vi.advanceTimersByTime(600);
    fire('pointerup', { pointerType: 'touch' }); expect(move.defaultPrevented).toBe(false); expect(surface.state.selection.main.head).toBe(1); expect(capture.size).toBe(0);
  });
  it('long press selects a word, emits GPU handles, then supports handle drag', () => {
    const { surface, fire, onHandles, capture } = pointerSetup(); fire('pointerdown', { pointerType: 'touch' }); vi.advanceTimersByTime(500);
    expect(surface.state.selection.main.from).toBe(0); expect(surface.state.selection.main.to).toBe(5); expect(capture.size).toBe(1);
    expect(onHandles).toHaveBeenLastCalledWith([{ pos: 0, end: false }, { pos: 5, end: true }]); fire('pointerup', { pointerType: 'touch' });
    fire('pointerdown', { pointerType: 'touch', clientX: 50, clientY: 20 }); fire('pointermove', { pointerType: 'touch', clientX: 80, clientY: 20 });
    expect(surface.state.selection.main.to).toBe(8); fire('pointercancel'); expect(capture.size).toBe(0);
  });
  it('touch cancellation prevents delayed word selection and tap places caret', () => {
    const { surface, fire, capture } = pointerSetup(); fire('pointerdown', { pointerType: 'touch' }); fire('pointercancel'); vi.advanceTimersByTime(600);
    expect(surface.state.selection.main.head).toBe(0); fire('pointerdown', { pointerType: 'touch' }); fire('pointerup', { pointerType: 'touch' });
    expect(surface.state.selection.main.head).toBe(3); expect(capture.size).toBe(0);
  });
  it('offers numeric first refusal only to eligible mouse gestures and ends it on cancel', () => {
    const { surface, fire, numericDrag } = pointerSetup(); const move = vi.fn(), end = vi.fn(); numericDrag.mockReturnValue({ move, end });
    fire('pointerdown'); fire('pointermove'); expect(move).toHaveBeenCalledOnce(); expect(surface.state.selection.main.head).toBe(0);
    fire('pointercancel'); expect(end).toHaveBeenCalledWith(true); numericDrag.mockClear();
    fire('pointerdown', { pointerType: 'touch' }); fire('pointercancel'); fire('pointerdown', { shiftKey: true }); fire('pointerup');
    expect(numericDrag).not.toHaveBeenCalled();
  });
  it('autoscrolls an active selection and disposal cancels frame/capture work', () => {
    const { fire, scrollBy, pointer, capture } = pointerSetup(); fire('pointerdown'); fire('pointermove', { clientX: 120, clientY: 60 }); vi.advanceTimersByTime(32);
    expect(scrollBy).toHaveBeenCalledWith(12, 12); pointer.dispose(); const count = scrollBy.mock.calls.length; vi.advanceTimersByTime(100);
    expect(scrollBy).toHaveBeenCalledTimes(count); expect(capture.size).toBe(0);
  });
  it('preserves selection on resize/orientation and suppresses pointer editing during composition', () => {
    const { fire, surface, el } = pointerSetup(); surface.dispatch({ selection: { anchor: 1, head: 5 } });
    window.dispatchEvent(new Event('resize')); window.dispatchEvent(new Event('orientationchange')); expect(surface.state.selection.main.to).toBe(5);
    composition(el, 'compositionstart'); fire('pointerdown'); expect(surface.state.selection.main.from).toBe(1); expect(surface.state.selection.main.to).toBe(5);
  });
});
