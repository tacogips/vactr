import { afterEach, describe, expect, it, vi } from 'vitest';
import { VactrolHost } from '../../worklet/host.js';
import type { EditorDeps } from '../../src/app/deps';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { WasmCore } from '../../src/protocol/wasm';
import type { Diagnostic, PkgReply, PkgResolveRequest } from '../../src/protocol/types';
import { mount } from '../../src/pkg/mount';
import { IMPORTED_TEXT, PROXY_KEY, type PkgPane, type StorageLike } from '../../src/pkg/ui';
import { MockClock } from '../support/clock';
import { FakeCore, fakeNode } from '../support/fake-core';
import { FakeProxy } from '../support/fetch';
import { fakeStorage } from '../support/opfs';
import { RecordingTransport } from '../support/recording';

const PROXY = 'https://proxy.test';
const PADS = 'github.com/test/vactrol-pads';
const dec = new TextDecoder();

/**
 * A stand-in for the Rust driver (G6): for each required path it needs
 * `@v/list`, then `@v/<v>.toml` and `@v/<v>.zip`, answering from the bodies
 * supplied so far; a zip whose bytes contain "TAMPERED" is
 * `package-integrity`.
 */
function simulatedResolver(fake: FakeCore): void {
  const supplied = new Map<string, { status: number; body: Uint8Array }>();
  fake.on('pkg_supply', (c) => {
    supplied.set(c.text ?? '', { status: Number(c.args[2]), body: c.bytes ?? new Uint8Array(0) });
  });
  const reply = (req: PkgResolveRequest): PkgReply => {
    if (!('requirements' in req)) return { status: 'error', code: 'package-resolve', message: 'no lock expected' };
    const resolved = [];
    for (const path of Object.keys(req.requirements)) {
      const dir = `${req.proxy}/${path}/@v`;
      const list = supplied.get(`${dir}/list`);
      if (!list) return { status: 'need', url: `${dir}/list` };
      if (list.status !== 200) return { status: 'error', code: 'package-resolve', message: `\`${path}\` not found` };
      const version = dec.decode(list.body).trim().split('\n').pop() as string;
      for (const ext of ['toml', 'zip']) {
        if (!supplied.has(`${dir}/${version}.${ext}`)) return { status: 'need', url: `${dir}/${version}.${ext}` };
      }
      const zip = supplied.get(`${dir}/${version}.zip`)?.body ?? new Uint8Array(0);
      if (dec.decode(zip).includes('TAMPERED')) {
        return {
          status: 'error',
          code: 'package-integrity',
          message: `unsafe package entry \`${path}@${version}\`: digest mismatch`,
        };
      }
      resolved.push({ path, version, sha256: 'ab'.repeat(32) });
    }
    return { status: 'done', lock: resolved.map((r) => `${r.path} ${r.version}`).join('\n'), resolved };
  };
  fake.on('pkg_resolve', (c) => {
    fake.emit(0x73, JSON.stringify(reply(JSON.parse(c.text ?? '{}') as PkgResolveRequest)));
  });
}

function memoryStorage(init: Record<string, string> = {}): StorageLike {
  const m = new Map(Object.entries(init));
  return { getItem: (k) => m.get(k) ?? null, setItem: (k, v) => void m.set(k, v) };
}

const diag = (code: string, message: string): Diagnostic => ({
  code,
  severity: code === 'package-not-fetched' ? 'warning' : 'error',
  message,
  span: { start: 0, end: 10 },
  file: 'main.vact',
});

function evalResult(diagnostics: Diagnostic[]) {
  return {
    kind: 'eval-result' as const,
    body: {
      file: 'main.vact',
      doc_revision: 1,
      forms: [],
      diagnostics,
      sites: [],
      directives: { file_level: {}, entries: [] },
    },
  };
}

const NOT_LOCKED = diag('package-not-locked', `package \`${PADS}\` is not in \`vactrol.lock\``);

interface Setup {
  root: HTMLElement;
  transport: RecordingTransport;
  proxy: FakeProxy;
  fake: FakeCore;
  pane: PkgPane;
}

const disposers: (() => void)[] = [];
afterEach(() => {
  for (const d of disposers.splice(0)) d();
});

