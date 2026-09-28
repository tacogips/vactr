// TASK-010 criterion 3, package half, against the REAL host-wasm artifact
// (`$VACTR_WASM`; design 15.1.10, 15.1.2 G6, 15.1.12). ED-PKG's pane and
// driver (`importPackages`, `restorePackages`) run over a `WasmCore` bound to
// the real exports through `worklet/host.js`, with the editor's `Client`
// over `WasmTransport`. The proxy is `FakeProxy` serving zips built by
// `test/support/zip.ts`. The Rust pipeline validates, digests and publishes;
// nothing here inspects a package body. jsdom environment: the loader's
// `node:fs` read still works, and the pane needs a document.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { VactrHost, type HostExports } from '../../worklet/host.js';
import type { EditorDeps } from '../../src/app/deps';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import type { EvalResultBody, ServerEnvelope } from '../../src/protocol/types';
import { WasmCore, WasmTransport } from '../../src/protocol/wasm';
import { importPackages, restorePackages, unresolvedImports } from '../../src/pkg/driver';
import { mount } from '../../src/pkg/mount';
import { MemoryStore } from '../../src/pkg/opfs';
import { IMPORTED_TEXT, PROXY_KEY, type PkgPane, type StorageLike } from '../../src/pkg/ui';
import { MockClock } from '../support/clock';
import { fakeNode } from '../support/fake-core';
import { FakeProxy } from '../support/fetch';
import { fakeStorage } from '../support/opfs';
import { loadVactrWasm } from '../support/wasm';
import { buildZip } from '../support/zip';

const FILE = 'main.vact';
const PROXY = 'https://proxy.test';
const PADS = 'github.com/test/vactr-pads';
const VERSION = 'v1.0.0';
const DIR = `${PROXY}/${PADS}/@v`;
const MANIFEST = `[package]\npath = "${PADS}"\n\n[deps]\n`;
/** The document: the import, then a form using the package (alias `pads`). */
const DOC = `import ${PADS}\npads.level\n`;

function fixtureProxy(): FakeProxy {
  return new FakeProxy(PROXY).add({
    path: PADS,
    version: VERSION,
    toml: MANIFEST,
    files: [
      { name: 'vactr.toml', data: MANIFEST },
      { name: 'mod.vact', data: 'let level 42\n' },
    ],
  });
}

/** A `WasmCore` over the real artifact, in session mode, behind the real host glue. */
async function realCore(): Promise<WasmCore> {
  const w = await loadVactrWasm();
  const core = new WasmCore();
  const host = new VactrHost(null, fakeNode(), w.exports as HostExports, {
    wasmUrl: '',
    processorUrl: '',
    init: 'session',
    onRecord: core.onRecord,
  });
  core.attach(host);
  expect(host.call('session_init', 48000, 0)).toBe(1);
  return core;
}

function memoryStorage(init: Record<string, string> = {}): StorageLike {
  const m = new Map(Object.entries(init));
  return { getItem: (k) => m.get(k) ?? null, setItem: (k, v) => void m.set(k, v) };
}

interface Editor {
  root: HTMLElement;
  client: Client;
  pane: PkgPane;
}

const disposers: (() => void)[] = [];
afterEach(() => {
  for (const d of disposers.splice(0)) d();
});

/** The browser-tier package pane over the real core; resolves after its startup restore. */
async function editor(proxy: FakeProxy): Promise<Editor> {
  const core = await realCore();
  const store = new Store();
  const client = new Client(new WasmTransport(core), { store });
  const deps: EditorDeps = { client, store, clock: new MockClock(), tier: 'browser', files: new MemoryFiles(), core };
  const root = document.createElement('div');
  document.body.appendChild(root);
  const pane = await new Promise<PkgPane>((resolve) => {
    const m = mount(root, deps, {
      fetchFn: proxy.fetch,
      storage: memoryStorage({ [PROXY_KEY]: PROXY }),
      storageManager: fakeStorage(),
      onReady: resolve,
    });
    disposers.push(() => {
      m.dispose();
      root.remove();
    });
  });
  return { root, client, pane };
}

async function evalDoc(client: Client, code: string): Promise<EvalResultBody> {
  const env: ServerEnvelope = await client.eval(FILE, code);
  expect(env.kind).toBe('eval-result');
  return env.body as EvalResultBody;
}

const listed = (root: HTMLElement): string[] =>
  [...root.querySelectorAll<HTMLElement>('[data-pkg="imports"] li')].map((li) => li.dataset.pkgPath ?? '');
