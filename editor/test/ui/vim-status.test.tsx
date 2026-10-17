import { createSignal } from 'solid-js';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { mountVimStatus, type VimStatusHandlers } from '../../src/code/vim-status-view';
import type { VimSnapshot } from '../../src/code/vim/types';

const base = (): VimSnapshot => ({ mode: 'normal', label: 'NORMAL', pending: '', recording: null, search: null, message: null, messageKind: null, prompt: null, cursorShape: 'block', displayCursor: 0, displayCursorEnd: 1 });
const cleanup: (() => void)[] = [];
afterEach(() => { for (const dispose of cleanup.splice(0)) dispose(); document.body.innerHTML = ''; });

function mount(initial = base(), handlers: Partial<VimStatusHandlers> = {}) {
  const parent = document.createElement('div');
  document.body.appendChild(parent);
  const [snapshot, setSnapshot] = createSignal(initial);
  const props: VimStatusHandlers = { submit: vi.fn(), cancel: vi.fn(), tapMode: vi.fn(), ...handlers };
  const dispose = mountVimStatus(parent, snapshot, props);
  cleanup.push(dispose);
  return { parent, snapshot, setSnapshot, props };
}

describe('Vim status view', () => {
  it('renders and reactively updates the mode label', () => {
    const { parent, setSnapshot } = mount();
    expect(parent.querySelector('.vact-vim-status')?.getAttribute('data-mode')).toBe('normal');
    setSnapshot({ ...base(), mode: 'visual-line', label: 'VISUAL LINE' });
    expect(parent.querySelector('.vact-vim-mode')?.textContent).toBe('VISUAL LINE');
    expect(parent.querySelector('.vact-vim-status')?.getAttribute('data-mode')).toBe('visual-line');
  });
  it('shows and clears pending, recording and search state', () => {
    const { parent, setSnapshot } = mount();
    setSnapshot({ ...base(), pending: '"a2d', recording: 'a', search: 'foo' });
    expect(parent.querySelector('.vact-vim-pending')?.textContent).toBe('"a2d');
    expect(parent.querySelector('.vact-vim-recording')?.textContent).toBe('recording @a');
    expect(parent.querySelector('.vact-vim-search')?.textContent).toBe('/foo');
    setSnapshot(base());
    expect(parent.querySelector('.vact-vim-recording')).toBeNull();
    expect(parent.querySelector('.vact-vim-search')).toBeNull();
  });
  it('announces informational messages as status', () => {
    const { parent } = mount({ ...base(), message: 'ready', messageKind: 'info' });
    expect(parent.querySelector('.vact-vim-message')?.getAttribute('role')).toBe('status');
    expect(parent.querySelector('.vact-vim-message')?.getAttribute('data-kind')).toBe('info');
  });
  it('announces error messages assertively', () => {
    const { parent } = mount({ ...base(), message: 'bad command', messageKind: 'error' });
    expect(parent.querySelector('.vact-vim-message')?.getAttribute('role')).toBe('alert');
    expect(parent.querySelector('.vact-vim-message')?.textContent).toBe('bad command');
  });
  it('focuses prompt input and places caret at the end of its initial text', () => {
    const { parent } = mount({ ...base(), prompt: { kind: ':', initial: "'<,'>" } });
    const input = parent.querySelector<HTMLInputElement>('.vact-vim-ex');
    expect(document.activeElement).toBe(input);
    expect(input?.value).toBe("'<,'>");
    expect(input?.selectionStart).toBe(input?.value.length);
  });
  it('submits once on Enter and blur does not cancel afterward', () => {
    const { parent, props } = mount({ ...base(), prompt: { kind: ':', initial: 'w' } });
    const input = parent.querySelector<HTMLInputElement>('.vact-vim-ex')!;
    input.value = 'format';
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
    input.dispatchEvent(new FocusEvent('blur'));
    expect(props.submit).toHaveBeenCalledOnce();
    expect(props.submit).toHaveBeenCalledWith('format');
    expect(props.cancel).not.toHaveBeenCalled();
  });
  it('does not submit Enter during IME composition', () => {
    const { parent, props } = mount({ ...base(), prompt: { kind: '/', initial: 'あ' } });
    const event = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true });
    Object.defineProperty(event, 'isComposing', { value: true });
    parent.querySelector('.vact-vim-ex')?.dispatchEvent(event);
    expect(props.submit).not.toHaveBeenCalled();
  });
  it('does not submit Enter for the legacy composing key code', () => {
    const { parent, props } = mount({ ...base(), prompt: { kind: '/', initial: '' } });
    const event = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true });
    Object.defineProperty(event, 'keyCode', { value: 229 });
    parent.querySelector('.vact-vim-ex')?.dispatchEvent(event);
    expect(props.submit).not.toHaveBeenCalled();
  });
  it('cancels Escape or Ctrl-[ and handles only once', () => {
    const { parent, props } = mount({ ...base(), prompt: { kind: '/', initial: '' } });
    const input = parent.querySelector('.vact-vim-ex')!;
    input.dispatchEvent(new KeyboardEvent('keydown', { key: '[', ctrlKey: true, bubbles: true, cancelable: true }));
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    expect(props.cancel).toHaveBeenCalledOnce();
  });
  it('the mode button calls tapMode', () => {
    const { parent, props } = mount();
    parent.querySelector<HTMLButtonElement>('.vact-vim-mode')?.click();
    expect(props.tapMode).toHaveBeenCalledOnce();
  });
  it('disposal removes the status row', () => {
    const { parent } = mount();
    cleanup.pop()?.();
    expect(parent.querySelector('.vact-vim-status')).toBeNull();
  });
});