async function setup(tier: 'browser' | 'native', proxy = new FakeProxy(PROXY)): Promise<Setup> {
  proxy.add({
    path: PADS,
    version: '1.0.0',
    toml: 'name = "vactrol-pads"\n',
    files: [{ name: 'pads.vact', data: '(def pad 1)\n' }],
  });
  const root = document.createElement('div');
  document.body.appendChild(root);
  const store = new Store();
  const transport = new RecordingTransport();
  const fake = new FakeCore();
  simulatedResolver(fake);
  const core = new WasmCore();
  core.attach(
    new VactrolHost(null, fakeNode(), fake.exports, { wasmUrl: '', processorUrl: '', init: 'session', onRecord: core.onRecord }),
  );
  const deps: EditorDeps = {
    client: new Client(transport, { store }),
    store,
    clock: new MockClock(),
    tier,
    files: new MemoryFiles(),
  };
  if (tier === 'browser') deps.core = core;
  const ready = new Promise<PkgPane>((resolve) => {
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
  const pane = await ready;
  return { root, transport, proxy, fake, pane };
}

const imports = (root: HTMLElement): string[] =>
  [...root.querySelectorAll<HTMLElement>('[data-pkg="imports"] li')].map((li) => li.dataset.pkgPath ?? '');
const shownDiags = (root: HTMLElement): string[] =>
  [...root.querySelectorAll('[data-pkg="diagnostics"] li')].map((li) => li.textContent ?? '');
const status = (root: HTMLElement): string => root.querySelector('[data-pkg="status"]')?.textContent ?? '';

describe('package pane (criterion 3, package half)', () => {
  it('lists the import, imports it via the proxy, never evaluates, then shows load diagnostics', async () => {
    const { root, transport, proxy, fake } = await setup('browser');
    expect(root.querySelector('[data-pane="right"] [data-area="pkg"]')).not.toBeNull();
    expect(imports(root)).toEqual([]);

    transport.emit(evalResult([NOT_LOCKED]));
    expect(imports(root)).toEqual([PADS]);

    root.querySelector<HTMLButtonElement>(`[data-pkg-path="${PADS}"] [data-pkg-action="import"]`)?.click();
    await vi.waitFor(() => expect(status(root)).toBe(IMPORTED_TEXT));
    expect(proxy.requested).toEqual([
      `${PROXY}/${PADS}/@v/list`,
      `${PROXY}/${PADS}/@v/1.0.0.toml`,
      `${PROXY}/${PADS}/@v/1.0.0.zip`,
    ]);
    const first = JSON.parse(fake.callsOf('pkg_resolve')[0]?.text ?? '') as unknown;
    expect(first).toEqual({ proxy: PROXY, requirements: { [PADS]: '' } });
    // The pane never sends eval (or anything else) itself.
    expect(transport.kinds()).not.toContain('eval');
    expect(transport.sent).toEqual([]);

    // The user's next eval brings the load diagnostics.
    transport.emit(
      evalResult([diag('package-load-failed', `package \`${PADS}\`: pads.vact:1: unbound \`pad2\``)]),
    );
    expect(imports(root)).toEqual([]);
    expect(shownDiags(root)).toEqual([`package-load-failed: package \`${PADS}\`: pads.vact:1: unbound \`pad2\``]);
  });

  it('shows a driver package-integrity error', async () => {
    const proxy = new FakeProxy(PROXY);
    const { root, transport } = await setup('browser', proxy);
    proxy.serve(`${PROXY}/${PADS}/@v/1.0.0.zip`, 'TAMPERED');
    transport.emit(evalResult([NOT_LOCKED]));
    root.querySelector<HTMLButtonElement>('[data-pkg-action="import"]')?.click();
    await vi.waitFor(() => expect(status(root)).toBe('import failed'));
    expect(shownDiags(root)).toContain(`package-integrity: unsafe package entry \`${PADS}@1.0.0\`: digest mismatch`);
    expect(transport.sent).toEqual([]);
  });

  it('native tier: shows `vactrol get <path>` and performs no fetch', async () => {
    const { root, transport, proxy, fake } = await setup('native');
    transport.emit(evalResult([NOT_LOCKED]));
    expect(imports(root)).toEqual([PADS]);
    const cmd = root.querySelector('[data-pkg="get-command"]');
    expect(cmd?.textContent).toBe(`vactrol get ${PADS}`);
    expect(root.querySelector('[data-pkg-action="import"]')).toBeNull();
    expect(root.querySelector<HTMLElement>('[data-pkg="proxy"]')?.closest('label')?.hidden).toBe(true);
    expect(proxy.requested).toEqual([]);
    expect(fake.calls).toEqual([]);
    expect(transport.sent).toEqual([]);
  });
});
