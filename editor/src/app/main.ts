// The editor entry (design 15.1.1, 15.1.3, 15.1.4).
//
// `boot` builds the dependencies from the page URL: `?session=<ws-url>`
// selects the native tier (a WebSocket to `vactrol serve`; the token stays
// in that URL and is never stored), anything else the browser tier (wasm
// #1 in session mode plus the worklet). This is the ONLY place a transport
// is chosen. `createEditor` mounts every area in a fixed order; an area's
// API appears on `deps` when its mount sets it, and consumers read it at
// use time. main makes no other decision.

import { BrowserFiles, TauriFiles, type FileAccess } from '../platform/files';
import { Client } from '../protocol/client';
import { SocketTransport } from '../protocol/socket';
import { Store } from '../protocol/store';
import { WasmCore, WasmTransport } from '../protocol/wasm';
import { mount as mountBind } from '../bind/mount';
import { mount as mountCode } from '../code/mount';
import { mount as mountMidi } from '../midi/mount';
import { mount as mountParams } from '../params/mount';
import { mount as mountPkg } from '../pkg/mount';
import { mount as mountVisual } from '../visual/mount';
import { AudioClock, PageClock } from './clock';
import type { EditorDeps, Mounted, MountFn } from './deps';
import { buildLayout, pane, type Layout } from './layout';
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
  if (choice.tier === 'native') {
    const client = new Client(new SocketTransport(choice.url), { store });
    deps = { client, store, clock: new PageClock(), tier: 'native', files };
  } else {
    const base = win.document.baseURI;
    const core = await WasmCore.start({
      wasmUrl: new URL('vactrol.wasm', base).href,
      processorUrl: new URL('worklet/processor.js', base).href,
    });
    const client = new Client(new WasmTransport(core), { store });
    const ctx = core.host.ctx;
    // Audio may start only after a user gesture.
    root.addEventListener('pointerdown', () => void ctx.resume(), { once: true });
    deps = { client, store, clock: new AudioClock(ctx), tier: 'browser', files, core };
  }
  const editor = createEditor(root, deps);
  deps.client.subscribe({ telemetry: true, levels: true, diagnostics: true });
  return editor;
}

// Page entry: only the editor page carries #vactrol-app.
const appRoot = typeof document === 'undefined' ? null : document.getElementById('vactrol-app');
if (appRoot) {
  boot(appRoot).catch((e: unknown) => {
    // Design 15.2: boot failures are shown, never only logged.
    buildLayout(appRoot);
    const status = pane(appRoot, 'status');
    status.dataset.state = 'failed';
    render(() => createComponent(BootFailure, { reason: e instanceof Error ? e.message : String(e) }), status);
  });
}
