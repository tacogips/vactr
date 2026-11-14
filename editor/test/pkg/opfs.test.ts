import { describe, expect, it } from 'vitest';
import { VactrHost } from '../../worklet/host.js';
import { WasmCore } from '../../src/protocol/wasm';
import type { PkgReply } from '../../src/protocol/types';
import { restorePackages } from '../../src/pkg/driver';
import {
  BODIES_DIR,
  DIR_NAME,
  INDEX_FILE,
  LOCK_FILE,
  MEMORY_ONLY_HINT,
  MemoryStore,
  OpfsStore,
  REQUIREMENTS_FILE,
  openPkgStore,
  sha256Hex,
} from '../../src/pkg/opfs';
import { PkgPane } from '../../src/pkg/ui';
import { FakeCore, fakeNode } from '../support/fake-core';
import { FakeProxy } from '../support/fetch';
import { FakeDir, fakeStorage } from '../support/opfs';

const PROXY = 'https://proxy.test';
const PADS_ZIP = `${PROXY}/github.com/test/vactr-pads/@v/1.0.0.zip`;
const PADS_TOML = `${PROXY}/github.com/test/vactr-pads/@v/1.0.0.toml`;
const DRUMS_ZIP = `${PROXY}/github.com/test/drums/@v/2.0.0.zip`;
const LOCK = 'github.com/test/vactr-pads 1.0.0 aa\ngithub.com/test/drums 2.0.0 bb\n';
const enc = new TextEncoder();

function scripted(reply: (req: string) => PkgReply): { fake: FakeCore; core: WasmCore } {
  const fake = new FakeCore();
  const core = new WasmCore();
  core.attach(
    new VactrHost(null, fakeNode(), fake.exports, {
      wasmUrl: '',
      processorUrl: '',
      init: 'session',
      onRecord: core.onRecord,
    }),
  );
  fake.on('pkg_resolve', (c) => fake.emit(0x73, JSON.stringify(reply(c.text ?? ''))));
  return { fake, core };
}

async function seeded(): Promise<{ root: FakeDir; store: OpfsStore }> {
  const storage = fakeStorage();
  const store = (await openPkgStore(storage)) as OpfsStore;
  await store.save({
    requirements: { 'github.com/test/vactr-pads': '', 'github.com/test/drums': '2.0.0' },
    lock: LOCK,
    bodies: new Map([
      [PADS_ZIP, enc.encode('pads-zip')],
      [PADS_TOML, enc.encode('pads-toml')],
      [DRUMS_ZIP, enc.encode('drums-zip')],
    ]),
  });
  return { root: storage.root, store };
}

describe('OpfsStore', () => {
  it('round-trips requirements, lock and bodies under vactr-pkg/', async () => {
    const { root, store } = await seeded();
    expect(store.persistent).toBe(true);
    expect(root.text(DIR_NAME, LOCK_FILE)).toBe(LOCK);
    expect(JSON.parse(root.text(DIR_NAME, REQUIREMENTS_FILE) ?? '')).toEqual({
      'github.com/test/vactr-pads': '',
      'github.com/test/drums': '2.0.0',
    });
    const index = JSON.parse(root.text(DIR_NAME, INDEX_FILE) ?? '') as Record<string, string>;
    expect(index[PADS_ZIP]).toBe(await sha256Hex(PADS_ZIP));
    expect(index[PADS_ZIP]).toMatch(/^[0-9a-f]{64}$/);
    expect(root.text(DIR_NAME, BODIES_DIR, index[PADS_ZIP] as string)).toBe('pads-zip');

    // A fresh store over the same directory sees the same content.
    const again = await openPkgStore(fakeStorage(root));
    const loaded = await again.load();
    expect(loaded?.lock).toBe(LOCK);
    expect(loaded?.requirements).toEqual({ 'github.com/test/vactr-pads': '', 'github.com/test/drums': '2.0.0' });
    expect([...(loaded?.bodies.keys() ?? [])].sort()).toEqual([DRUMS_ZIP, PADS_TOML, PADS_ZIP].sort());
    expect(new TextDecoder().decode(loaded?.bodies.get(DRUMS_ZIP))).toBe('drums-zip');

    await again.remove(DRUMS_ZIP);
    expect((await again.load())?.bodies.has(DRUMS_ZIP)).toBe(false);
    expect(await openPkgStore(fakeStorage()).then((s) => s.load())).toBeNull();
  });
});

