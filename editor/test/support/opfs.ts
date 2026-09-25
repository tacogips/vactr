// An in-memory `FileSystemDirectoryHandle` fake for the OPFS store tests:
// nested directories, byte files, `NotFoundError` on a missing entry, and a
// `storage` manager whose `getDirectory()` returns the root.

import type { DirHandleLike, FileHandleLike, StorageManagerLike, WritableLike } from '../../src/pkg/opfs';

const enc = new TextEncoder();

function notFound(name: string): Error {
  const e = new Error(`${name} not found`);
  e.name = 'NotFoundError';
  return e;
}

export class FakeFile implements FileHandleLike {
  data = new Uint8Array(0);

  async getFile(): Promise<{ arrayBuffer(): Promise<ArrayBuffer> }> {
    const copy = this.data.slice();
    return { arrayBuffer: async () => copy.buffer };
  }

  async createWritable(): Promise<WritableLike> {
    const parts: Uint8Array[] = [];
    return {
      write: async (d) => {
        parts.push(typeof d === 'string' ? enc.encode(d) : d.slice());
      },
      close: async () => {
        const n = parts.reduce((s, p) => s + p.length, 0);
        const out = new Uint8Array(n);
        let at = 0;
        for (const p of parts) {
          out.set(p, at);
          at += p.length;
        }
        this.data = out;
      },
    };
  }
}

export class FakeDir implements DirHandleLike {
  readonly dirs = new Map<string, FakeDir>();
  readonly files = new Map<string, FakeFile>();

  async getDirectoryHandle(name: string, opts: { create?: boolean } = {}): Promise<FakeDir> {
    let d = this.dirs.get(name);
    if (!d) {
      if (!opts.create) throw notFound(name);
      d = new FakeDir();
      this.dirs.set(name, d);
    }
    return d;
  }

  async getFileHandle(name: string, opts: { create?: boolean } = {}): Promise<FakeFile> {
    let f = this.files.get(name);
    if (!f) {
      if (!opts.create) throw notFound(name);
      f = new FakeFile();
      this.files.set(name, f);
    }
    return f;
  }

  async removeEntry(name: string): Promise<void> {
    if (!this.files.delete(name) && !this.dirs.delete(name)) throw notFound(name);
  }

  /** A nested directory, or undefined. */
  dir(...path: string[]): FakeDir | undefined {
    let d: FakeDir | undefined = this;
    for (const p of path) d = d?.dirs.get(p);
    return d;
  }

  /** A file's text, or undefined. */
  text(...path: string[]): string | undefined {
    const name = path[path.length - 1] as string;
    const f = this.dir(...path.slice(0, -1))?.files.get(name);
    return f ? new TextDecoder().decode(f.data) : undefined;
  }
}

/** A storage manager over a fresh (or given) root directory. */
export function fakeStorage(root = new FakeDir()): StorageManagerLike & { root: FakeDir } {
  return { root, getDirectory: async () => root };
}
