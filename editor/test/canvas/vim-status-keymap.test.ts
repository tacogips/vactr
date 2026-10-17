// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { mountVimStatus, type VimStatusHandlers } from '../../src/code/vim-status-view';
import type { VimSnapshot } from '../../src/code/vim/types';

const disposers: (() => void)[] = [];
function setup(prompt: VimSnapshot['prompt'] = { kind: ':', initial: 'help' }, openKeymap = false, resolver?: (input: { key: string; code?: string; ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean }) => string | null) {
  const parent = document.createElement('div'); document.body.append(parent);
  const [snapshot] = createSignal<VimSnapshot>({ mode: 'normal', label: 'NORMAL', pending: '', recording: null, search: null, message: null, messageKind: null, prompt, cursorShape: 'block', displayCursor: 0, displayCursorEnd: 0 });
  const handlers: VimStatusHandlers = { submit: vi.fn(), cancel: vi.fn(), tapMode: vi.fn(), ...(openKeymap ? { openKeymap: vi.fn() } : {}) };
  disposers.push(mountVimStatus(parent, snapshot, handlers, resolver));
  return { parent, handlers };
}
function key(parent: HTMLElement, name: string, options: KeyboardEventInit = {}) {
  const event = new KeyboardEvent('keydown', { key: name, bubbles: true, cancelable: true, ...options });
  parent.querySelector<HTMLInputElement>('.vact-vim-ex')!.dispatchEvent(event); return event;
}

afterEach(() => { for (const dispose of disposers.splice(0)) dispose(); document.body.replaceChildren(); });

describe('Vim status keymap seam', () => {
  it('renders and invokes the optional keymap button, and omits it for old callers', () => {
    const enabled = setup(null, true);
    enabled.parent.querySelector<HTMLButtonElement>('.vact-vim-keymap-button')!.click();
    expect(enabled.handlers.openKeymap).toHaveBeenCalledOnce();
    const legacy = setup(null);
    expect(legacy.parent.querySelector('.vact-vim-keymap-button')).toBeNull();
  });

  it('maps Ctrl-g to cancel and consumes the mapped event', () => {
    const view = setup({ kind: ':', initial: '' }, false, (input) => input.ctrlKey && input.key.toLowerCase() === 'g' ? '<Esc>' : null);
    const event = key(view.parent, 'g', { ctrlKey: true });
    expect(view.handlers.cancel).toHaveBeenCalledOnce();
    expect(event.defaultPrevented).toBe(true);
  });

  it('maps Ctrl-a to Home in the ex input', () => {
    const view = setup({ kind: ':', initial: 'help' }, false, (input) => input.ctrlKey && input.key.toLowerCase() === 'a' ? '<Home>' : null);
    const input = view.parent.querySelector<HTMLInputElement>('.vact-vim-ex')!; input.setSelectionRange(4, 4);
    key(view.parent, 'a', { ctrlKey: true });
    expect(input.selectionStart).toBe(0);
    expect(input.selectionEnd).toBe(0);
  });

  it('keeps the existing Enter behavior when the resolver returns null', () => {
    const view = setup({ kind: ':', initial: 'help' }, false, () => null);
    const event = key(view.parent, 'Enter');
    expect(view.handlers.submit).toHaveBeenCalledWith('help');
    expect(event.defaultPrevented).toBe(true);
  });

  it('applies mapped caret movement, deletion and printable insertion', () => {
    const lookup: Record<string, string> = { ArrowLeft: '<Left>', Backspace: '<BS>', Delete: '<Del>', x: 'X' };
    const view = setup({ kind: ':', initial: 'abcd' }, false, (input) => lookup[input.key] ?? null);
    const input = view.parent.querySelector<HTMLInputElement>('.vact-vim-ex')!;
    input.setSelectionRange(2, 2); key(view.parent, 'ArrowLeft'); expect(input.selectionStart).toBe(1);
    key(view.parent, 'Backspace'); expect(input.value).toBe('bcd');
    key(view.parent, 'Delete'); expect(input.value).toBe('cd');
    key(view.parent, 'x'); expect(input.value).toBe('Xcd');
  });

  it('submits when the resolver maps a key to CR', () => {
    const view = setup({ kind: ':', initial: 'write' }, false, (input) => input.key === 's' ? '<CR>' : null);
    key(view.parent, 's');
    expect(view.handlers.submit).toHaveBeenCalledWith('write');
  });

  it('does not resolve mapped keys during IME composition', () => {
    const resolver = vi.fn(() => '<Esc>'); const view = setup({ kind: ':', initial: 'x' }, false, resolver);
    const event = new KeyboardEvent('keydown', { key: 'g', isComposing: true, bubbles: true, cancelable: true });
    view.parent.querySelector('.vact-vim-ex')!.dispatchEvent(event);
    expect(resolver).not.toHaveBeenCalled();
    expect(view.handlers.cancel).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(false);
  });
});
