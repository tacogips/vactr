import { Text, Transaction } from '@codemirror/state';
import { describe, expect, it, vi } from 'vitest';
import { CodeSurface } from '../../../src/code/surface';
import { DocumentSync } from '../../../src/code/sync';
import { InputController } from '../../../src/code/input';
import { VimController } from '../../../src/code/vim/controller';
import type { VimKeyInput, VimMode } from '../../../src/code/vim/types';
import { Client } from '../../../src/protocol/client';
import { Store } from '../../../src/protocol/store';
import { createTestHost } from './host';
import { RecordingTransport } from '../../support/recording';

const event = (key: string, patch: Partial<VimKeyInput> = {}): VimKeyInput => ({ key, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...patch });
const typeInto = (host: ReturnType<typeof createTestHost>, text: string) => { for (const char of text) { const pos = host.state().selection.main.head; host.dispatch({ changes: { from: pos, insert: char }, selection: { anchor: pos + 1 }, annotations: Transaction.userEvent.of('input.type') }); } };
const enterMode = (vim: VimController, mode: VimMode) => {
  if (mode === 'insert') vim.handleKey(event('i'));
  if (mode === 'replace') vim.handleKey(event('R'));
  if (mode === 'visual') vim.handleKey(event('v'));
  if (mode === 'visual-line') vim.handleKey(event('V'));
  if (mode === 'operator-pending') vim.handleKey(event('d'));
};

