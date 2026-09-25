// The package pane (design 15.1.10 "UI", 14.5.1).
//
// Browser tier: a proxy URL field (stored in localStorage, no default), the
// unresolved imports taken from `package-not-locked`/`package-not-fetched`
// diagnostics, one "Import" button per import, and the package diagnostics
// (driver errors, then the load diagnostics of the next user-started eval).
// An import runs the driver with the current requirements plus the path at
// `""` (latest) and then shows "imported; evaluate to load": the pane NEVER
// sends `eval`. Native tier: the same list with the exact `vactrol get
// <path>` command text, and no fetching at all.

import type { Tier } from '../app/deps';
import type { Diagnostic } from '../protocol/types';
import {
  importPackages,
  proxyBase,
  restorePackages,
  unresolvedImports,
  type FetchFn,
  type PkgCore,
  type RestoreResult,
  type UnresolvedImport,
} from './driver';
import { MEMORY_ONLY_HINT, type PkgStore } from './opfs';

export type StorageLike = Pick<Storage, 'getItem' | 'setItem'>;

export const PROXY_KEY = 'vactrol.pkg.proxy';
export const IMPORTED_TEXT = 'imported; evaluate to load';

export function defaultStorage(): StorageLike | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
  } catch {
    return null;
  }
}

/** The exact native-tier command for `path`. */
export const getCommand = (path: string): string => `vactrol get ${path}`;

export interface PkgPaneOptions {
  tier: Tier;
  /** Browser tier only. */
  core?: PkgCore;
  fetchFn?: FetchFn;
  store?: PkgStore;
  storage?: StorageLike | null;
}

interface PaneDiag {
  code: string;
  message: string;
}

export class PkgPane {
  readonly el: HTMLElement;
  private readonly doc: Document;
  private readonly opts: PkgPaneOptions;
  private readonly proxyInput: HTMLInputElement;
  private readonly list: HTMLUListElement;
  private readonly statusEl: HTMLElement;
  private readonly hintEl: HTMLElement;
  private readonly diagList: HTMLUListElement;
  private requirements: Record<string, string> = {};
  private unresolved: UnresolvedImport[] = [];
  private loadDiags: PaneDiag[] = [];
  private driverDiags: PaneDiag[] = [];
  private busy = false;

  constructor(doc: Document, opts: PkgPaneOptions) {
    this.doc = doc;
    this.opts = opts;
    const el = doc.createElement('section');
    el.className = 'pkg-pane';
    el.dataset.area = 'pkg';
    const title = doc.createElement('div');
    title.className = 'pkg-title';
    title.textContent = 'Packages';

    const proxyRow = doc.createElement('label');
    proxyRow.className = 'pkg-proxy';
    proxyRow.textContent = 'Proxy ';
    this.proxyInput = doc.createElement('input');
    this.proxyInput.type = 'url';
    this.proxyInput.placeholder = 'https://proxy.example';
    this.proxyInput.dataset.pkg = 'proxy';
    this.proxyInput.value = opts.storage?.getItem(PROXY_KEY) ?? '';
    this.proxyInput.addEventListener('change', () => {
      opts.storage?.setItem(PROXY_KEY, this.proxyInput.value.trim());
    });
    proxyRow.appendChild(this.proxyInput);
    proxyRow.hidden = opts.tier !== 'browser';

    this.list = doc.createElement('ul');
    this.list.className = 'pkg-imports';
    this.list.dataset.pkg = 'imports';
    this.statusEl = doc.createElement('div');
    this.statusEl.className = 'pkg-status';
    this.statusEl.dataset.pkg = 'status';
    this.hintEl = doc.createElement('div');
    this.hintEl.className = 'pkg-hint';
    this.hintEl.dataset.pkg = 'hint';
    this.hintEl.hidden = true;
    this.diagList = doc.createElement('ul');
    this.diagList.className = 'pkg-diagnostics';
    this.diagList.dataset.pkg = 'diagnostics';

    el.append(title, proxyRow, this.hintEl, this.list, this.statusEl, this.diagList);
    this.el = el;
    if (opts.store && !opts.store.persistent) this.showHint(MEMORY_ONLY_HINT);
  }

