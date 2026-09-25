// The browser package driver (design 15.1.10, 15.1.2 G6; command.md
// `pkg_resolve`/`pkg_supply`, `0x73`).
//
// The Rust side validates, digests and publishes; this file is only the IO
// loop: `pkgResolve` answers `need <url>`, the driver fetches that URL from
// the configured proxy and hands the body back through `pkgSupply`, and the
// loop repeats until `done` or `error`. Package content reaches the session
// ONLY through `pkgSupply` + `pkgResolve`; nothing here inspects a body.
// Network requests go only to URLs under the configured proxy.

import type { Diagnostic, PkgReply, PkgResolveRequest, PkgResolved } from '../protocol/types';
import type { PkgStore } from './opfs';

/** The slice of `WasmCore` the driver uses. */
export interface PkgCore {
  pkgResolve(req: PkgResolveRequest): void;
  pkgSupply(url: string, status: number, body: Uint8Array): void;
  onPkg(cb: (r: PkgReply) => void): () => void;
}

export interface FetchResponseLike {
  status: number;
  arrayBuffer(): Promise<ArrayBuffer>;
}

export type FetchFn = (url: string) => Promise<FetchResponseLike>;

export type DriverInput = PkgResolveRequest;

export type DriverResult =
  | { status: 'done'; lock: string; resolved: PkgResolved[]; bodies: Map<string, Uint8Array> }
  | { status: 'error'; code: string; message: string; bodies: Map<string, Uint8Array> };

/** At most this many fetches per driver run. */
export const MAX_FETCHES = 256;

/** `pkg_supply` status of a network failure (any status but 200 and 404). */
export const STATUS_NETWORK = 0;

const EMPTY = new Uint8Array(0);

/** The proxy base without trailing slashes. */
export function proxyBase(proxy: string): string {
  return proxy.trim().replace(/\/+$/, '');
}

/**
 * Whether `url` lies under the proxy: same origin and, after URL
 * normalization (so `..` segments cannot escape), a path under the proxy's
 * path. Only http(s) proxies qualify.
 */
export function underProxy(proxy: string, url: string): boolean {
  let base: URL;
  let target: URL;
  try {
    base = new URL(`${proxyBase(proxy)}/`);
    target = new URL(url);
  } catch {
    return false;
  }
  if (base.protocol !== 'http:' && base.protocol !== 'https:') return false;
  return target.origin === base.origin && target.href.startsWith(base.href);
}

/** Calls `step` and resolves with the next `0x73` reply (synchronous or later). */
function nextReply(core: PkgCore, step: () => void): Promise<PkgReply> {
  return new Promise((resolve, reject) => {
    let settled = false;
    const off = core.onPkg((r) => {
      if (settled) return;
      settled = true;
      off();
      resolve(r);
    });
    try {
      step();
    } catch (e) {
      if (!settled) {
        settled = true;
        off();
        reject(e instanceof Error ? e : new Error(String(e)));
      }
    }
  });
}

async function fetchOne(fetchFn: FetchFn, url: string): Promise<{ status: number; body: Uint8Array }> {
  try {
    const res = await fetchFn(url);
    if (res.status === 200) return { status: 200, body: new Uint8Array(await res.arrayBuffer()) };
    if (res.status === 404) return { status: 404, body: EMPTY };
  } catch {
    // A thrown fetch is a network failure.
  }
  return { status: STATUS_NETWORK, body: EMPTY };
}

/**
 * The need-URL loop: `pkgResolve(input)` until `done` or `error`. A 200
 * supplies the body (and records it for OPFS), a 404 supplies 404, any
 * other status or a thrown fetch supplies status 0.
 */
export async function importPackages(
  core: PkgCore,
  input: DriverInput,
  fetchFn: FetchFn,
): Promise<DriverResult> {
  const bodies = new Map<string, Uint8Array>();
  const error = (code: string, message: string): DriverResult => ({ status: 'error', code, message, bodies });
  let fetches = 0;
  for (;;) {
    let reply: PkgReply;
    try {
      reply = await nextReply(core, () => core.pkgResolve(input));
    } catch (e) {
      return error('package-resolve', `package driver failed: ${String(e)}`);
    }
    if (reply.status === 'done') {
      return { status: 'done', lock: reply.lock, resolved: reply.resolved, bodies };
    }
    if (reply.status === 'error') return error(reply.code, reply.message);
    if (reply.status !== 'need' || typeof reply.url !== 'string') {
      return error('package-resolve', 'malformed package driver reply');
    }
    const url = reply.url;
    if (!underProxy(input.proxy, url)) {
      return error('package-resolve', `refused to fetch \`${url}\`: not under the proxy \`${proxyBase(input.proxy)}\``);
    }
    if (fetches >= MAX_FETCHES) return error('package-resolve', 'too many fetches');
    fetches += 1;
    const got = await fetchOne(fetchFn, url);
    if (got.status === 200) bodies.set(url, got.body);
    core.pkgSupply(url, got.status, got.body);
  }
}

