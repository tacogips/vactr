// @vitest-environment jsdom

import { Transaction } from '@codemirror/state';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { EditorDeps } from '../../src/app/deps';
import { buildLayout } from '../../src/app/layout';
import { mount } from '../../src/code/mount';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { MockClock } from '../support/clock';
import { RecordingTransport } from '../support/recording';
import { installCanvasFakes, type CanvasFakes } from '../support/canvas';

interface RafHost {
  requestAnimationFrame(cb: FrameRequestCallback): number;
  cancelAnimationFrame(id: number): void;
  devicePixelRatio: number;
  innerHeight: number;
  callbacks: Map<number, FrameRequestCallback>;
  step(time: number): void;
}
function rafHost(): RafHost {
  let next = 1; const callbacks = new Map<number, FrameRequestCallback>();
  return { callbacks, devicePixelRatio: 1, innerHeight: 800,
    requestAnimationFrame(cb) { const id = next++; callbacks.set(id, cb); return id; },
    cancelAnimationFrame(id) { callbacks.delete(id); },
    step(time) { const first = callbacks.entries().next().value as [number, FrameRequestCallback] | undefined; if (first) { callbacks.delete(first[0]); first[1](time); } },
  };
}
const rigs: { root: HTMLElement; deps: EditorDeps; mounted: ReturnType<typeof mount>; fakes: CanvasFakes; host: RafHost; transport: RecordingTransport }[] = [];
function setup() {
  const fakes = installCanvasFakes();
  const context = fakes.ctx(document.createElement('canvas')) as unknown as Record<string, unknown>;
  Object.assign(Object.getPrototypeOf(context) as object, { measureText: (text: string) => ({ width: text.length * 8 }), setTransform() {}, save() {}, restore() {}, rect() {}, clip() {} });
  const root = document.createElement('div'); document.body.append(root);
  buildLayout(root);
  const store = new Store(), transport = new RecordingTransport();
  const deps: EditorDeps = { client: new Client(transport, { store }), store, clock: new MockClock(), tier: 'browser', files: new MemoryFiles() };
  const host = rafHost(), mounted = mount(root, deps, { frameHost: host });
  const rig = { root, deps, mounted, fakes, host, transport }; rigs.push(rig); return rig;
}
function key(rig: ReturnType<typeof setup>, name: string, options: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key: name, bubbles: true, cancelable: true, ...options });
  rig.root.querySelector('textarea')!.dispatchEvent(event); return event;
}
function replace(rig: ReturnType<typeof setup>, text: string, userEvent?: string) {
  const surface = rig.deps.code!.surface;
  surface.dispatch({ changes: { from: 0, to: surface.state.doc.length, insert: text },
    ...(userEvent ? { annotations: [Transaction.addToHistory.of(false), Transaction.userEvent.of(userEvent)] } : {}) });
}
function enable(rig: ReturnType<typeof setup>) {
  rig.root.querySelector<HTMLButtonElement>('.vact-vim-toggle')!.click();
}
function promptKey(rig: ReturnType<typeof setup>, name: string, options: KeyboardEventInit = {}) {
  const event = new KeyboardEvent('keydown', { key: name, bubbles: true, cancelable: true, ...options });
  rig.root.querySelector<HTMLInputElement>('.vact-vim-ex')!.dispatchEvent(event); return event;
}
function seedMap(lhs: string, rhs: string) {
  localStorage.setItem('vactr.vimKeymap', JSON.stringify({ version: 1, leader: '\\', timeoutlen: 1000,
    maps: { c: [{ lhs, rhs }] }, hints: { enabled: false, delayMs: 200, triggers: {} },
    hop: { enabled: false, keys: 'fdjhklsagqwertyuiopzxcvbnm', uppercaseLabels: false } }));
}
afterEach(() => {
  for (const rig of rigs.splice(0)) { rig.mounted.dispose(); rig.deps.client.close(); rig.deps.store.dispose(); rig.root.remove(); rig.fakes.restore(); }
  localStorage.clear(); document.body.replaceChildren(); vi.restoreAllMocks();
});

