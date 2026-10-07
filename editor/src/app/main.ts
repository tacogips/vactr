// The editor entry (design 15.1.1, 15.1.3, 15.1.4).
//
// `boot` builds the dependencies from the page URL: `?session=<ws-url>`
// selects the native tier (a WebSocket to `vactr serve`; the token stays
// in that URL and is never stored), anything else the browser tier (wasm
// #1 in session mode plus the worklet). This is the ONLY place a transport
// is chosen. `createEditor` mounts every area in a fixed order; an area's
// API appears on `deps` when its mount sets it, and consumers read it at
// use time. main makes no other decision.

import { BrowserFiles, TauriFiles, type FileAccess } from '../platform/files';
import { invoke } from '@tauri-apps/api/core';
import { Client } from '../protocol/client';
import { SocketTransport } from '../protocol/socket';
import { TauriTransport } from '../protocol/tauri';
import { Store } from '../protocol/store';
import { WasmCore, WasmTransport } from '../protocol/wasm';
import { mount as mountBind } from '../bind/mount';
import { DOC_FILE, mount as mountCode } from '../code/mount';
import { WasmCompletionEngine } from '../code/completion';
import { WasmFormatter } from '../code/format';
import { ToolWasm } from '../code/tool-wasm';
import { loadVactSyntax } from '../code/syntax';
import { mount as mountMidi } from '../midi/mount';
import { mount as mountParams } from '../params/mount';
import { mount as mountPkg } from '../pkg/mount';
import { mount as mountVisual } from '../visual/mount';
import { AudioClock, PageClock, audibleFor } from './clock';
import type { EditorDeps, Mounted, MountFn } from './deps';
import { buildLayout, pane, type Layout } from './layout';
import { mount as mountSong } from './song';
import { mountShell, pageStorage } from '../ui/shell';
import { BootFailure } from '../ui/status-view';
import { createComponent } from 'solid-js';
import { render } from 'solid-js/web';

export const MOUNT_ORDER = ['code', 'midi', 'visual', 'bind', 'params', 'pkg'] as const;

export type AreaName = (typeof MOUNT_ORDER)[number];

const MOUNTS: Record<AreaName, MountFn> = {
  code: mountCode,
  midi: mountMidi,
  visual: mountVisual,
  bind: mountBind,
  params: mountParams,
  pkg: mountPkg,
};

export interface Editor {
  root: HTMLElement;
  deps: EditorDeps;
  layout: Layout;
  dispose(): void;
}

/** Builds the layout and mounts every area in `MOUNT_ORDER`. */
export function createEditor(root: HTMLElement, deps: EditorDeps): Editor {
  const layout = buildLayout(root);
  const handles: Mounted[] = [mountShell(root, pageStorage(root.ownerDocument.defaultView))];
  for (const area of MOUNT_ORDER) handles.push(MOUNTS[area](root, deps));
  handles.push(mountSong(root, deps, DOC_FILE));
  return {
    root,
    deps,
    layout,
    dispose() {
      for (const h of handles.reverse()) h.dispose();
    },
  };
}

export type TierChoice = { tier: 'native'; url: string } | { tier: 'browser' };

/** `?session=<ws-url>` selects the native tier. */
export function tierFromUrl(search: string): TierChoice {
  const url = new URLSearchParams(search).get('session');
  return url ? { tier: 'native', url } : { tier: 'browser' };
}

export function isTauri(win: Window): boolean {
  return '__TAURI_INTERNALS__' in win;
}