describe('restorePackages', () => {
  it('supplies every stored body before pkgResolve({proxy, lock})', async () => {
    const { store } = await seeded();
    const { fake, core } = scripted(() => ({ status: 'done', lock: LOCK, resolved: [] }));
    const proxy = new FakeProxy(PROXY);
    const r = await restorePackages(core, store, PROXY, proxy.fetch);
    expect(r.status).toBe('done');
    const order = fake.calls.filter((c) => c.name === 'pkg_supply' || c.name === 'pkg_resolve');
    expect(order.map((c) => c.name)).toEqual(['pkg_supply', 'pkg_supply', 'pkg_supply', 'pkg_resolve']);
    expect(order.slice(0, 3).map((c) => [c.text, c.args[2]])).toEqual(
      expect.arrayContaining([
        [PADS_ZIP, 200],
        [PADS_TOML, 200],
        [DRUMS_ZIP, 200],
      ]),
    );
    expect(new TextDecoder().decode(order[0]?.bytes)).toMatch(/-(zip|toml)$/);
    expect(JSON.parse(order[3]?.text ?? '')).toEqual({ proxy: PROXY, lock: LOCK });
    // Nothing was fetched: every body came from OPFS.
    expect(proxy.requested).toEqual([]);
  });

  it('deletes the offending stored bodies and the lock on package-integrity', async () => {
    const { root, store } = await seeded();
    const message = 'unsafe package entry `github.com/test/vactr-pads@1.0.0`: digest mismatch: expected aa, got cc';
    const { core } = scripted(() => ({ status: 'error', code: 'package-integrity', message }));
    const r = await restorePackages(core, store, PROXY, new FakeProxy(PROXY).fetch);
    expect(r).toMatchObject({ status: 'error', code: 'package-integrity', message });
    if (r.status === 'error') expect(r.removed.sort()).toEqual([PADS_TOML, PADS_ZIP].sort());
    expect(root.text(DIR_NAME, LOCK_FILE)).toBeUndefined();
    const bodies = root.dir(DIR_NAME, BODIES_DIR);
    expect(bodies?.files.has(await sha256Hex(PADS_ZIP))).toBe(false);
    expect(bodies?.files.has(await sha256Hex(PADS_TOML))).toBe(false);
    expect(bodies?.files.has(await sha256Hex(DRUMS_ZIP))).toBe(true);
    const index = JSON.parse(root.text(DIR_NAME, INDEX_FILE) ?? '') as Record<string, string>;
    expect(Object.keys(index)).toEqual([DRUMS_ZIP]);
    // A later restore finds no lock and does nothing.
    const again = scripted(() => ({ status: 'done', lock: LOCK, resolved: [] }));
    expect(await restorePackages(again.core, store, PROXY, new FakeProxy(PROXY).fetch)).toEqual({ status: 'empty' });
    expect(again.fake.callsOf('pkg_resolve')).toEqual([]);
  });

  it('does not restore without a proxy URL', async () => {
    const { store } = await seeded();
    const { fake, core } = scripted(() => ({ status: 'done', lock: LOCK, resolved: [] }));
    expect(await restorePackages(core, store, '', new FakeProxy(PROXY).fetch)).toEqual({ status: 'no-proxy' });
    expect(fake.calls).toEqual([]);
  });
});

describe('memory-only fallback', () => {
  it('uses a memory store when OPFS is absent and the pane shows the hint', async () => {
    for (const storage of [null, {}, { getDirectory: () => Promise.reject(new Error('SecurityError')) }]) {
      const store = await openPkgStore(storage);
      expect(store).toBeInstanceOf(MemoryStore);
      expect(store.persistent).toBe(false);
    }
    const store = await openPkgStore(null);
    expect(await store.load()).toBeNull();
    await store.save({ requirements: { a: '' }, lock: 'L', bodies: new Map([[PADS_ZIP, enc.encode('x')]]) });
    expect((await store.load())?.lock).toBe('L');
    await store.removeLock();
    expect((await store.load())?.lock).toBeNull();

    const pane = new PkgPane(document, { tier: 'browser', store });
    const hint = pane.el.querySelector<HTMLElement>('[data-pkg="hint"]');
    expect(hint?.hidden).toBe(false);
    expect(hint?.textContent).toBe(MEMORY_ONLY_HINT);
    const persistent = new PkgPane(document, { tier: 'browser', store: new OpfsStore(new FakeDir()) });
    expect(persistent.el.querySelector<HTMLElement>('[data-pkg="hint"]')?.hidden).toBe(true);
  });
});
