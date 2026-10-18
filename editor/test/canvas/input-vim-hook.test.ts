import { Text } from '@codemirror/state';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { InputController, type InputPresentation } from '../../src/code/input';
import type { VimInputHook } from '../../src/code/vim/types';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { RecordingTransport } from '../support/recording';

const cleanup: (() => void)[] = [];
function setup(text = '', allow = true, consume = false, range: { from: number; to: number } | null = null,
  onPresentation?: (presentation: InputPresentation) => void) {
  const transport = new RecordingTransport();
  const client = new Client(transport, { store: new Store(), now: () => Date.now() });
  const sync = new DocumentSync(client.document('main.vact'), Text.of(text.split('\n')));
  const surface = new CodeSurface({ sync });
  const container = document.createElement('div'); document.body.append(container);
  const evalSelection = vi.fn(), evalAll = vi.fn(), stopAll = vi.fn(), cut = vi.fn(), scrollCaret = vi.fn();
  const input = new InputController(surface, container, { evalSelection, evalAll, stopAll, cut, scrollCaret, onPresentation });
  const hook: VimInputHook = {
    handleKey: vi.fn(() => consume), allowsTextInput: vi.fn(() => allow),
    overwriteRange: vi.fn(() => range), noteCompositionDiscarded: vi.fn(),
  };
  cleanup.push(() => { input.dispose(); surface.dispose(); container.remove(); });
  return { surface, sync, input, container, el: input.accessibility.textarea, transport, evalSelection, evalAll, stopAll, cut, scrollCaret, hook };
}
function composition(el: HTMLElement, kind: string, data = '') { el.dispatchEvent(new CompositionEvent(kind, { data, bubbles: true })); }
function before(el: HTMLElement, inputType: string, data: string | null = null, isComposing = false, cancelable = true) {
  const event = new InputEvent('beforeinput', { inputType, data, isComposing, bubbles: true, cancelable }); el.dispatchEvent(event); return event;
}
function key(el: HTMLElement, k: string, options: KeyboardEventInit = {}) {
  const event = new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true, ...options }); el.dispatchEvent(event); return event;
}
const originalPlatform = navigator.platform;
afterEach(() => {
  for (const fn of cleanup.splice(0).reverse()) fn();
  Object.defineProperty(navigator, 'platform', { value: originalPlatform, configurable: true });
});