  get proxy(): string {
    return proxyBase(this.proxyInput.value);
  }

  get currentRequirements(): Readonly<Record<string, string>> {
    return this.requirements;
  }

  get status(): string {
    return this.statusEl.textContent ?? '';
  }

  /** The static and runtime diagnostics the pane derives its lists from. */
  setDiagnostics(diags: readonly Diagnostic[]): void {
    this.unresolved = unresolvedImports(diags);
    this.loadDiags = diags
      .filter((d) => d.code.startsWith('package-'))
      .map((d) => ({ code: d.code, message: d.message }));
    this.render();
  }

  /** Runs the restore once (browser tier with a core and a store). */
  async restore(): Promise<RestoreResult | null> {
    const { core, store, fetchFn } = this.opts;
    if (this.opts.tier !== 'browser' || !core || !store || !fetchFn) return null;
    const saved = await store.load();
    if (saved) this.requirements = { ...saved.requirements };
    const r = await restorePackages(core, store, this.proxy, fetchFn);
    if (r.status === 'done') {
      this.requirements = { ...r.requirements };
      this.setStatus(`restored ${r.resolved.length} package(s)`);
    } else if (r.status === 'no-proxy') {
      this.setStatus('stored packages need a proxy URL to restore');
    } else if (r.status === 'error') {
      this.driverDiags = [{ code: r.code, message: r.message }];
      this.setStatus('restore failed');
      this.render();
    }
    return r;
  }

  /** Imports `path` at `""` (latest) plus the current requirements. */
  async importPath(path: string): Promise<void> {
    const { core, store, fetchFn } = this.opts;
    if (this.opts.tier !== 'browser' || !core || !fetchFn || this.busy) return;
    const proxy = this.proxy;
    if (proxy === '') {
      this.setStatus('set a proxy URL first');
      return;
    }
    this.busy = true;
    this.driverDiags = [];
    this.setStatus(`importing ${path}`);
    this.render();
    try {
      const requirements = { ...this.requirements, [path]: this.requirements[path] ?? '' };
      const r = await importPackages(core, { proxy, requirements }, fetchFn);
      if (r.status === 'done') {
        this.requirements = requirements;
        await store?.save({ requirements, lock: r.lock, bodies: r.bodies });
        this.setStatus(IMPORTED_TEXT);
      } else {
        this.driverDiags = [{ code: r.code, message: r.message }];
        this.setStatus('import failed');
      }
    } catch (e) {
      this.driverDiags = [{ code: 'package-resolve', message: String(e) }];
      this.setStatus('import failed');
    } finally {
      this.busy = false;
      this.render();
    }
  }

  dispose(): void {
    this.el.remove();
  }

  private setStatus(text: string): void {
    this.statusEl.textContent = text;
  }

  private showHint(text: string): void {
    this.hintEl.textContent = text;
    this.hintEl.hidden = false;
  }

  private render(): void {
    const doc = this.doc;
    this.list.replaceChildren(
      ...this.unresolved.map((u) => {
        const li = doc.createElement('li');
        li.dataset.pkgPath = u.path;
        li.title = u.message;
        const name = doc.createElement('span');
        name.className = 'pkg-path';
        name.textContent = u.path;
        li.appendChild(name);
        if (this.opts.tier === 'native') {
          const cmd = doc.createElement('code');
          cmd.dataset.pkg = 'get-command';
          cmd.textContent = getCommand(u.path);
          li.appendChild(cmd);
        } else {
          const btn = doc.createElement('button');
          btn.type = 'button';
          btn.dataset.pkgAction = 'import';
          btn.textContent = 'Import';
          btn.disabled = this.busy;
          btn.addEventListener('click', () => void this.importPath(u.path));
          li.appendChild(btn);
        }
        return li;
      }),
    );
    this.diagList.replaceChildren(
      ...[...this.driverDiags, ...this.loadDiags].map((d) => {
        const li = doc.createElement('li');
        li.dataset.code = d.code;
        li.textContent = `${d.code}: ${d.message}`;
        return li;
      }),
    );
  }
}
