// A fake package proxy: serves `<base>/<path>/@v/list`, `@v/<v>.toml` and
// `@v/<v>.zip` bodies (zips built with `zip.ts`), answers 404 for any other
// URL under the base, and records every requested URL in order. Individual
// URLs can be scripted to another status or to a thrown network error.

import type { FetchFn, FetchResponseLike } from '../../src/pkg/driver';
import { buildZip, type ZipEntry } from './zip';

const enc = new TextEncoder();

export interface FakePackage {
  path: string;
  version: string;
  toml: string;
  files: ZipEntry[];
}

export class FakeProxy {
  readonly base: string;
  readonly requested: string[] = [];
  private readonly bodies = new Map<string, Uint8Array>();
  private readonly statuses = new Map<string, number>();
  private readonly failures = new Set<string>();

  constructor(base = 'https://proxy.test') {
    this.base = base.replace(/\/+$/, '');
  }

  /** Serves `pkg` (and adds its version to the path's `list`). */
  add(pkg: FakePackage): this {
    const dir = `${this.base}/${pkg.path}/@v`;
    const listUrl = `${dir}/list`;
    const prev = this.bodies.get(listUrl);
    const list = prev ? `${new TextDecoder().decode(prev)}${pkg.version}\n` : `${pkg.version}\n`;
    this.bodies.set(listUrl, enc.encode(list));
    this.bodies.set(`${dir}/${pkg.version}.toml`, enc.encode(pkg.toml));
    this.bodies.set(`${dir}/${pkg.version}.zip`, buildZip(pkg.files));
    return this;
  }

  /** Serves raw bytes (or text) at `url`. */
  serve(url: string, body: Uint8Array | string): this {
    this.bodies.set(url, typeof body === 'string' ? enc.encode(body) : body);
    return this;
  }

  /** Answers `url` with `status` and an empty body. */
  status(url: string, status: number): this {
    this.statuses.set(url, status);
    return this;
  }

  /** Makes `url` throw like a network failure. */
  fail(url: string): this {
    this.failures.add(url);
    return this;
  }

  body(url: string): Uint8Array | undefined {
    return this.bodies.get(url);
  }

  readonly fetch: FetchFn = async (url: string): Promise<FetchResponseLike> => {
    this.requested.push(url);
    if (this.failures.has(url)) throw new TypeError('Failed to fetch');
    const scripted = this.statuses.get(url);
    const body = this.bodies.get(url);
    const status = scripted ?? (body ? 200 : 404);
    const bytes = status === 200 && body ? body.slice() : new Uint8Array(0);
    return { status, arrayBuffer: async () => bytes.buffer };
  };
}
