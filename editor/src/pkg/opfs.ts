// Package persistence in the browser (design 15.1.10 "OPFS").
//
// Under `navigator.storage.getDirectory()` / `vactr-pkg/`:
//   requirements.json   the root requirements `{path: version | ""}`
//   vactr.lock        the lock text of the last `done`
//   index.json          `{url: file}` for every stored proxy body
//   bodies/<sha256(url)> the raw proxy response bodies
// OPFS holds raw bytes only, never a trusted tree: a restore hands them to
// the Rust pipeline (validation, digest, staged publication) again. Without
// OPFS the store is memory-only and the pane shows `MEMORY_ONLY_HINT`.

export const DIR_NAME = 'vactr-pkg';
export const REQUIREMENTS_FILE = 'requirements.json';
export const LOCK_FILE = 'vactr.lock';
export const INDEX_FILE = 'index.json';
export const BODIES_DIR = 'bodies';
export const MEMORY_ONLY_HINT = 'packages are kept for this session only';

export interface StoredPackages {
  requirements: Record<string, string>;
  lock: string | null;
  bodies: Map<string, Uint8Array>;
}

export interface SaveInput {
  requirements: Record<string, string>;
  lock: string;
  /** Bodies to add (merged with the stored ones). */
  bodies: ReadonlyMap<string, Uint8Array>;
}

export interface PkgStore {
  /** False for the memory-only fallback. */
  readonly persistent: boolean;
  save(input: SaveInput): Promise<void>;
  /** Null when nothing is stored. */
  load(): Promise<StoredPackages | null>;
  remove(url: string): Promise<void>;
  removeLock(): Promise<void>;
}

// The structural slice of the File System Access API used here (the test
// fake implements exactly this).

export interface WritableLike {
  write(data: Uint8Array | string): Promise<void>;
  close(): Promise<void>;
}

export interface FileHandleLike {
  getFile(): Promise<{ arrayBuffer(): Promise<ArrayBuffer> }>;
  createWritable(): Promise<WritableLike>;
}

export interface DirHandleLike {
  getDirectoryHandle(name: string, opts?: { create?: boolean }): Promise<DirHandleLike>;
  getFileHandle(name: string, opts?: { create?: boolean }): Promise<FileHandleLike>;
  removeEntry(name: string): Promise<void>;
}

export interface StorageManagerLike {
  getDirectory?: () => Promise<DirHandleLike>;
}

const enc = new TextEncoder();
const dec = new TextDecoder();

/** Lower-case hex SHA-256 of `text` (the body file name of a URL). */
export async function sha256Hex(text: string): Promise<string> {
  const digest = await globalThis.crypto.subtle.digest('SHA-256', enc.encode(text));
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

function isNotFound(e: unknown): boolean {
  return typeof e === 'object' && e !== null && (e as { name?: unknown }).name === 'NotFoundError';
}

async function readBytes(dir: DirHandleLike, name: string): Promise<Uint8Array | null> {
  try {
    const file = await (await dir.getFileHandle(name)).getFile();
    return new Uint8Array(await file.arrayBuffer());
  } catch (e) {
    if (isNotFound(e)) return null;
    throw e;
  }
}

async function readText(dir: DirHandleLike, name: string): Promise<string | null> {
  const bytes = await readBytes(dir, name);
  return bytes === null ? null : dec.decode(bytes);
}

async function writeBytes(dir: DirHandleLike, name: string, data: Uint8Array | string): Promise<void> {
  const w = await (await dir.getFileHandle(name, { create: true })).createWritable();
  await w.write(data);
  await w.close();
}

async function removeQuiet(dir: DirHandleLike, name: string): Promise<void> {
  try {
    await dir.removeEntry(name);
  } catch (e) {
    if (!isNotFound(e)) throw e;
  }
}

function parseRecord(text: string | null): Record<string, string> {
  if (text === null) return {};
  try {
    const v = JSON.parse(text) as unknown;
    if (typeof v !== 'object' || v === null || Array.isArray(v)) return {};
    const out: Record<string, string> = {};
    for (const [k, x] of Object.entries(v)) if (typeof x === 'string') out[k] = x;
    return out;
  } catch {
    return {};
  }
}

/** The OPFS-backed store under `vactr-pkg/`. */
export class OpfsStore implements PkgStore {
  readonly persistent = true;
  private readonly dir: DirHandleLike;

  constructor(dir: DirHandleLike) {
    this.dir = dir;
  }

  private bodiesDir(): Promise<DirHandleLike> {
    return this.dir.getDirectoryHandle(BODIES_DIR, { create: true });
  }

  private async index(): Promise<Record<string, string>> {
    return parseRecord(await readText(this.dir, INDEX_FILE));
  }

  async save(input: SaveInput): Promise<void> {
    const index = await this.index();
    const bodies = await this.bodiesDir();
    for (const [url, body] of input.bodies) {
      const file = await sha256Hex(url);
      await writeBytes(bodies, file, body);
      index[url] = file;
    }
    await writeBytes(this.dir, INDEX_FILE, JSON.stringify(index));
    await writeBytes(this.dir, REQUIREMENTS_FILE, JSON.stringify(input.requirements));
    await writeBytes(this.dir, LOCK_FILE, input.lock);
  }

  async load(): Promise<StoredPackages | null> {
    const reqText = await readText(this.dir, REQUIREMENTS_FILE);
    const lock = await readText(this.dir, LOCK_FILE);
    if (reqText === null && lock === null) return null;
    const bodies = new Map<string, Uint8Array>();
    const dir = await this.bodiesDir();
    for (const [url, file] of Object.entries(await this.index())) {
      const body = await readBytes(dir, file);
      if (body !== null) bodies.set(url, body);
    }
    return { requirements: parseRecord(reqText), lock, bodies };
  }

  async remove(url: string): Promise<void> {
    const index = await this.index();
    const file = index[url] ?? (await sha256Hex(url));
    await removeQuiet(await this.bodiesDir(), file);
    delete index[url];
    await writeBytes(this.dir, INDEX_FILE, JSON.stringify(index));
  }

  async removeLock(): Promise<void> {
    await removeQuiet(this.dir, LOCK_FILE);
  }
}

/** The memory-only fallback (no OPFS). */
export class MemoryStore implements PkgStore {
  readonly persistent = false;
  private requirements: Record<string, string> | null = null;
  private lock: string | null = null;
  private readonly bodies = new Map<string, Uint8Array>();

  async save(input: SaveInput): Promise<void> {
    for (const [url, body] of input.bodies) this.bodies.set(url, body);
    this.requirements = { ...input.requirements };
    this.lock = input.lock;
  }

  async load(): Promise<StoredPackages | null> {
    if (this.requirements === null && this.lock === null) return null;
    return { requirements: { ...(this.requirements ?? {}) }, lock: this.lock, bodies: new Map(this.bodies) };
  }

  async remove(url: string): Promise<void> {
    this.bodies.delete(url);
  }

  async removeLock(): Promise<void> {
    this.lock = null;
  }
}

function defaultStorageManager(): StorageManagerLike | null {
  return typeof navigator === 'undefined' ? null : ((navigator as { storage?: StorageManagerLike }).storage ?? null);
}

/** OPFS when available, else the memory-only store. */
export async function openPkgStore(
  storage: StorageManagerLike | null = defaultStorageManager(),
): Promise<PkgStore> {
  if (!storage || typeof storage.getDirectory !== 'function') return new MemoryStore();
  try {
    const root = await storage.getDirectory();
    return new OpfsStore(await root.getDirectoryHandle(DIR_NAME, { create: true }));
  } catch {
    return new MemoryStore();
  }
}
