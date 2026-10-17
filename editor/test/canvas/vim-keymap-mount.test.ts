// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import { Text } from '@codemirror/state';
import type { EditorDeps } from '../../src/app/deps';
import { mount } from '../../src/code/mount';
import { CanvasRenderer } from '../../src/code/renderer';
import { buildLayout } from '../../src/app/layout';
import { FileCancelled, MemoryFiles } from '../../src/platform/files';
import { BrowserJsonFiles, JsonFileTooLarge } from '../../src/platform/json-files';
import { KEYMAP_MAX_BYTES } from '../../src/code/vim/keymap-schema';
import { KEYMAP_PRESETS } from '../../src/code/vim/presets';
import { LocalKeymapStore, VIM_KEYMAP_KEY, type KeymapStore } from '../../src/code/vim-keymap-store';
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
  step(time: number): void;
}
function rafHost(): RafHost {
  let next = 1; const callbacks = new Map<number, FrameRequestCallback>();
  return { devicePixelRatio: 1, innerHeight: 800,
    requestAnimationFrame(cb) { const id = next++; callbacks.set(id, cb); return id; },
    cancelAnimationFrame(id) { callbacks.delete(id); },
    step(time) { const first = callbacks.entries().next().value as [number, FrameRequestCallback] | undefined; if (first) { callbacks.delete(first[0]); first[1](time); } },
  };
}
const rigs: { root: HTMLElement; deps: EditorDeps; mounted: ReturnType<typeof mount>; fakes: CanvasFakes; host: RafHost }[] = [];
function setup(options: { enabled?: boolean; keymap?: string; keymapStore?: KeymapStore } = {}) {
  const fakes = installCanvasFakes();
  const context = fakes.ctx(document.createElement('canvas')) as unknown as Record<string, unknown>;
  Object.assign(Object.getPrototypeOf(context) as object, { measureText: (text: string) => ({ width: text.length * 8 }), setTransform() {}, save() {}, restore() {}, rect() {}, clip() {} });
  const root = document.createElement('div'); document.body.append(root); buildLayout(root);
  const store = new Store(), transport = new RecordingTransport();
  const deps: EditorDeps = { client: new Client(transport, { store }), store, clock: new MockClock(), tier: 'browser', files: new MemoryFiles() };
  if (options.keymapStore) deps.vimKeymap = { store: options.keymapStore, files: new BrowserJsonFiles(window) };
  if (options.keymap) localStorage.setItem(VIM_KEYMAP_KEY, options.keymap);
  if (options.enabled !== false) localStorage.setItem('vactr.vimMode', 'true');
  const host = rafHost(), mounted = mount(root, deps, { frameHost: host });
  const rig = { root, deps, mounted, fakes, host }; rigs.push(rig); return rig;
}
async function settle(): Promise<void> { await Promise.resolve(); await Promise.resolve(); }
function key(rig: ReturnType<typeof setup>, value: string, options: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key: value, bubbles: true, cancelable: true, ...options });
  rig.root.querySelector('textarea')!.dispatchEvent(event); return event;
}
function selectMisePreset(rig: ReturnType<typeof setup>): void {
  [...rig.root.querySelectorAll<HTMLButtonElement>('.vact-vim-keymap-preset')]
    .find((button) => button.textContent === 'mise-darwin')!.click();
}
afterEach(() => {
  for (const rig of rigs.splice(0)) { rig.mounted.dispose(); rig.deps.client.close(); rig.deps.store.dispose(); rig.root.remove(); rig.fakes.restore(); }
  vi.useRealTimers(); localStorage.clear(); document.body.replaceChildren(); vi.restoreAllMocks();
});