/** Builds the dependencies for this page and mounts the editor. */
export async function boot(root: HTMLElement, win: Window = window): Promise<Editor> {
  const choice = tierFromUrl(win.location.search);
  const store = new Store();
  const files: FileAccess = isTauri(win) ? new TauriFiles() : new BrowserFiles(win);
  let deps: EditorDeps;
  let audibleLifecycle: ReturnType<typeof audibleFor>;
  let tauriFallback: string | null = null;
  if (choice.tier === 'native') {
    const client = new Client(new SocketTransport(choice.url), { store });
    audibleLifecycle = audibleFor('native', { client, epoch: () => store.transportSample?.epoch ?? null });
    deps = { client, store, clock: new PageClock(), audible: audibleLifecycle.audible, tier: 'native', files };
  } else {
    let tauriClient: Client | null = null;
    if (isTauri(win)) {
      try {
        tauriClient = new Client(await TauriTransport.connect(), { store });
      } catch (error) {
        tauriFallback = error instanceof Error ? error.message : String(error);
      }
    }
    if (tauriClient) {
      audibleLifecycle = audibleFor('native', { client: tauriClient, epoch: () => store.transportSample?.epoch ?? null });
      deps = { client: tauriClient, store, clock: new PageClock(), audible: audibleLifecycle.audible, tier: 'native', files };
    } else {
      const base = win.document.baseURI;
      const core = await WasmCore.start({
        wasmUrl: new URL('vactr.wasm', base).href,
        processorUrl: new URL('worklet/processor.js', base).href,
      });
      const client = new Client(new WasmTransport(core), { store });
      const ctx = core.host.ctx;
      audibleLifecycle = audibleFor('browser', { ctx, epoch: () => store.transportSample?.epoch ?? null });
      // Audio may start only after a user gesture.
      root.addEventListener('pointerdown', () => void ctx.resume(), { once: true });
      deps = { client, store, clock: new AudioClock(ctx), audible: audibleLifecycle.audible, tier: 'browser', files, core };
    }
  }
  const formatterUrl = new URL('vactr.wasm', win.document.baseURI).href;
  const tool = new ToolWasm(formatterUrl);
  deps.formatter = new WasmFormatter(tool);
  deps.completion = new WasmCompletionEngine(tool);
  deps.syntax = () => loadVactSyntax(win.document.baseURI);
  if (typeof Worker === 'function') {
    deps.syntaxWorker = () => new Worker(new URL('../code/syntax-worker.ts', import.meta.url), { type: 'module' });
  }
  const editor = createEditor(root, deps);
  if (tauriFallback) {
    const notice = root.ownerDocument.createElement('div');
    notice.setAttribute('role', 'status');
    notice.dataset.tauriFallback = 'true';
    notice.textContent = `Native connection failed; using browser mode: ${tauriFallback}`;
    editor.layout.status.appendChild(notice);
  }
  audibleLifecycle.start();
  deps.client.subscribe({ telemetry: true, levels: true, diagnostics: true });
  if (isTauri(win)) void reportSelfCheck(root, win, deps);
  return { ...editor, dispose() { audibleLifecycle.dispose(); editor.dispose(); } };
}

export async function reportSelfCheck(root: HTMLElement, win: Window, deps: EditorDeps): Promise<void> {
  let playingEvents = 0;
  const offPlaying = deps.client.on('playing', () => { playingEvents += 1; });
  try {
    if (!await invoke<boolean>('self_check_enabled')) return;
    let off = () => {};
    const telemetry = new Promise<void>((resolve) => {
      off = deps.client.on('tempo', () => resolve());
    });
    await Promise.race([telemetry, new Promise<void>((resolve) => setTimeout(resolve, 3000))]);
    off();
    const status = root.querySelector<HTMLElement>('[data-pane="code"] [role="status"]');
    const canvas = root.querySelector<HTMLCanvasElement>('[data-pane="code"] canvas');
    const webgl2 = Boolean(canvas?.getContext('webgl2'));
    const gpuStatus = status?.textContent ?? '';
    const latencyKind = deps.store.transportSample?.latency_kind ?? 'unavailable';
    await invoke('self_check', { report: JSON.stringify({
      tier: deps.tier,
      webgl2,
      renderer: deps.code ? 'mounted' : 'absent',
      gpuStatus,
      effectiveDpr: win.devicePixelRatio,
      latencyKind,
      syntax: root.querySelector<HTMLElement>('[data-pane="code"]')?.dataset.syntax ?? 'absent',
      playingEvents,
    }) });
  } catch {
    // Self-check is diagnostic only and must not take the editor down.
  } finally {
    offPlaying();
  }
}

// Page entry: only the editor page carries #vactr-app.
const appRoot = typeof document === 'undefined' ? null : document.getElementById('vactr-app');
if (appRoot) {
  boot(appRoot).catch((e: unknown) => {
    // Design 15.2: boot failures are shown, never only logged.
    buildLayout(appRoot);
    const status = pane(appRoot, 'status');
    status.dataset.state = 'failed';
    render(() => createComponent(BootFailure, { reason: e instanceof Error ? e.message : String(e) }), status);
  });
}