describe('Vim input hook', () => {
  it('keeps ordinary insertText behavior when no hook is installed', () => {
    const { surface, el } = setup(); before(el, 'insertText', 'x');
    expect(surface.state.doc.toString()).toBe('x'); expect(surface.state.selection.main.head).toBe(1);
  });
  it('keeps ordinary history shortcuts when no hook is installed', () => {
    const { surface, el } = setup(); before(el, 'insertText', 'x'); key(el, 'z', { ctrlKey: true });
    expect(surface.state.doc.toString()).toBe(''); key(el, 'z', { ctrlKey: true, shiftKey: true });
    expect(surface.state.doc.toString()).toBe('x');
  });
  it('keeps Tab insertion when no hook is installed', () => {
    const { surface, el } = setup(); key(el, 'Tab'); expect(surface.state.doc.toString()).toBe('\t');
  });
  it('keeps Japanese composition commit and one undo group when no hook is installed', () => {
    const { surface, sync, el } = setup('abc'); surface.dispatch({ selection: { anchor: 1, head: 2 } });
    composition(el, 'compositionstart'); composition(el, 'compositionupdate', 'に');
    before(el, 'insertCompositionText', '日本', true); composition(el, 'compositionend', '日本');
    before(el, 'insertFromComposition', '日本', true);
    expect(surface.state.doc.toString()).toBe('a日本c'); expect(sync.revision).toBe(2);
    expect(surface.retentionStatus.undoDepth).toBe(1); surface.undo(); expect(surface.state.doc.toString()).toBe('abc');
  });
  it('keeps Mod-Enter evaluation when no hook is installed', () => {
    const { el, evalSelection } = setup(); key(el, 'Enter', { ctrlKey: true }); expect(evalSelection).toHaveBeenCalledOnce();
  });
  it('consumes a Vim key before KeyboardController handles it', () => {
    const { surface, el, input, hook } = setup('abc', true, true); input.setVim(hook);
    const handle = vi.spyOn(input.keyboard, 'handle');
    const event = key(el, 'j'); expect(event.defaultPrevented).toBe(true);
    expect(surface.state.doc.toString()).toBe('abc'); expect(handle).not.toHaveBeenCalled();
  });
  it('prevents a consumed Tab from inserting text', () => {
    const { surface, el, input, hook } = setup('', true, true); input.setVim(hook); key(el, 'Tab');
    expect(surface.state.doc.toString()).toBe('');
  });
  it('passes a declined Ctrl-Enter to Linux default keymaps', () => {
    Object.defineProperty(navigator, 'platform', { value: 'Linux x86_64', configurable: true });
    const { surface, el, input, hook, evalSelection } = setup(); input.setVim(hook);
    surface.addKeymap([{ key: 'Mod-Enter', run: () => { evalSelection(); return true; } }], 'default');
    key(el, 'Enter', { ctrlKey: true }); expect(evalSelection).toHaveBeenCalledOnce();
  });
  it('passes a declined Meta-Enter to Mac default keymaps', () => {
    Object.defineProperty(navigator, 'platform', { value: 'MacIntel', configurable: true });
    const { surface, el, input, hook, evalSelection } = setup(); input.setVim(hook);
    surface.addKeymap([{ key: 'Mod-Enter', run: () => { evalSelection(); return true; } }], 'default');
    key(el, 'Enter', { metaKey: true }); expect(evalSelection).toHaveBeenCalledOnce();
  });
  it('runs highest precedence before the Vim hook', () => {
    const { surface, el, input, hook } = setup(); input.setVim(hook);
    surface.addKeymap([{ key: 'Escape', run: () => true }], 'highest');
    expect(key(el, 'Escape').defaultPrevented).toBe(true); expect(hook.handleKey).not.toHaveBeenCalled();
  });
  it('blocks insertText while the text gate is closed', () => {
    const { surface, el, input, hook } = setup('abc', false); input.setVim(hook);
    expect(before(el, 'insertText', 'x').defaultPrevented).toBe(true); expect(surface.state.doc.toString()).toBe('abc');
  });
  it('blocks backward deletion while the text gate is closed', () => {
    const { surface, el, input, hook } = setup('abc', false); input.setVim(hook);
    expect(before(el, 'deleteContentBackward').defaultPrevented).toBe(true); expect(surface.state.doc.toString()).toBe('abc');
  });
  it('keeps historyUndo available while the text gate is closed', () => {
    const { surface, el, input, hook } = setup(''); surface.dispatch({ changes: { from: 0, insert: 'x' }, userEvent: 'input.type' }); input.setVim(hook);
    const event = before(el, 'historyUndo'); expect(event.defaultPrevented).toBe(true); expect(surface.state.doc.toString()).toBe('');
  });
  it('ignores non-cancelable native mutations while the text gate is closed', () => {
    const { surface, el, input, hook } = setup('abc', false); input.setVim(hook);
    el.value = 'abcx'; el.dispatchEvent(new InputEvent('input', { inputType: 'insertText', data: 'x' }));
    expect(surface.state.doc.toString()).toBe('abc'); expect(el.value).toBe('abc');
  });
  it('discards a normal-mode composition and terminates the platform session', async () => {
    const { surface, sync, el, input, hook } = setup('abc', false); input.setVim(hook);
    const blur = vi.spyOn(el, 'blur'), focus = vi.spyOn(el, 'focus');
    composition(el, 'compositionstart'); composition(el, 'compositionupdate', 'に');
    await Promise.resolve(); expect(blur).toHaveBeenCalledOnce(); expect(focus).toHaveBeenCalledOnce();
    composition(el, 'compositionend', '日本');
    expect(surface.state.doc.toString()).toBe('abc'); expect(sync.revision).toBe(1);
    expect(input.presentation.annotations).toEqual([]); expect(hook.noteCompositionDiscarded).toHaveBeenCalledOnce();
  });
  it('continues discarding a composition if the text gate opens before compositionend', () => {
    const { surface, sync, el, input, hook } = setup('abc', false); input.setVim(hook);
    composition(el, 'compositionstart'); composition(el, 'compositionupdate', 'に');
    vi.mocked(hook.allowsTextInput).mockReturnValue(true); composition(el, 'compositionend', '日本');
    expect(surface.state.doc.toString()).toBe('abc'); expect(sync.revision).toBe(1); expect(input.presentation.annotations).toEqual([]);
  });
  it('keeps Chromium composition events discarded after blur/refocus and a gate change', async () => {
    const { surface, sync, el, input, hook } = setup('stable', false); input.setVim(hook);
    surface.dispatch({ selection: { anchor: 5 } });
    const revision = sync.revision;
    composition(el, 'compositionstart');
    await Promise.resolve();
    el.dispatchEvent(new FocusEvent('blur'));
    composition(el, 'compositionupdate', 'にほん');
    const composingInput = before(el, 'insertCompositionText', 'にほん', true);
    input.updateComposition('日本');
    expect(composingInput.defaultPrevented).toBe(true);
    expect(input.isComposing).toBe(false);
    expect(input.presentation.annotations).toEqual([]);
    vi.mocked(hook.allowsTextInput).mockReturnValue(true);
    composition(el, 'compositionend', '日本');
    before(el, 'insertFromComposition', '日本', true);
    expect(surface.state.doc.toString()).toBe('stable');
    expect(sync.revision).toBe(revision);
    expect(input.presentation.annotations).toEqual([]);
    expect(hook.noteCompositionDiscarded).toHaveBeenCalledOnce();
  });
  it('prevents trailing composition beforeinput after a discarded session', () => {
    const { el, input, hook } = setup('', false); input.setVim(hook);
    composition(el, 'compositionstart'); composition(el, 'compositionend', '語');
    expect(before(el, 'insertFromComposition', '語', true).defaultPrevented).toBe(true);
  });
  it('keeps Japanese composition commits when the gate is open', () => {
    const { surface, sync, el, input, hook } = setup('abc', true); input.setVim(hook); surface.dispatch({ selection: { anchor: 1, head: 2 } });
    composition(el, 'compositionstart'); composition(el, 'compositionupdate', 'に'); before(el, 'insertCompositionText', '日本', true);
    composition(el, 'compositionend', '日本'); before(el, 'insertFromComposition', '日本', true);
    expect(surface.state.doc.toString()).toBe('a日本c'); expect(sync.revision).toBe(2);
    expect(surface.retentionStatus.undoDepth).toBe(1);
  });
  it('overwrites the requested range for insertText and scrolls the caret', () => {
    const { surface, el, input, hook, scrollCaret } = setup('abc', true, false, { from: 0, to: 1 }); input.setVim(hook);
    before(el, 'insertText', 'x'); expect(surface.state.doc.toString()).toBe('xbc'); expect(surface.state.selection.main.head).toBe(1);
    expect(hook.overwriteRange).toHaveBeenCalledWith('x'); expect(scrollCaret).toHaveBeenCalledOnce();
  });
  it('does not ask for overwrite on a line break', () => {
    const { surface, el, input, hook } = setup('abc', true, false, { from: 0, to: 1 }); input.setVim(hook);
    before(el, 'insertLineBreak'); expect(hook.overwriteRange).not.toHaveBeenCalled(); expect(surface.state.doc.toString()).toBe('\nabc');
  });
  it('queries both keymap registries without running bindings', () => {
    const { surface } = setup(); const highest = vi.fn(() => true), fallback = vi.fn(() => true);
    surface.addKeymap([{ key: 'Escape', run: highest }], 'highest'); surface.addKeymap([{ key: 'Escape', run: fallback }], 'default');
    const event = new KeyboardEvent('keydown', { key: 'Escape', cancelable: true });
    expect(surface.matchesKeymap(event, 'highest')).toBe(true); expect(surface.matchesKeymap(event, 'default')).toBe(true);
    expect(highest).not.toHaveBeenCalled(); expect(fallback).not.toHaveBeenCalled();
  });
  it('filters runKeymaps to the requested default registry', () => {
    const { surface } = setup(); const highest = vi.fn(() => true), fallback = vi.fn(() => true);
    surface.addKeymap([{ key: 'Escape', run: highest }], 'highest'); surface.addKeymap([{ key: 'Escape', run: fallback }], 'default');
    expect(surface.runKeymaps(new KeyboardEvent('keydown', { key: 'Escape', cancelable: true }), 'default')).toBe(true);
    expect(highest).not.toHaveBeenCalled(); expect(fallback).toHaveBeenCalledOnce();
  });
});
