// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import { Text } from '@codemirror/state';
import { InputController } from '../../src/code/input';
import { TextLayout } from '../../src/code/layout';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { VimController } from '../../src/code/vim/controller';
import { createSurfaceHost } from '../../src/code/vim-mount';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { RecordingTransport } from '../support/recording';

const cleanup: (() => void)[] = [];
function setup(text = 'alpha beta\ngamma', vim = true) {
  const transport = new RecordingTransport();
  const client = new Client(transport, { store: new Store(), now: () => Date.now() });
  const sync = new DocumentSync(client.document('main.vact'), Text.of(text.split('\n')));
  const surface = new CodeSurface({ sync });
  const container = document.createElement('div'); document.body.append(container);
  const evalSelection = vi.fn(), evalAll = vi.fn(), stopAll = vi.fn(), cut = vi.fn(), scrollCaret = vi.fn();
  const input = new InputController(surface, container, { evalSelection, evalAll, stopAll, cut, scrollCaret });
  const scrollBy = vi.fn(), viewHost = {
    viewport: { scrollTop: 0, height: 180 }, scrollBy, scrollCaret,
  };
  const layout = new TextLayout({ font: '', measureText: (value) => ({ width: value.length * 8 }) },
    { font: '13px monospace', lineHeight: 18, baseline: 14 });
  const evalCtl = { evalAtCursor: vi.fn(), evalAll: vi.fn(), hush: vi.fn() };
  const deps = { surface, input, viewHost: viewHost as never, layout, evalCtl: evalCtl as never,
    formatter: () => undefined, onChange: vi.fn() };
  const controller = vim ? new VimController(createSurfaceHost(deps)) : null;
  if (controller) input.setVim(controller);
  const defaultBindings = { eval: vi.fn(() => true), evalAll: vi.fn(() => true), hush: vi.fn(() => true), format: vi.fn(() => true) };
  const removeBindings = surface.addKeymap([
    { key: 'Mod-Enter', run: defaultBindings.eval },
    { key: 'Mod-Shift-Enter', run: defaultBindings.evalAll },
    { key: 'Mod-.', run: defaultBindings.hush },
    { key: 'Shift-Alt-f', run: defaultBindings.format },
  ], 'default');
  cleanup.push(() => { removeBindings(); controller?.dispose(); input.dispose(); surface.dispose(); client.close(); container.remove(); });
  return { surface, input, controller, el: input.accessibility.textarea, evalSelection, evalAll, stopAll, cut, scrollBy, defaultBindings };
}
function key(el: HTMLElement, value: string, options: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key: value, bubbles: true, cancelable: true, ...options });
  el.dispatchEvent(event); return event;
}
function before(el: HTMLElement, inputType: string, data: string | null = null, isComposing = false): InputEvent {
  const event = new InputEvent('beforeinput', { inputType, data, isComposing, bubbles: true, cancelable: true });
  el.dispatchEvent(event); return event;
}
function composition(el: HTMLElement, kind: string, data = ''): void {
  el.dispatchEvent(new CompositionEvent(kind, { data, bubbles: true }));
}
function platform(value: string): () => void {
  const descriptor = Object.getOwnPropertyDescriptor(navigator, 'platform');
  Object.defineProperty(navigator, 'platform', { configurable: true, value });
  return () => descriptor ? Object.defineProperty(navigator, 'platform', descriptor) : Reflect.deleteProperty(navigator, 'platform');
}

afterEach(() => { for (const dispose of cleanup.splice(0).reverse()) dispose(); });

