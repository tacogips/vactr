// The `pkg` area (design 15.1.3, 15.1.10): the package pane in the right
// pane. It follows the static diagnostics of every file's latest
// `eval-result` plus the runtime diagnostics, and on the browser tier runs
// the OPFS restore once. Package content reaches the session only through
// the driver (`pkgSupply` + `pkgResolve`); mounting sends no protocol
// message and never evaluates.

import type { EditorDeps, Mounted } from '../app/deps';
import { buildLayout } from '../app/layout';
import type { Diagnostic } from '../protocol/types';
import type { FetchFn, PkgCore } from './driver';
import { openPkgStore, type PkgStore, type StorageManagerLike } from './opfs';
import { PkgPane, defaultStorage, type StorageLike } from './ui';

// The pane's stylesheet as a Vite asset URL (a side-effect CSS import fails
// `tsc` without a CSS module declaration).
const STYLESHEET = new URL('./pkg.css', import.meta.url).href;

function addStylesheet(doc: Document): HTMLLinkElement | null {
  if (doc.head.querySelector('link[data-style="pkg"]')) return null;
  const link = doc.createElement('link');
  link.rel = 'stylesheet';
  link.href = STYLESHEET;
  link.dataset.style = 'pkg';
  doc.head.appendChild(link);
  return link;
}

function defaultFetch(): FetchFn | undefined {
  return typeof fetch === 'function' ? (url) => fetch(url) : undefined;
}

/** Test seams; the defaults are the page's fetch, localStorage and OPFS. */
export interface PkgMountOptions {
  core?: PkgCore;
  fetchFn?: FetchFn;
  storage?: StorageLike | null;
  storageManager?: StorageManagerLike | null;
  store?: PkgStore;
  /** Settles when the startup restore has finished (tests). */
  onReady?: (pane: PkgPane) => void;
}

export function mount(root: HTMLElement, deps: EditorDeps, opts: PkgMountOptions = {}): Mounted {
  const doc = root.ownerDocument;
  const stylesheet = addStylesheet(doc);
  const browser = deps.tier === 'browser';
  const core = browser ? (opts.core ?? deps.core) : undefined;
  const fetchFn = browser ? (opts.fetchFn ?? defaultFetch()) : undefined;
  const storage = opts.storage === undefined ? defaultStorage() : opts.storage;
  let disposed = false;
  let pane: PkgPane | null = null;
  const files = new Set<string>();

  const diagnostics = (): Diagnostic[] => {
    const out: Diagnostic[] = [];
    for (const f of files) out.push(...deps.store.diagnostics(f));
    for (const list of deps.store.runtimeDiagnostics().values()) out.push(...list);
    return out;
  };
  const refresh = (): void => pane?.setDiagnostics(diagnostics());

  // The store has applied the eval-result before client listeners run.
  const offEval = deps.client.on('eval-result', (env) => {
    if (env.kind !== 'eval-result') return;
    files.add(env.body.file);
    refresh();
  });
  const offStore = deps.store.subscribe(['diag'], refresh);

  const section = doc.createElement('div');
  section.className = 'pkg-area';
  buildLayout(root).right.appendChild(section);

  const start = async (): Promise<void> => {
    const store = browser ? (opts.store ?? (await openPkgStore(opts.storageManager))) : undefined;
    if (disposed) return;
    const p = new PkgPane(doc, { tier: deps.tier, core, fetchFn, store, storage });
    pane = p;
    section.appendChild(p.el);
    refresh();
    await p.restore();
    if (!disposed) opts.onReady?.(p);
  };
  void start().catch((e: unknown) => {
    if (!disposed) section.textContent = `packages unavailable: ${String(e)}`;
  });

  return {
    dispose() {
      disposed = true;
      offEval();
      offStore();
      pane?.dispose();
      section.remove();
      stylesheet?.remove();
    },
  };
}
