// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import { Text } from '@codemirror/state';
import { InputController } from '../../src/code/input';
import { TextLayout } from 'emaki/engine';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { VimController } from '../../src/code/vim/controller';
import { compileKeymap } from '../../src/code/vim/keymap-schema';
import { KEYMAP_PRESETS } from '../../src/code/vim/presets';
import { createSurfaceHost } from '../../src/code/vim-mount';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { RecordingTransport } from '../support/recording';

const disposers: (() => void)[] = [];
function setup(text = 'alpha beta\ngamma') {
  const transport = new RecordingTransport();
  const client = new Client(transport, { store: new Store(), now: () => Date.now() });
  const sync = new DocumentSync(client.document('main.vact'), Text.of(text.split('\n')));
  const surface = new CodeSurface({ sync });
  const container = document.createElement('div'); document.body.append(container);
  const input = new InputController(surface, container);
  const viewHost = { viewport: { width: 600, height: 180, scrollTop: 0, scrollLeft: 0, left: 0, top: 0 }, scrollBy: vi.fn(), scrollCaret: vi.fn() };
  const layout = new TextLayout({ font: '', measureText: (value) => ({ width: value.length * 8 }) },
    { font: '13px monospace', lineHeight: 18, baseline: 14 });
  const evalCtl = { evalAtCursor: vi.fn(), evalAll: vi.fn(), hush: vi.fn() };
  const deps = { surface, input, viewHost: viewHost as never, layout, evalCtl: evalCtl as never,
    formatter: () => undefined, onChange: vi.fn() };
  const controller = new VimController(createSurfaceHost(deps));
  const compiled = compileKeymap(KEYMAP_PRESETS.find((preset) => preset.id === 'mise-darwin')!.text);
  expect(compiled.keymap).not.toBeNull();
  controller.setKeymap(compiled.keymap);
  input.setVim(controller);
  disposers.push(() => { controller.dispose(); input.dispose(); surface.dispose(); client.close(); container.remove(); });
  return { surface, input, controller, el: input.accessibility.textarea, client };
}
function key(el: HTMLElement, value: string, options: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key: value, bubbles: true, cancelable: true, ...options });
  el.dispatchEvent(event); return event;
}
function before(el: HTMLElement, inputType: string, data: string | null = null, isComposing = false): void {
  el.dispatchEvent(new InputEvent('beforeinput', { inputType, data, isComposing, bubbles: true, cancelable: true }));
}
function composition(el: HTMLElement, kind: string, data = ''): void {
  el.dispatchEvent(new CompositionEvent(kind, { data, bubbles: true }));
}
afterEach(() => { vi.useRealTimers(); for (const dispose of disposers.splice(0).reverse()) dispose(); document.body.replaceChildren(); });

describe('Vim keymap InputController integration', () => {
  it('maps kj to Escape without inserting either key', () => {
    const { surface, controller, el } = setup('abc');
    key(el, 'i'); key(el, 'k'); key(el, 'j');
    expect(controller.snapshot.mode).toBe('normal');
    expect(surface.state.doc.toString()).toBe('abc');
  });

  it('inserts a deferred k after timeoutlen when kj does not complete', () => {
    vi.useFakeTimers();
    const { surface, controller, el } = setup('abc');
    key(el, 'i'); key(el, 'k'); vi.advanceTimersByTime(1001);
    expect(controller.snapshot.mode).toBe('insert');
    expect(surface.state.doc.toString()).toBe('kabc');
  });

  it('commits Japanese composition after a pending k exactly once', () => {
    vi.useFakeTimers();
    const { surface, el } = setup('abc');
    key(el, 'i'); key(el, 'k');
    composition(el, 'compositionstart'); composition(el, 'compositionupdate', '日本');
    composition(el, 'compositionend', '日本'); before(el, 'insertFromComposition', '日本', true);
    expect(surface.state.doc.toString()).toBe('k日本abc');
    expect(surface.retentionStatus.undoDepth).toBe(2);
    surface.undo(); expect(surface.state.doc.toString()).toBe('kabc');
    surface.undo(); expect(surface.state.doc.toString()).toBe('abc');
  });

  it('discards normal-mode Japanese composition without inserting text', async () => {
    const { surface, el } = setup('abc');
    composition(el, 'compositionstart'); composition(el, 'compositionupdate', '日本');
    composition(el, 'compositionend', '日本'); before(el, 'insertFromComposition', '日本', true);
    await Promise.resolve();
    expect(surface.state.doc.toString()).toBe('abc');
  });

  it('routes c-mode Ctrl-g to cancel and Ctrl-a to the start of the command', () => {
    const document = JSON.parse(KEYMAP_PRESETS.find((preset) => preset.id === 'mise-darwin')!.text) as { maps: { c: unknown[] } };
    expect(document.maps.c).toEqual(expect.arrayContaining([
      expect.objectContaining({ lhs: '<C-g>', rhs: '<Esc>' }),
      expect.objectContaining({ lhs: '<C-a>', rhs: '<Home>' }),
    ]));
  });

  it('rejects reserved Ctrl-a in normal mappings while preserving schema fallback', () => {
    const result = compileKeymap(JSON.stringify({ version: 1, maps: { n: [{ lhs: '<C-a>', rhs: 'x' }] } }));
    expect(result.keymap).not.toBeNull();
    expect(result.issues).toContainEqual(expect.objectContaining({ path: 'maps.n[0].lhs', severity: 'error' }));
  });

  it('keeps normal-mode Ctrl-a available to the editor select-all keymap', () => {
    const { surface, el } = setup('abc');
    let selected = false;
    const remove = surface.addKeymap([{ key: 'Ctrl-a', run: () => { selected = true; return true; } }], 'highest');
    key(el, 'a', { ctrlKey: true });
    expect(selected).toBe(true); remove();
  });
});