describe('VimController input and shortcut pass-through', () => {
  const passthrough = [
    ['Enter', { ctrlKey: true }], ['Enter', { metaKey: true }], ['Enter', { ctrlKey: true, shiftKey: true }],
    ['.', { ctrlKey: true }], ['a', { ctrlKey: true }], ['z', { ctrlKey: true }], ['y', { ctrlKey: true }],
    ['c', { ctrlKey: true }], ['v', { ctrlKey: true }], ['Enter', { ctrlKey: true, altKey: true }],
    ['.', { metaKey: true }], ['a', { metaKey: true }], ['z', { metaKey: true }],
    ['c', { metaKey: true }], ['x', { metaKey: true }], ['v', { metaKey: true }], ['\\', { ctrlKey: true }],
  ] as const;
  const modePlatform = (['other', 'apple'] as const).flatMap((platform) =>
    (['normal', 'visual', 'visual-line', 'operator-pending', 'insert', 'replace'] as const).map((mode) => ({ platform, mode })));
  it.each(modePlatform)('passes live shortcuts through in $mode mode on $platform', ({ platform, mode }) => {
    for (const [key, modifiers] of passthrough) {
      const host = createTestHost('abc|', { platform }), vim = new VimController(host);
      enterMode(vim, mode);
      expect(vim.handleKey(event(key, modifiers))).toBe(false);
    }
  });
  it.each(['normal', 'visual', 'visual-line', 'operator-pending'] as const)('consumes modified delete and tab keys without editing in %s mode', (mode) => {
    for (const chord of [event('Backspace', { altKey: true }), event('Backspace', { metaKey: true }), event('Backspace', { ctrlKey: true }), event('Delete', { altKey: true }), event('Delete', { metaKey: true }), event('Delete', { ctrlKey: true }), event('Tab', { shiftKey: true }), event('Tab', { altKey: true })]) {
      const host = createTestHost('abc|'), vim = new VimController(host); enterMode(vim, mode);
      const before = host.state().doc.toString();
      expect(vim.handleKey(chord)).toBe(true);
      expect(host.state().doc.toString()).toBe(before);
    }
  });
  it('preserves operator pending across pass-through shortcuts and default keymaps', () => {
    const host = createTestHost('|abc', { defaultKeymaps: (input) => input.key === 'f' && input.altKey });
    const vim = new VimController(host); vim.handleKey(event('d'));
    expect(vim.handleKey(event('Enter', { ctrlKey: true }))).toBe(false);
    expect(vim.snapshot.pending).toBe('d');
    expect(vim.handleKey(event('f', { altKey: true, shiftKey: true }))).toBe(false);
    expect(vim.snapshot.pending).toBe('d');
    vim.handleKey(event('w'));
    expect(host.state().doc.toString()).toBe('');
  });
  it('routes Mod-Enter to default keymaps and consumes modified deletion through InputController', () => {
    const originalPlatform = navigator.platform;
    try {
      for (const mode of ['normal', 'visual', 'visual-line', 'operator-pending'] as const) {
        for (const platform of ['Linux x86_64', 'MacIntel']) {
        Object.defineProperty(navigator, 'platform', { value: platform, configurable: true });
        const transport = new RecordingTransport();
        const client = new Client(transport, { store: new Store(), now: () => Date.now() });
        const surface = new CodeSurface({ sync: new DocumentSync(client.document('main.vact'), Text.of(['abc'])) });
        const container = document.createElement('div'); document.body.append(container);
        const evalSelection = vi.fn();
        const input = new InputController(surface, container, { evalSelection });
        const host = createTestHost('|abc', { platform: platform.startsWith('Mac') ? 'apple' : 'other' });
        host.matchesDefaultKeymap = (key) => surface.matchesKeymap({ ...key, code: key.code ?? '' }, 'default');
        const vim = new VimController(host); enterMode(vim, mode); input.setVim(vim);
        const removeKeymap = surface.addKeymap([{ key: 'Mod-Enter', run: () => { evalSelection(); return true; } }], 'default');
        try {
          const handleKeyboard = vi.spyOn(input.keyboard, 'handle');
          const isApple = platform.startsWith('Mac');
          const enter = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true, [isApple ? 'metaKey' : 'ctrlKey']: true });
          input.accessibility.textarea.dispatchEvent(enter);
          expect(evalSelection).toHaveBeenCalledOnce();
          expect(enter.defaultPrevented).toBe(true);
          if (mode === 'operator-pending') expect(vim.snapshot.pending).toBe('d');
          const before = surface.state.doc.toString();
          const backspace = new KeyboardEvent('keydown', { key: 'Backspace', ctrlKey: true, bubbles: true, cancelable: true });
          input.accessibility.textarea.dispatchEvent(backspace);
          expect(backspace.defaultPrevented).toBe(true);
          expect(surface.state.doc.toString()).toBe(before);
          expect(handleKeyboard).not.toHaveBeenCalled();
        } finally {
          removeKeymap(); vim.dispose(); input.dispose(); surface.dispose(); container.remove();
        }
        }
      }
    } finally {
      Object.defineProperty(navigator, 'platform', { value: originalPlatform, configurable: true });
    }
  });
  it('gates composition, prompt, text input and replace overwrite', () => {
    const host = createTestHost('abc|'), vim = new VimController(host);
    expect(vim.allowsTextInput()).toBe(false); vim.handleKey(event('i')); expect(vim.allowsTextInput()).toBe(true);
    vim.handleKey(event('Escape')); vim.handleKey(event('R')); expect(vim.overwriteRange('x')).toEqual({ from: 2, to: 3 });
    expect(vim.allowsTextInput()).toBe(true); vim.handleKey(event('Escape')); vim.handleKey(event('/'));
    expect(vim.allowsTextInput()).toBe(false); vim.cancelPrompt(); vim.dispose();
  });
  it('dispatches search targets and messages', () => {
    const host = createTestHost('foo bar foo|'), vim = new VimController(host);
    vim.handleKey(event('/')); vim.submitPrompt('foo');
    expect(host.state().selection.main.head).toBe(0);
    vim.handleKey(event('n')); expect(host.state().selection.main.head).toBe(8);
    vim.handleKey(event('n')); expect(vim.snapshot.message).toContain('search hit BOTTOM');
  });
  it('matches interactive search anchors at each line boundary', () => {
    const host = createTestHost('foo\nbar\nbaz|'), vim = new VimController(host);
    vim.handleKey(event('/')); vim.submitPrompt('^bar');
    expect(host.state().selection.main.head).toBe(4);
  });
  it('repeats a resolved asynchronous clipboard paste exactly once', async () => {
    const host = createTestHost('|abc'), vim = new VimController(host);
    host.memoryClipboard.value = 'x';
    vim.handleKey(event('"')); vim.handleKey(event('+')); vim.handleKey(event('p'));
    await Promise.resolve();
    expect(host.state().doc.toString()).toBe('axbc');
    vim.handleKey(event('.'));
    expect(host.state().doc.toString()).toBe('axxbc');
  });
  it('Ctrl-[ closes an open popup and leaves insert mode untouched', () => {
    const host = createTestHost('|abc'), vim = new VimController(host); vim.handleKey(event('i'));
    host.closePopupWithEscape = vi.fn(() => true);
    expect(vim.handleKey(event('[', { ctrlKey: true }))).toBe(true);
    expect(vim.snapshot.mode).toBe('insert'); expect(host.state().doc.toString()).toBe('abc');
  });
  it('Ctrl-[ without a popup exits insert mode', () => {
    const host = createTestHost('|abc'), vim = new VimController(host); vim.handleKey(event('i'));
    expect(vim.handleKey(event('[', { ctrlKey: true }))).toBe(true);
    expect(vim.snapshot.mode).toBe('normal'); expect(host.state().doc.toString()).toBe('abc');
  });
  it('consumes unmodified Tab and Q in normal mode without edits', () => {
    const host = createTestHost('|abc'), vim = new VimController(host);
    expect(vim.handleKey(event('Tab'))).toBe(true); expect(vim.handleKey(event('Q'))).toBe(true);
    expect(host.state().doc.toString()).toBe('abc'); expect(host.state().selection.main.head).toBe(0);
  });
  it('restores overwritten text with replace-mode Backspace', () => {
    const host = createTestHost('|abc'), vim = new VimController(host); vim.handleKey(event('R'));
    const range = vim.overwriteRange('X')!;
    host.dispatch({ changes: { from: range.from, to: range.to, insert: 'X' }, selection: { anchor: range.from + 1 }, annotations: Transaction.userEvent.of('input.type') });
    expect(host.state().doc.toString()).toBe('Xbc'); vim.handleKey(event('Backspace'));
    expect(host.state().doc.toString()).toBe('abc'); expect(host.state().selection.main.head).toBe(1);
  });
  it('searches backward with ? and N, including wrap feedback', () => {
    const host = createTestHost('foo bar foo|'), vim = new VimController(host);
    vim.handleKey(event('?')); vim.submitPrompt('foo'); expect(host.state().selection.main.head).toBe(8);
    vim.handleKey(event('N')); expect(host.state().selection.main.head).toBe(0);
    expect(host.messages.some(({ message }) => message.includes('search hit'))).toBe(true);
  });
  it('reports E486 when a search has no match', () => {
    const host = createTestHost('|abc'), vim = new VimController(host);
    vim.handleKey(event('/')); vim.submitPrompt('z');
    expect(vim.snapshot.message).toContain('E486'); expect(host.state().doc.toString()).toBe('abc');
  });
  it('repeats word-under-cursor searches through * and #', () => {
    const host = createTestHost('|foo x foo'), vim = new VimController(host);
    vim.handleKey(event('*')); expect(host.state().selection.main.head).toBe(6);
    vim.handleKey(event('#')); expect(host.state().selection.main.head).toBe(0);
    expect(host.state().doc.toString()).toBe('foo x foo');
  });
  it('navigates with :3 and substitutes as one undo unit', () => {
    const host = createTestHost('a a\nb|'), vim = new VimController(host);
    vim.handleKey(event(':')); vim.submitPrompt('3'); expect(host.state().selection.main.head).toBe(4);
    vim.handleKey(event(':')); vim.submitPrompt('%s/a/b/g'); expect(host.state().doc.toString()).toBe('b b\nb');
    vim.handleKey(event('u')); expect(host.state().doc.toString()).toBe('a a\nb');
  });
  it('applies visual-range substitution and clears search with :noh', () => {
    const host = createTestHost('|a a\nb'), vim = new VimController(host);
    vim.handleKey(event('V')); vim.handleKey(event('j')); vim.handleKey(event(':')); vim.submitPrompt("'<,'>s/a/x/g");
    expect(host.state().doc.toString()).toBe('x x\nb');
    vim.handleKey(event('/')); vim.submitPrompt('x'); vim.handleKey(event(':')); vim.submitPrompt('noh');
    expect(vim.snapshot.search).toBe(null);
  });
  it('shows the live-sync message for :w and E492 for unknown ex commands', () => {
    const host = createTestHost('|abc'), vim = new VimController(host);
    vim.handleKey(event(':')); vim.submitPrompt('w'); expect(vim.snapshot.message).toContain('vactr syncs');
    vim.handleKey(event(':')); vim.submitPrompt('not-a-command'); expect(vim.snapshot.message).toContain('E492');
    expect(host.state().doc.toString()).toBe('abc');
  });
  it('dispatches eval, evalall, hush and format ex actions', () => {
    const host = createTestHost('|abc'), vim = new VimController(host);
    for (const [command, action] of [['eval', 'evalAtCursor'], ['evalall', 'evalAll'], ['hush', 'hush'], ['format', 'format']] as const) {
      vim.handleKey(event(':')); vim.submitPrompt(command); expect(host.actions[action]).toHaveBeenCalledOnce();
    }
    expect(host.state().doc.toString()).toBe('abc');
  });
  it('writes a linewise + yank to the clipboard', async () => {
    const host = createTestHost('|one\ntwo'), vim = new VimController(host);
    vim.handleKey(event('"')); vim.handleKey(event('+')); vim.handleKey(event('y')); vim.handleKey(event('y'));
    await Promise.resolve(); expect(host.memoryClipboard.value).toBe('one'); expect(host.state().doc.toString()).toBe('one\ntwo');
  });
  it('reports clipboard write rejection while retaining the document', async () => {
    const host = createTestHost('|one'); host.memoryClipboard.fail = true; const vim = new VimController(host);
    vim.handleKey(event('"')); vim.handleKey(event('+')); vim.handleKey(event('y')); vim.handleKey(event('y')); await Promise.resolve();
    expect(vim.snapshot.message).toContain('clipboard unavailable'); expect(host.state().doc.toString()).toBe('one');
  });
  it('drops a clipboard paste when the document changes before resolution', async () => {
    const host = createTestHost('|abc'), vim = new VimController(host); host.memoryClipboard.value = 'X';
    vim.handleKey(event('"')); vim.handleKey(event('+')); vim.handleKey(event('p'));
    host.dispatch({ changes: { from: 0, insert: 'z' } }); await Promise.resolve();
    expect(host.state().doc.toString()).toBe('zabc'); expect(vim.snapshot.message).toContain('clipboard paste dropped');
  });
  it('falls back to the internal clipboard mirror after read rejection', async () => {
    const host = createTestHost('|abc'), vim = new VimController(host);
    vim.handleKey(event('"')); vim.handleKey(event('+')); vim.handleKey(event('y')); vim.handleKey(event('y')); await Promise.resolve();
    host.memoryClipboard.fail = true; vim.handleKey(event('"')); vim.handleKey(event('+')); vim.handleKey(event('p')); await Promise.resolve(); await Promise.resolve();
    expect(host.state().doc.toString()).toBe('abc\nabc'); expect(host.messages.at(-1)?.message).toContain('clipboard unavailable');
  });
  it('exposes pending register/count state and cursor shape per mode', () => {
    const host = createTestHost('abc|'), vim = new VimController(host);
    expect(vim.snapshot.label).toBe('NORMAL'); expect(vim.snapshot.displayCursor).toBe(2); expect(vim.snapshot.displayCursorEnd).toBe(3);
    vim.handleKey(event('"')); vim.handleKey(event('a')); vim.handleKey(event('2')); vim.handleKey(event('d'));
    expect(vim.snapshot.pending).toBe('"a2d'); expect(vim.snapshot.cursorShape).toBe('underline');
    vim.handleKey(event('Escape')); vim.handleKey(event('v')); expect(vim.snapshot.label).toBe('VISUAL'); vim.handleKey(event('Escape'));
    vim.handleKey(event('V')); expect(vim.snapshot.label).toBe('VISUAL LINE'); vim.handleKey(event('Escape'));
    vim.handleKey(event('i')); expect(vim.snapshot.label).toBe('INSERT'); expect(vim.snapshot.cursorShape).toBe('bar');
    vim.handleKey(event('Escape')); vim.handleKey(event('R')); expect(vim.snapshot.label).toBe('REPLACE'); expect(vim.snapshot.cursorShape).toBe('underline');
  });
  it('tapMode enters insert then exits to normal with document changes grouped', () => {
    const host = createTestHost('|abc'), vim = new VimController(host);
    vim.tapMode(); typeInto(host, 'x'); expect(vim.snapshot.mode).toBe('insert');
    vim.tapMode(); expect(host.state().doc.toString()).toBe('xabc'); expect(vim.snapshot.mode).toBe('normal');
  });
  it('external nonempty selection enters visual without reentrant dispatch', () => {
    const host = createTestHost('|abcd'), vim = new VimController(host);
    host.dispatch({ selection: { anchor: 1, head: 3 } });
    expect(vim.snapshot.mode).toBe('visual'); expect(host.state().doc.toString()).toBe('abcd');
    expect(host.state().selection.main.head).toBe(3);
  });
  it('announces discarded normal-mode composition input', () => {
    const host = createTestHost('|abc'), vim = new VimController(host); vim.noteCompositionDiscarded();
    expect(vim.snapshot.message).toContain('IME input ignored'); expect(host.state().doc.toString()).toBe('abc');
  });
  it('calls scrollLines for Ctrl-d, Ctrl-u, Ctrl-f and Ctrl-b', () => {
    const host = createTestHost('|a\nb\nc\nd', { viewport: { topLine: 1, rows: 4 } }), vim = new VimController(host);
    for (const [key, amount] of [['d', 2], ['u', -2], ['f', 2], ['b', -2]] as const) {
      vim.handleKey(event(key, { ctrlKey: true })); expect(host.scrolls.at(-1)).toBe(amount);
    }
    expect(host.state().doc.toString()).toBe('a\nb\nc\nd');
  });
});