describe('Vim keymap mount integration', () => {
  it('opens the keymap panel and selecting mise-darwin fills its JSON', async () => {
    const rig = setup(); await settle();
    rig.root.querySelector<HTMLButtonElement>('.vact-vim-keymap-button')!.click();
    selectMisePreset(rig);
    expect(rig.root.querySelector<HTMLTextAreaElement>('.vact-vim-keymap-text')!.value).toContain('"lhs": "kj"');
    expect(rig.root.querySelector('.vact-vim-keymap')).not.toBeNull();
  });

  it('shows a path-qualified issue for invalid entries and does not save', async () => {
    const rig = setup(); await settle();
    rig.root.querySelector<HTMLButtonElement>('.vact-vim-keymap-button')!.click();
    const field = rig.root.querySelector<HTMLTextAreaElement>('.vact-vim-keymap-text')!;
    field.value = '{"version":2}';
    field.dispatchEvent(new InputEvent('input', { bubbles: true }));
    rig.root.querySelector<HTMLButtonElement>('.vact-vim-keymap-apply')!.click(); await settle();
    expect(rig.root.querySelector('.vact-vim-keymap-issues')?.textContent).toContain('version');
    expect(localStorage.getItem(VIM_KEYMAP_KEY)).toBeNull();
  });

  it('saves a valid keymap and applies kj live', async () => {
    const rig = setup(); await settle();
    rig.root.querySelector<HTMLButtonElement>('.vact-vim-keymap-button')!.click();
    selectMisePreset(rig);
    rig.root.querySelector<HTMLButtonElement>('.vact-vim-keymap-apply')!.click(); await settle();
    expect(localStorage.getItem(VIM_KEYMAP_KEY)).toContain('"version": 1');
    key(rig, 'i'); key(rig, 'k'); key(rig, 'j');
    expect(rig.root.querySelector('.vact-vim-status')?.getAttribute('data-mode')).toBe('normal');
  });

  it('reset removes stored keymap and restores K0 without hop', async () => {
    const rig = setup({ keymap: KEYMAP_PRESETS.find((preset) => preset.id === 'mise-darwin')!.text }); await settle();
    rig.root.querySelector<HTMLButtonElement>('.vact-vim-keymap-button')!.click();
    rig.root.querySelector<HTMLButtonElement>('.vact-vim-keymap-reset')!.click(); await settle();
    expect(localStorage.getItem(VIM_KEYMAP_KEY)).toBeNull();
    key(rig, 'g'); key(rig, 'j');
    expect(rig.root.querySelector('.vact-vim-hints')).toBeNull();
  });

  it('keeps a valid keymap active in session when storage writes fail', async () => {
    const badStore: KeymapStore = { get: async () => null, set: async () => ({ ok: false, reason: 'unavailable' }), reset: async () => ({ ok: false, reason: 'unavailable' }) };
    const rig = setup({ keymapStore: badStore }); await settle();
    rig.root.querySelector<HTMLButtonElement>('.vact-vim-keymap-button')!.click();
    selectMisePreset(rig);
    rig.root.querySelector<HTMLButtonElement>('.vact-vim-keymap-apply')!.click(); await settle();
    expect(rig.root.querySelector('.vact-vim-keymap-status')?.textContent).toContain('keymap not saved: storage unavailable');
    key(rig, 'i'); key(rig, 'k'); key(rig, 'j');
    expect(rig.root.querySelector('.vact-vim-status')?.getAttribute('data-mode')).toBe('normal');
  });

  it('shows hint summary after configured 200 ms delay', async () => {
    vi.useFakeTimers();
    const rig = setup({ keymap: KEYMAP_PRESETS.find((preset) => preset.id === 'mise-darwin')!.text }); await settle();
    key(rig, ' '); vi.advanceTimersByTime(200);
    expect(rig.root.querySelector('.vact-vim-hints-summary')?.textContent).toContain('next');
  });

  it('closes hints on Escape and clears pending state on focusout', async () => {
    vi.useFakeTimers();
    const rig = setup({ keymap: KEYMAP_PRESETS.find((preset) => preset.id === 'mise-darwin')!.text }); await settle();
    key(rig, ' '); vi.advanceTimersByTime(200);
    expect(rig.root.querySelector('.vact-vim-hints')).not.toBeNull();
    key(rig, 'Escape'); expect(rig.root.querySelector('.vact-vim-hints')).toBeNull();
    key(rig, ' '); rig.root.querySelector('textarea')!.dispatchEvent(new FocusEvent('focusout', { bubbles: true }));
    expect(rig.root.querySelector('.vact-vim-hints')).toBeNull();
  });

  it('forwards hop labels through the renderer overlay after gj and a target character', async () => {
    const render = vi.spyOn(CanvasRenderer.prototype, 'render');
    const rig = setup({ keymap: KEYMAP_PRESETS.find((preset) => preset.id === 'mise-darwin')!.text }); await settle();
    const surface = rig.deps.code!.surface;
    surface.dispatch({ changes: { from: 0, insert: Text.of(['banana bandana']) } });
    const pane = rig.root.querySelector<HTMLElement>('.vact-code')!;
    vi.spyOn(pane, 'getBoundingClientRect').mockReturnValue({ x: 0, y: 0, width: 600, height: 180, top: 0, right: 600, bottom: 180, left: 0, toJSON: () => ({}) });
    window.dispatchEvent(new Event('resize')); rig.host.step(1);
    key(rig, 'g'); key(rig, 'j'); key(rig, 'a'); rig.host.step(16);
    expect(render.mock.calls.some(([frame]) => !!frame && (frame.hopLabels?.length ?? 0) > 0 && typeof frame.hopRevision === 'number')).toBe(true);
  });

  it('keeps keymap store byte limit aligned with the schema limit', async () => {
    const storage = new Map<string, string>();
    const adapter = { getItem: (key: string) => storage.get(key) ?? null, setItem: (key: string, value: string) => storage.set(key, value), removeItem: (key: string) => storage.delete(key) };
    const result = await new LocalKeymapStore(adapter).set('x'.repeat(KEYMAP_MAX_BYTES + 1));
    expect(result).toMatchObject({ ok: false, reason: 'too-large' });
  });

  it('resolves command-line Ctrl-a and Ctrl-g through the compiled c-mode map', async () => {
    const rig = setup({ keymap: KEYMAP_PRESETS.find((preset) => preset.id === 'mise-darwin')!.text }); await settle();
    key(rig, ':');
    const command = rig.root.querySelector<HTMLInputElement>('.vact-vim-ex')!;
    command.value = 'abc'; command.setSelectionRange(3, 3);
    command.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', ctrlKey: true, bubbles: true, cancelable: true }));
    expect(command.selectionStart).toBe(0);
    command.dispatchEvent(new KeyboardEvent('keydown', { key: 'g', ctrlKey: true, bubbles: true, cancelable: true }));
    expect(rig.root.querySelector('.vact-vim-ex')).toBeNull();
  });

  it('imports JSON while treating cancellation and oversize files as explicit outcomes', async () => {
    const picker = { showOpenFilePicker: vi.fn().mockRejectedValue(new DOMException('cancelled', 'AbortError')), document } as unknown as Window;
    const files = new BrowserJsonFiles(picker);
    await expect(files.importJson()).rejects.toBeInstanceOf(FileCancelled);
    const tooLarge = { showOpenFilePicker: vi.fn().mockResolvedValue([{ getFile: async () => ({ text: async () => 'x'.repeat(KEYMAP_MAX_BYTES + 1) }) }]), document } as unknown as Window;
    await expect(new BrowserJsonFiles(tooLarge).importJson()).rejects.toBeInstanceOf(JsonFileTooLarge);
  });
});