describe('mounted Vim parity oracle', () => {
  it.each(['input.load.document', 'input.replace.document'])('P1a resets insert, repeat and undo on %s', async (boundary) => {
    const rig = setup(); replace(rig, 'abc'); enable(rig); await Promise.resolve();
    key(rig, 'x'); await Promise.resolve(); expect(rig.deps.code!.surface.state.doc.toString()).toBe('bc');
    key(rig, 'i'); await Promise.resolve(); expect(rig.root.querySelector('.vact-vim-status')?.getAttribute('data-mode')).toBe('insert');
    replace(rig, 'new text', boundary); await Promise.resolve();
    expect(rig.root.querySelector('.vact-vim-status')?.getAttribute('data-mode')).toBe('normal');
    key(rig, '.'); await Promise.resolve(); expect(rig.deps.code!.surface.state.doc.toString()).toBe('new text');
    key(rig, 'u'); await Promise.resolve(); expect(rig.deps.code!.surface.state.doc.toString()).toBe('new text');
  });
  it('P1b resets visual mode on load', async () => {
    const rig = setup(); replace(rig, 'abc'); enable(rig); await Promise.resolve(); key(rig, 'v'); await Promise.resolve();
    expect(rig.root.querySelector('.vact-vim-status')?.getAttribute('data-mode')).toBe('visual');
    replace(rig, 'new text', 'input.load.document'); await Promise.resolve();
    expect(rig.root.querySelector('.vact-vim-status')?.getAttribute('data-mode')).toBe('normal');
  });
  it('P2a applies the persisted command-line escape mapping', async () => {
    seedMap('<C-g>', '<Esc>'); const rig = setup(); enable(rig); await vi.waitFor(() => expect(rig.root.querySelector('.vact-vim-status')).not.toBeNull());
    await new Promise(resolve => setTimeout(resolve, 0)); key(rig, ':'); await Promise.resolve();
    const event = promptKey(rig, 'g', { ctrlKey: true }); await Promise.resolve();
    expect(event.defaultPrevented).toBe(true); expect(rig.root.querySelector('.vact-vim-ex')).toBeNull();
  });
  it('P2b applies the persisted command-line submit mapping', async () => {
    seedMap('<C-j>', '<CR>'); const rig = setup(); enable(rig); await vi.waitFor(() => expect(rig.root.querySelector('.vact-vim-status')).not.toBeNull());
    await new Promise(resolve => setTimeout(resolve, 0)); key(rig, ':'); await Promise.resolve();
    rig.root.querySelector<HTMLInputElement>('.vact-vim-ex')!.value = 'eval'; promptKey(rig, 'j', { ctrlKey: true }); await Promise.resolve();
    expect(rig.root.querySelector('.vact-vim-ex')).toBeNull(); expect(rig.transport.of('eval')).toHaveLength(1);
  });
  it.each(['w', 'q', 'wq', 'x', 'w!'])('P3 displays live sync feedback for :%s', async (command) => {
    const rig = setup(); replace(rig, 'abc'); enable(rig); await Promise.resolve(); key(rig, ':'); await Promise.resolve();
    rig.root.querySelector<HTMLInputElement>('.vact-vim-ex')!.value = command; promptKey(rig, 'Enter'); await Promise.resolve();
    const message = rig.root.querySelector('.vact-vim-message');
    expect(message?.textContent).toBe('vactr syncs the document live; use :eval, :evalall or :hush');
    expect(message?.getAttribute('data-kind')).toBe('info'); expect(rig.deps.code!.surface.state.doc.toString()).toBe('abc');
  });
  it('P4 preserves the document for Mac command deletion keys', async () => {
    const original = Object.getOwnPropertyDescriptor(navigator, 'platform');
    Object.defineProperty(navigator, 'platform', { configurable: true, value: 'MacIntel' });
    try {
      const rig = setup(); replace(rig, 'abc def'); enable(rig); await Promise.resolve();
      for (const name of ['Backspace', 'Delete']) { key(rig, name, { metaKey: true }); await Promise.resolve(); expect(rig.deps.code!.surface.state.doc.toString()).toBe('abc def'); }
    } finally { if (original) Object.defineProperty(navigator, 'platform', original); else Reflect.deleteProperty(navigator, 'platform'); }
  });
});
