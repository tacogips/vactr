import { describe, expect, it } from 'vitest';
import { VactrHost } from '../../worklet/host.js';
import { WasmCore } from '../../src/protocol/wasm';
import type { PkgReply } from '../../src/protocol/types';
import {
  MAX_FETCHES,
  importPackages,
  importPath,
  offendingUrls,
  underProxy,
  unresolvedImports,
} from '../../src/pkg/driver';
import { FakeCore, fakeNode } from '../support/fake-core';
import { FakeProxy } from '../support/fetch';

const PROXY = 'https://proxy.test';

/** A WasmCore over fake exports whose `pkg_resolve` answers from `script`. */
function scripted(script: (step: number, req: string) => PkgReply): { fake: FakeCore; core: WasmCore } {
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
  let step = 0;
  fake.on('pkg_resolve', (c) => {
    fake.emit(0x73, JSON.stringify(script(step, c.text ?? '')));
    step += 1;
  });
  return { fake, core };
}

const supplies = (fake: FakeCore): [string | undefined, unknown, number][] =>
  fake.callsOf('pkg_supply').map((c) => [c.text, c.args[2], c.bytes?.length ?? -1]);

describe('importPackages', () => {
  it('fetches only the requested URLs, in order, and supplies 200 / 404 / network', async () => {
    const a = `${PROXY}/github.com/test/a/@v/list`;
    const b = `${PROXY}/github.com/test/b/@v/list`;
    const c = `${PROXY}/github.com/test/c/@v/list`;
    const d = `${PROXY}/github.com/test/d/@v/list`;
    const proxy = new FakeProxy(PROXY).serve(a, '1.0.0\n').fail(c).status(d, 500);
    const needs = [a, b, c, d];
    const { fake, core } = scripted((i) =>
      i < needs.length
        ? { status: 'need', url: needs[i] as string }
        : { status: 'done', lock: 'LOCK', resolved: [{ path: 'github.com/test/a', version: '1.0.0', sha256: 'ab' }] },
    );
    const req = { proxy: PROXY, requirements: { 'github.com/test/a': '' } };
    const r = await importPackages(core, req, proxy.fetch);

    expect(proxy.requested).toEqual(needs);
    expect(supplies(fake)).toEqual([
      [a, 200, 6],
      [b, 404, 0],
      [c, 0, 0],
      [d, 0, 0],
    ]);
    // pkgResolve is re-run with the same request after each supply.
    const resolves = fake.callsOf('pkg_resolve').map((x) => JSON.parse(x.text ?? '') as unknown);
    expect(resolves).toEqual([req, req, req, req, req]);
    expect(r.status).toBe('done');
    if (r.status !== 'done') return;
    expect(r.lock).toBe('LOCK');
    expect(r.resolved).toHaveLength(1);
    // Only 200 bodies are recorded for OPFS.
    expect([...r.bodies.keys()]).toEqual([a]);
  });

  it('returns the driver error with its code and message', async () => {
    const url = `${PROXY}/github.com/test/a/@v/list`;
    const proxy = new FakeProxy(PROXY).serve(url, '1.0.0\n');
    const { core } = scripted((i) =>
      i === 0 ? { status: 'need', url } : { status: 'error', code: 'package-resolve', message: 'no version' },
    );
    const r = await importPackages(core, { proxy: PROXY, requirements: {} }, proxy.fetch);
    expect(r).toMatchObject({ status: 'error', code: 'package-resolve', message: 'no version' });
    expect(proxy.requested).toEqual([url]);
  });

  it('refuses a URL outside the proxy without fetching or supplying', async () => {
    const proxy = new FakeProxy(PROXY);
    for (const url of ['https://evil.test/x', 'https://proxy.test.evil/x', 'http://proxy.test/x']) {
      const { fake, core } = scripted(() => ({ status: 'need', url }));
      const r = await importPackages(core, { proxy: PROXY, requirements: {} }, proxy.fetch);
      expect(r.status).toBe('error');
      if (r.status === 'error') {
        expect(r.code).toBe('package-resolve');
        expect(r.message).toContain('not under the proxy');
      }
      expect(fake.callsOf('pkg_supply')).toEqual([]);
    }
    expect(proxy.requested).toEqual([]);
  });

  it('stops after MAX_FETCHES with "too many fetches"', async () => {
    const proxy = new FakeProxy(PROXY);
    const { fake, core } = scripted((i) => ({ status: 'need', url: `${PROXY}/p/@v/${i}.zip` }));
    const r = await importPackages(core, { proxy: PROXY, requirements: {} }, proxy.fetch);
    expect(MAX_FETCHES).toBe(256);
    expect(r).toMatchObject({ status: 'error', code: 'package-resolve', message: 'too many fetches' });
    expect(proxy.requested).toHaveLength(MAX_FETCHES);
    expect(fake.callsOf('pkg_supply')).toHaveLength(MAX_FETCHES);
  });
});

describe('underProxy', () => {
  it('accepts only same-origin URLs under the proxy path after normalization', () => {
    const base = 'https://proxy.test/base/';
    expect(underProxy(base, 'https://proxy.test/base/github.com/x/@v/list')).toBe(true);
    expect(underProxy(base, 'https://proxy.test/base/../other/x')).toBe(false);
    expect(underProxy(base, 'https://proxy.test/basement/x')).toBe(false);
    expect(underProxy(base, 'https://proxy.test.evil/base/x')).toBe(false);
    expect(underProxy('file:///tmp', 'file:///tmp/x')).toBe(false);
    expect(underProxy('', 'https://proxy.test/x')).toBe(false);
  });
});

describe('unresolvedImports', () => {
  // Rule: the first backticked token of the message, cut at its first `@`.
  it('parses the path from package-not-locked / package-not-fetched messages', () => {
    const d = (code: string, message: string) => ({
      code,
      severity: 'error' as const,
      message,
      span: { start: 0, end: 1 },
      file: 'main.vact',
    });
    const diags = [
      d('package-not-locked', 'package `github.com/test/vactr-pads` is not in `vactr.lock`'),
      d('package-not-fetched', '`github.com/test/drums@1.2.0` is locked but not fetched; run `vactr get`'),
      d('package-not-locked', 'package `github.com/test/vactr-pads` is not in `vactr.lock`'),
      d('unbound', 'unbound `x`'),
      d('package-not-locked', 'no path here'),
    ];
    expect(unresolvedImports(diags).map((u) => u.path)).toEqual([
      'github.com/test/vactr-pads',
      'github.com/test/drums',
    ]);
    expect(importPath('package `a/b@` x')).toBe('a/b');
    expect(importPath('nothing')).toBeNull();
  });

  it('blames the stored bodies of the named package@version, else all', () => {
    const urls = [
      `${PROXY}/github.com/test/pads/@v/1.0.0.zip`,
      `${PROXY}/github.com/test/pads/@v/1.0.0.toml`,
      `${PROXY}/github.com/test/drums/@v/2.0.0.zip`,
    ];
    expect(offendingUrls('unsafe package entry `github.com/test/pads@1.0.0`: digest mismatch', urls)).toEqual(
      urls.slice(0, 2),
    );
    expect(offendingUrls('unsafe package entry `../evil`: escapes the root', urls)).toEqual(urls);
  });
});