// ------------------------------------------------------------ restore

export type RestoreResult =
  | { status: 'empty' }
  | { status: 'no-proxy' }
  | { status: 'done'; lock: string; resolved: PkgResolved[]; requirements: Record<string, string> }
  | { status: 'error'; code: string; message: string; removed: string[] };

/**
 * The stored bodies a `package-integrity` message blames: the first
 * backticked token names `<path>@<version>`, whose proxy URLs contain
 * `/<path>/@v/<version>.`. When the message names no stored body, every
 * stored body is blamed (none can be trusted to be the good one).
 */
export function offendingUrls(message: string, urls: Iterable<string>): string[] {
  const all = [...urls];
  const token = /`([^`]+)`/.exec(message)?.[1] ?? '';
  const at = token.lastIndexOf('@');
  if (at > 0) {
    const needle = `/${token.slice(0, at)}/@v/${token.slice(at + 1)}.`;
    const hit = all.filter((u) => u.includes(needle));
    if (hit.length > 0) return hit;
  }
  return all;
}

/**
 * Restore on startup (browser tier): supply every stored body under the
 * proxy, then `pkgResolve({proxy, lock})`, which re-validates and compares
 * every digest with the lock. `done` stores the lock again; a
 * `package-integrity` error deletes the offending bodies and the lock.
 */
export async function restorePackages(
  core: PkgCore,
  store: PkgStore,
  proxy: string,
  fetchFn: FetchFn,
): Promise<RestoreResult> {
  const saved = await store.load();
  if (!saved || saved.lock === null) return { status: 'empty' };
  if (proxyBase(proxy) === '') return { status: 'no-proxy' };
  for (const [url, body] of saved.bodies) {
    if (underProxy(proxy, url)) core.pkgSupply(url, 200, body);
  }
  const r = await importPackages(core, { proxy: proxyBase(proxy), lock: saved.lock }, fetchFn);
  if (r.status === 'done') {
    await store.save({ requirements: saved.requirements, lock: r.lock, bodies: r.bodies });
    return { status: 'done', lock: r.lock, resolved: r.resolved, requirements: saved.requirements };
  }
  const removed: string[] = [];
  if (r.code === 'package-integrity') {
    for (const url of offendingUrls(r.message, saved.bodies.keys())) {
      await store.remove(url);
      removed.push(url);
    }
    await store.removeLock();
  }
  return { status: 'error', code: r.code, message: r.message, removed };
}

// ------------------------------------------------ unresolved imports

export const UNRESOLVED_CODES: ReadonlySet<string> = new Set(['package-not-locked', 'package-not-fetched']);

export interface UnresolvedImport {
  path: string;
  code: string;
  message: string;
}

/**
 * The package path of a `package-not-locked`/`package-not-fetched`
 * diagnostic. Rule: the first backticked token of the message (Rust
 * `PkgError` Display: "package `<path>` is not in `vactrol.lock`",
 * "`<path>[@<version>]` is locked but not fetched; ..."), cut at its first
 * `@`. Null when the message carries no backticked token.
 */
export function importPath(message: string): string | null {
  const token = /`([^`]+)`/.exec(message)?.[1];
  if (!token) return null;
  const at = token.indexOf('@');
  const path = (at >= 0 ? token.slice(0, at) : token).trim();
  return path === '' ? null : path;
}

/** The unresolved imports of `diags`, deduplicated by path, in order. */
export function unresolvedImports(diags: readonly Diagnostic[]): UnresolvedImport[] {
  const out: UnresolvedImport[] = [];
  const seen = new Set<string>();
  for (const d of diags) {
    if (!UNRESOLVED_CODES.has(d.code)) continue;
    const path = importPath(d.message);
    if (path === null || seen.has(path)) continue;
    seen.add(path);
    out.push({ path, code: d.code, message: d.message });
  }
  return out;
}