describe('Vim input adapter', () => {
  it('leaves typing, composition and Ctrl-Enter unchanged when Vim is off', () => {
    const { surface, el, defaultBindings } = setup('', false);
    before(el, 'insertText', '語');
    composition(el, 'compositionstart'); composition(el, 'compositionend', '日本');
    key(el, 'Enter', { ctrlKey: true });
    expect(surface.state.doc.toString()).toBe('語日本');
    expect(defaultBindings.eval).toHaveBeenCalledOnce();
  });

  it.each(['Dead', 'Tab'])('consumes %s in normal mode without editing', (value) => {
    const { surface, el } = setup();
    const event = key(el, value);
    expect(event.defaultPrevented).toBe(true);
    expect(surface.state.doc.toString()).toBe('alpha beta\ngamma');
  });

  it('discards keyCode 229 composition in normal mode without preedit or revision', async () => {
    const { surface, el } = setup();
    const revision = surface.sync.revision;
    const ime = new KeyboardEvent('keydown', { key: 'Process', bubbles: true, cancelable: true });
    Object.defineProperty(ime, 'keyCode', { value: 229 }); el.dispatchEvent(ime);
    composition(el, 'compositionstart'); composition(el, 'compositionupdate', '日本');
    expect(surface.state.doc.toString()).toBe('alpha beta\ngamma');
    expect(surface.sync.revision).toBe(revision);
    composition(el, 'compositionend', '日本'); before(el, 'insertFromComposition', '日本', true);
    await Promise.resolve();
    expect(surface.state.doc.toString()).toBe('alpha beta\ngamma');
    expect(surface.sync.revision).toBe(revision);
  });

  it('discards a composition begun in normal mode even if i is pressed before commit', async () => {
    const { surface, el, controller } = setup();
    composition(el, 'compositionstart'); composition(el, 'compositionupdate', '語');
    key(el, 'i');
    composition(el, 'compositionend', '語'); before(el, 'insertFromComposition', '語', true);
    await Promise.resolve();
    expect(controller?.snapshot.mode).toBe('insert');
    expect(surface.state.doc.toString()).toBe('alpha beta\ngamma');
  });

  it('commits Japanese composition once in insert mode as one undo group', () => {
    const { surface, el, controller } = setup('abc');
    key(el, 'i'); composition(el, 'compositionstart'); composition(el, 'compositionupdate', '日本');
    composition(el, 'compositionend', '日本'); before(el, 'insertFromComposition', '日本', true);
    expect(controller?.snapshot.mode).toBe('insert');
    expect(surface.state.doc.toString()).toBe('日本abc');
    expect(surface.retentionStatus.undoDepth).toBe(1);
    surface.undo(); expect(surface.state.doc.toString()).toBe('abc');
  });

  it('closes a completion popup before Escape leaves insert mode, including Ctrl-[', () => {
    const { el, controller, surface } = setup();
    key(el, 'i'); let close = true;
    const remove = surface.addKeymap([{ key: 'Escape', run: () => { if (!close) return false; close = false; return true; } }], 'highest');
    key(el, 'Escape'); expect(controller?.snapshot.mode).toBe('insert');
    key(el, '[', { ctrlKey: true }); expect(controller?.snapshot.mode).toBe('normal');
    remove();
  });

  it.each([
    ['Linux x86_64', 'ctrlKey'], ['MacIntel', 'metaKey'],
  ] as const)('%s routes Mod eval, eval-all, hush and format in normal mode', (os, modKey) => {
    const restore = platform(os); const { el, surface, defaultBindings } = setup();
    key(el, 'Enter', { [modKey]: true }); key(el, 'Enter', { [modKey]: true, shiftKey: true });
    key(el, '.', { [modKey]: true });
    const formatEvent = new KeyboardEvent('keydown', { key: 'f', code: 'KeyF', altKey: true, shiftKey: true, cancelable: true });
    expect(surface.matchesKeymap(formatEvent, 'default')).toBe(true);
    el.dispatchEvent(formatEvent);
    expect(formatEvent.defaultPrevented).toBe(true);
    expect(defaultBindings.eval).toHaveBeenCalledOnce(); expect(defaultBindings.evalAll).toHaveBeenCalledOnce();
    expect(defaultBindings.hush).toHaveBeenCalledOnce(); expect(defaultBindings.format).toHaveBeenCalledOnce();
    restore();
  });

  it.each(['normal', 'visual', 'operator-pending', 'insert'])('routes registered shortcut bindings in %s mode', (mode) => {
    const restore = platform('Linux x86_64'); const { el, controller, defaultBindings } = setup();
    if (mode === 'visual') key(el, 'v');
    if (mode === 'operator-pending') key(el, 'd');
    if (mode === 'insert') key(el, 'i');
    key(el, 'Enter', { ctrlKey: true }); key(el, 'Enter', { ctrlKey: true, shiftKey: true }); key(el, '.', { ctrlKey: true });
    key(el, 'f', { code: 'KeyF', altKey: true, shiftKey: true });
    expect(defaultBindings.eval).toHaveBeenCalledOnce(); expect(defaultBindings.evalAll).toHaveBeenCalledOnce();
    expect(defaultBindings.hush).toHaveBeenCalledOnce(); expect(defaultBindings.format).toHaveBeenCalledOnce();
    expect(controller?.snapshot.mode).toBe(mode);
    restore();
  });

  it.each(['normal', 'visual', 'operator-pending'])('passes KeyboardController Mod fallbacks in %s mode', (mode) => {
    const restore = platform('Linux x86_64'); const { el, surface, controller } = setup();
    if (mode === 'visual') key(el, 'v');
    if (mode === 'operator-pending') key(el, 'd');
    key(el, 'a', { ctrlKey: true });
    expect(surface.state.selection.main.from).toBe(0);
    expect(surface.state.selection.main.to).toBe(surface.state.doc.length);
    expect(controller?.snapshot.mode).toBe(mode === 'normal' ? 'visual' : mode);
    restore();
  });

  it.each(['normal', 'visual', 'operator-pending'])('passes undo and redo fallbacks through Vim in %s mode', (mode) => {
    const restore = platform('Linux x86_64'); const { el, surface } = setup('abc');
    surface.dispatch({ changes: { from: 3, insert: 'x' }, userEvent: 'input.type' });
    if (mode === 'visual') key(el, 'v');
    if (mode === 'operator-pending') key(el, 'd');
    key(el, 'z', { ctrlKey: true }); expect(surface.state.doc.toString()).toBe('abc');
    key(el, 'y', { ctrlKey: true }); expect(surface.state.doc.toString()).toBe('abcx');
    restore();
  });

  it.each(['Alt', 'Meta', 'Ctrl'])('consumes modified Backspace and Delete (%s) in every non-insert mode', (modifier) => {
    const restore = platform('Linux x86_64');
    for (const mode of ['normal', 'visual', 'operator-pending'] as const) {
      for (const keyName of ['Backspace', 'Delete']) {
        const { surface, el } = setup('alpha beta');
        if (mode === 'visual') key(el, 'v');
        if (mode === 'operator-pending') key(el, 'd');
        const event = key(el, keyName, { [`${modifier.toLowerCase()}Key`]: true });
        expect(event.defaultPrevented).toBe(true);
        expect(surface.state.doc.toString()).toBe('alpha beta');
        cleanup.pop()?.();
      }
    }
    restore();
  });

  it.each(['Tab', 'Shift-Tab'])('consumes %s without editing in non-insert modes', (chord) => {
    const { surface, el } = setup();
    const event = chord === 'Tab' ? key(el, 'Tab') : key(el, 'Tab', { shiftKey: true });
    expect(event.defaultPrevented).toBe(true);
    expect(surface.state.doc.toString()).toBe('alpha beta\ngamma');
  });

  it('retains insert-mode Alt-Backspace word deletion', () => {
    const { surface, el } = setup('alpha beta'); key(el, 'A'); key(el, 'Backspace', { altKey: true });
    expect(surface.state.doc.toString()).toBe('alpha ');
  });

  it('restores replaced text with Backspace and keeps Ctrl-r redo', () => {
    const { surface, el } = setup('abc'); key(el, 'R'); before(el, 'insertText', 'X');
    expect(surface.state.doc.toString()).toBe('Xbc'); key(el, 'Backspace');
    expect(surface.state.doc.toString()).toBe('abc');
  });

  it('routes Ctrl-r and Ctrl-d to Vim redo and viewport scrolling', () => {
    const { surface, el, scrollBy } = setup('abc');
    surface.dispatch({ changes: { from: 3, insert: 'x' }, userEvent: 'input.type' });
    surface.undo(); key(el, 'r', { ctrlKey: true }); expect(surface.state.doc.toString()).toBe('abcx');
    key(el, 'd', { ctrlKey: true }); expect(scrollBy).toHaveBeenCalledWith(0, 90);
  });
});
