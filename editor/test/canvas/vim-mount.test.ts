// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import type { EditorDeps } from '../../src/app/deps';
import { mount } from '../../src/code/mount';
import { CanvasRenderer } from '../../src/code/renderer';
import { buildLayout } from '../../src/app/layout';
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

afterEach(() => {
  for (const rig of rigs.splice(0)) { rig.mounted.dispose(); rig.deps.client.close(); rig.deps.store.dispose(); rig.root.remove(); rig.fakes.restore(); }
  localStorage.clear(); document.body.replaceChildren(); vi.restoreAllMocks();
});

describe('Vim mount adapter', () => {
  it('starts off when the setting is absent and adds a reachable toggle', () => {
    const rig = setup();
    expect(rig.root.querySelector('.vact-vim-status')).toBeNull();
    expect(rig.root.querySelector('.vact-code')?.hasAttribute('data-vim')).toBe(false);
    expect(rig.root.querySelector('.vact-vim-toggle')?.getAttribute('aria-pressed')).toBe('false');
  });

  it('restores enabled mode and mounts the normal status row', () => {
    localStorage.setItem('vactr.vimMode', 'true'); const rig = setup();
    expect(rig.root.querySelector('.vact-vim-status')?.getAttribute('data-mode')).toBe('normal');
    expect(rig.root.querySelector('.vact-code')?.getAttribute('data-vim')).toBe('on');
    expect(rig.root.querySelector('.vact-vim-toggle')?.getAttribute('aria-pressed')).toBe('true');
  });

  it('persists toggle on and off and mounts or removes status', () => {
    const rig = setup(), toggle = rig.root.querySelector<HTMLButtonElement>('.vact-vim-toggle')!;
    toggle.click(); expect(localStorage.getItem('vactr.vimMode')).toBe('true');
    expect(rig.root.querySelector('.vact-vim-status')).not.toBeNull();
    toggle.click(); expect(localStorage.getItem('vactr.vimMode')).toBe('false');
    expect(rig.root.querySelector('.vact-vim-status')).toBeNull();
  });

  it('handles unavailable localStorage reads', () => {
    const descriptor = Object.getOwnPropertyDescriptor(window, 'localStorage');
    Object.defineProperty(window, 'localStorage', { configurable: true, get() { throw new Error('blocked'); } });
    try { const rig = setup(); expect(rig.root.querySelector('.vact-vim-toggle')?.getAttribute('aria-pressed')).toBe('false'); }
    finally { if (descriptor) Object.defineProperty(window, 'localStorage', descriptor); }
  });

  it('handles unavailable localStorage writes while toggling', () => {
    const write = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('blocked'); });
    const rig = setup(); expect(() => rig.root.querySelector<HTMLButtonElement>('.vact-vim-toggle')!.click()).not.toThrow();
    expect(rig.root.querySelector('.vact-vim-status')).not.toBeNull(); expect(write).toHaveBeenCalledWith('vactr.vimMode', 'true');
  });

  it('passes block and bar cursor feedback to the renderer and omits shape when off', () => {
    const render = vi.spyOn(CanvasRenderer.prototype, 'render');
    const rig = setup(); rig.host.step(16);
    expect(render.mock.calls.at(-1)?.[0]).not.toHaveProperty('cursorShape');
    rig.root.querySelector<HTMLButtonElement>('.vact-vim-toggle')!.click(); rig.host.step(32);
    expect(render.mock.calls.at(-1)?.[0]).toMatchObject({ cursorShape: 'block' });
    key(rig, 'i'); rig.host.step(48);
    expect(render.mock.calls.at(-1)?.[0]).toMatchObject({ cursorShape: 'bar' });
  });

  it.each([['Linux x86_64', 'ctrlKey'], ['MacIntel', 'metaKey']] as const)('%s routes Mod-Enter to eval', (os, modKey) => {
    const original = Object.getOwnPropertyDescriptor(navigator, 'platform');
    Object.defineProperty(navigator, 'platform', { configurable: true, value: os });
    try {
      const rig = setup(); rig.root.querySelector<HTMLButtonElement>('.vact-vim-toggle')!.click();
      key(rig, 'Enter', { [modKey]: true });
      expect(rig.transport.of('eval')).toHaveLength(1);
    } finally {
      if (original) Object.defineProperty(navigator, 'platform', original); else Reflect.deleteProperty(navigator, 'platform');
    }
  });

  it.each([['Linux x86_64', 'metaKey'], ['MacIntel', 'ctrlKey']] as const)('%s routes alternate shortcut fallback while Vim is on', (os, modKey) => {
    const original = Object.getOwnPropertyDescriptor(navigator, 'platform');
    Object.defineProperty(navigator, 'platform', { configurable: true, value: os });
    try {
      const rig = setup(); rig.root.querySelector<HTMLButtonElement>('.vact-vim-toggle')!.click();
      key(rig, 'Enter', { [modKey]: true });
      key(rig, 'Enter', { [modKey]: true, shiftKey: true });
      key(rig, '.', { [modKey]: true });
      expect(rig.transport.of('eval')).toHaveLength(2);
      expect(rig.transport.of('hush')).toHaveLength(1);
    } finally {
      if (original) Object.defineProperty(navigator, 'platform', original); else Reflect.deleteProperty(navigator, 'platform');
    }
  });

  it('disposes status, toggle handlers, and Vim input routing with the mount', () => {
    const rig = setup(); rig.root.querySelector<HTMLButtonElement>('.vact-vim-toggle')!.click();
    const surface = rig.deps.code!.surface, before = surface.state.doc.toString();
    rig.mounted.dispose();
    expect(rig.root.querySelector('.vact-vim-status')).toBeNull();
    expect(rig.root.querySelector('.vact-vim-toggle')).toBeNull();
    expect(rig.root.querySelector('.vact-code')).toBeNull();
    expect(surface.state.doc.toString()).toBe(before);
  });

  it('routes registered Mod shortcut while normal mode is active', () => {
    const rig = setup(); rig.root.querySelector<HTMLButtonElement>('.vact-vim-toggle')!.click();
    const mod = /Mac|iP/.test(navigator.platform) ? { metaKey: true } : { ctrlKey: true };
    key(rig, 'Enter', mod);
    expect(rig.transport.of('eval')).toHaveLength(1);
    expect(rig.root.querySelector('.vact-vim-status')?.getAttribute('data-mode')).toBe('normal');
  });

  it('keeps modified Backspace and Delete from editing in mounted normal mode', () => {
    const rig = setup(); rig.root.querySelector<HTMLButtonElement>('.vact-vim-toggle')!.click();
    const original = rig.deps.code!.surface.state.doc.toString();
    for (const name of ['Backspace', 'Delete']) {
      for (const modifier of [{ altKey: true }, { metaKey: true }, { ctrlKey: true }]) {
        key(rig, name, modifier);
        expect(rig.deps.code!.surface.state.doc.toString()).toBe(original);
      }
    }
  });
});