const shownDiags = (root: HTMLElement): string[] =>
  [...root.querySelectorAll('[data-pkg="diagnostics"] li')].map((li) => li.textContent ?? '');
const status = (root: HTMLElement): string => root.querySelector('[data-pkg="status"]')?.textContent ?? '';
const importButton = (root: HTMLElement): HTMLButtonElement | null =>
  root.querySelector<HTMLButtonElement>(`[data-pkg-path="${PADS}"] [data-pkg-action="import"]`);

describe('package import over the real driver (criterion 3, package half)', () => {
  it('an unlocked import is listed; importing it makes the next eval load it with no package-not-locked', async () => {
    const proxy = fixtureProxy();
    const { root, client } = await editor(proxy);

    const before = await evalDoc(client, DOC);
    expect(before.diagnostics.map((d) => d.code)).toContain('package-not-locked');
    expect(unresolvedImports(before.diagnostics).map((u) => u.path)).toEqual([PADS]);
    expect(listed(root)).toEqual([PADS]);

    importButton(root)?.click();
    await vi.waitFor(() => expect(status(root)).toBe(IMPORTED_TEXT));
    expect(proxy.requested).toEqual([`${DIR}/list`, `${DIR}/${VERSION}.toml`, `${DIR}/${VERSION}.zip`]);

    const after = await evalDoc(client, DOC);
    expect(after.diagnostics.filter((d) => d.code.startsWith('package-'))).toEqual([]);
    expect(after.diagnostics.filter((d) => d.severity === 'error')).toEqual([]);
    expect(after.forms.at(-1)?.value).toBe('42');
    expect(listed(root)).toEqual([]);
    expect(shownDiags(root)).toEqual([]);
  });

  it('a tampered zip is package-integrity, shown by the pane; the session stays unlocked', async () => {
    const proxy = fixtureProxy();
    // A path-traversal entry: the Rust validation rejects the archive before any digest.
    proxy.serve(
      `${DIR}/${VERSION}.zip`,
      buildZip([
        { name: 'vactr.toml', data: MANIFEST },
        { name: '../evil.vact', data: 'let level 1\n' },
      ]),
    );
    const { root, client } = await editor(proxy);
    await evalDoc(client, DOC);
    importButton(root)?.click();
    await vi.waitFor(() => expect(status(root)).toBe('import failed'));
    // Beside the eval's own `package-not-locked`, the driver error is shown.
    const integrity = shownDiags(root).filter((t) => t.startsWith('package-integrity: '));
    expect(integrity).toHaveLength(1);
    expect(integrity[0]).toContain('evil.vact');
    expect(shownDiags(root).filter((t) => !t.startsWith('package-integrity: '))).toEqual(
      shownDiags(root).filter((t) => t.startsWith('package-not-locked: ')),
    );
    const again = await evalDoc(client, DOC);
    expect(again.diagnostics.map((d) => d.code)).toContain('package-not-locked');
  });

  it('restore of a stored lock with a tampered body is package-integrity and deletes the stored entries', async () => {
    const proxy = fixtureProxy();
    const first = await realCore();
    const done = await importPackages(first, { proxy: PROXY, requirements: { [PADS]: VERSION } }, proxy.fetch);
    expect(done.status).toBe('done');
    if (done.status !== 'done') return;
    expect(done.resolved.map((r) => [r.path, r.version])).toEqual([[PADS, VERSION]]);
    expect(done.lock).toContain(done.resolved[0]?.sha256 ?? '-');

    // The stored zip body is replaced by a different, still valid archive.
    const zipUrl = `${DIR}/${VERSION}.zip`;
    const bodies = new Map(done.bodies);
    bodies.set(zipUrl, buildZip([{ name: 'vactr.toml', data: `${MANIFEST}# tampered\n` }]));
    const store = new MemoryStore();
    await store.save({ requirements: { [PADS]: VERSION }, lock: done.lock, bodies });

    const second = await realCore();
    const fetched = proxy.requested.length;
    const r = await restorePackages(second, store, PROXY, proxy.fetch);
    expect(r).toMatchObject({ status: 'error', code: 'package-integrity' });
    // Only stored bodies were supplied; nothing was fetched.
    expect(proxy.requested.length).toBe(fetched);
    const left = await store.load();
    expect(left?.lock).toBeNull();
    expect(left?.bodies.has(zipUrl)).toBe(false);
    if (r.status === 'error') expect(r.removed).toContain(zipUrl);
  });
});
