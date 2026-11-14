// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import { Text } from '@codemirror/state';
import { InputController } from '../../src/code/input';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import type { VimInputHook } from '../../src/code/vim/types';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { RecordingTransport } from '../support/recording';

const cleanup: (() => void)[] = [];
function setup(text = '') {
  const transport = new RecordingTransport();
  const client = new Client(transport, { store: new Store(), now: () => Date.now() });
  const sync = new DocumentSync(client.document('main.vact'), Text.of(text.split('\n')));
  const surface = new CodeSurface({ sync });
  const container = document.createElement('div'); document.body.append(container);
  const input = new InputController(surface, container);
  const hook: VimInputHook = {
    handleKey: vi.fn(() => false), allowsTextInput: vi.fn(() => true),
    overwriteRange: vi.fn(() => null), noteCompositionDiscarded: vi.fn(),
  };
  cleanup.push(() => { input.dispose(); surface.dispose(); client.close(); container.remove(); });
  return { surface, sync, input, el: input.accessibility.textarea, hook };
}
function composition(el: HTMLElement, type: string, data = ''): void {
  el.dispatchEvent(new CompositionEvent(type, { data, bubbles: true }));
}
function before(el: HTMLElement, inputType: string, data: string | null = null, isComposing = false): InputEvent {
  const event = new InputEvent('beforeinput', { inputType, data, isComposing, bubbles: true, cancelable: true });
  el.dispatchEvent(event); return event;
}
function key(el: HTMLElement, value: string): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key: value, bubbles: true, cancelable: true });
  el.dispatchEvent(event); return event;
}
afterEach(() => { for (const dispose of cleanup.splice(0).reverse()) dispose(); });

describe('InputController keymap hooks', () => {
  it('typeText inserts text and records the input.type event', () => {
    const { surface, input } = setup('ab');
    surface.dispatch({ selection: { anchor: 1 } });
    const dispatch = vi.spyOn(surface, 'dispatch');
    input.typeText('k');
    expect(surface.state.doc.toString()).toBe('akb');
    expect(dispatch).toHaveBeenCalledWith(expect.objectContaining({ userEvent: 'input.type' }));
  });

  it('typeText uses the Vim overwrite range when provided', () => {
    const { surface, input, hook } = setup('abc');
    vi.mocked(hook.overwriteRange).mockReturnValue({ from: 1, to: 2 }); input.setVim(hook);
    input.typeText('X');
    expect(surface.state.doc.toString()).toBe('aXc');
    expect(hook.overwriteRange).toHaveBeenCalledWith('X');
  });

  it('typeText does nothing during composition', () => {
    const { surface, input } = setup('ab');
    input.beginComposition(); input.typeText('X');
    expect(surface.state.doc.toString()).toBe('ab');
    expect(input.isComposing).toBe(true);
  });

  it('defaultKey maps Backspace and Enter through KeyboardController', () => {
    const { surface, input } = setup('ab');
    surface.dispatch({ selection: { anchor: 2 } });
    expect(input.defaultKey('<BS>')).toBe(true);
    expect(surface.state.doc.toString()).toBe('a');
    expect(input.defaultKey('<CR>')).toBe(true);
    expect(surface.state.doc.toString()).toBe('a\n');
  });

  it('defaultKey returns false for unsupported keys without changing the document', () => {
    const { surface, input } = setup('ab');
    expect(input.defaultKey('<F1>')).toBe(false);
    expect(surface.state.doc.toString()).toBe('ab');
  });

  it('defaultKey runs default surface keymaps before the keyboard fallback', () => {
    const { surface, input } = setup('ab');
    const run = vi.fn(() => true); surface.addKeymap([{ key: 'Enter', run }], 'default');
    expect(input.defaultKey('<CR>')).toBe(true);
    expect(run).toHaveBeenCalledOnce();
    expect(surface.state.doc.toString()).toBe('ab');
  });

  it('flushes pending input before composition state starts', () => {
    const { input, el, hook } = setup(); input.setVim(hook);
    const flush = vi.fn(() => expect(input.isComposing).toBe(false)); hook.flushPendingInput = flush;
    composition(el, 'compositionstart');
    expect(flush).toHaveBeenCalledOnce();
    expect(input.isComposing).toBe(true);
  });

  it('flushes pending input before beforeinput text is handled', () => {
    const { surface, input, el, hook } = setup(); input.setVim(hook);
    hook.flushPendingInput = () => input.typeText('k');
    before(el, 'insertText', 'x');
    expect(surface.state.doc.toString()).toBe('kx');
  });

  it('flushes pending input before fallback input reconciliation', () => {
    const { surface, input, el, hook } = setup('ab'); input.setVim(hook);
    surface.dispatch({ selection: { anchor: 1 } }); input.accessibility.refresh();
    hook.flushPendingInput = () => input.typeText('k');
    el.setRangeText('x', 1, 1, 'end');
    el.dispatchEvent(new InputEvent('input', { inputType: 'insertText', data: 'x', bubbles: true }));
    expect(surface.state.doc.toString()).toBe('akxb');
  });

  it('does not flush while an input event is composing', () => {
    const { input, el, hook } = setup(); input.setVim(hook);
    const flush = vi.fn(); hook.flushPendingInput = flush;
    el.dispatchEvent(new InputEvent('input', { inputType: 'insertCompositionText', data: 'に', isComposing: true, bubbles: true }));
    expect(flush).not.toHaveBeenCalled();
  });

  it('does not flush pending input for a plain keydown', () => {
    const { input, el, hook } = setup(); input.setVim(hook);
    const flush = vi.fn(); hook.flushPendingInput = flush;
    key(el, 'z');
    expect(flush).not.toHaveBeenCalled();
  });

  it('flushes deferred text before committing Japanese composition exactly once', () => {
    const { surface, sync, input, el, hook } = setup(); input.setVim(hook);
    hook.flushPendingInput = () => input.typeText('k');
    const revision = sync.revision;
    composition(el, 'compositionstart'); composition(el, 'compositionupdate', 'に');
    composition(el, 'compositionend', '日本');
    before(el, 'insertFromComposition', '日本', true);
    expect(surface.state.doc.toString()).toBe('k日本');
    expect(sync.revision).toBe(revision + 2);
  });
});
