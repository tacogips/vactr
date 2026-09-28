// The package pane (design 15.1.10 "UI", 14.5.1).
//
// Browser tier: a proxy URL field (stored in localStorage, no default), the
// unresolved imports taken from `package-not-locked`/`package-not-fetched`
// diagnostics, one "Import" button per import, and the package diagnostics
// (driver errors, then the load diagnostics of the next user-started eval).
// An import runs the driver with the current requirements plus the path at
// `""` (latest) and then shows "imported; evaluate to load": the pane NEVER
// sends `eval`. Native tier: the same list with the exact `vactr get
// <path>` command text, and no fetching at all.

import { createComponent, createSignal, type Setter } from 'solid-js';
import { render } from 'solid-js/web';
import { PkgView, type PkgModel } from './pkg-view';
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

export const PROXY_KEY = 'vactr.pkg.proxy';
export const IMPORTED_TEXT = 'imported; evaluate to load';

export function defaultStorage(): StorageLike | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
  } catch {
    return null;
  }
}

/** The exact native-tier command for `path`. */
export const getCommand = (path: string): string => `vactr get ${path}`;

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
  private readonly opts: PkgPaneOptions;
  private readonly proxyInput: HTMLInputElement;
  private readonly setModel: Setter<PkgModel>;
  private readonly disposeView: () => void;
  private statusText = '';
  private hintText = '';
  private requirements: Record<string, string> = {};
  private unresolved: UnresolvedImport[] = [];
  private loadDiags: PaneDiag[] = [];
  private driverDiags: PaneDiag[] = [];
  private busy = false;

  constructor(doc: Document, opts: PkgPaneOptions) {
    this.opts = opts;
    const holder = doc.createElement('div');
    const [model, setModel] = createSignal<PkgModel>(this.model());
    this.setModel = setModel;
    this.disposeView = render(() => createComponent(PkgView, {
      tier: opts.tier,
      proxy: opts.storage?.getItem(PROXY_KEY) ?? '',
      model,
      onProxy: (value) => opts.storage?.setItem(PROXY_KEY, value),
      onImport: (path) => void this.importPath(path),
    }), holder);
    this.el = holder.firstElementChild as HTMLElement;
    this.proxyInput = this.el.querySelector<HTMLInputElement>('[data-pkg="proxy"]')!;
    if (opts.store && !opts.store.persistent) this.showHint(MEMORY_ONLY_HINT);
  }

  get proxy(): string {
    return proxyBase(this.proxyInput.value);
  }

  get currentRequirements(): Readonly<Record<string, string>> {
    return this.requirements;
  }

  get status(): string {
    return this.statusText;
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
    this.disposeView();
    this.el.remove();
  }

  private setStatus(text: string): void {
    this.statusText = text;
    this.render();
  }

  private showHint(text: string): void {
    this.hintText = text;
    this.render();
  }

  private model(): PkgModel {
    return {
      unresolved: [...this.unresolved],
      diagnostics: [...this.driverDiags, ...this.loadDiags],
      busy: this.busy,
      status: this.statusText,
      hint: this.hintText,
    };
  }

  private render(): void {
    this.setModel(this.model());
  }
}
