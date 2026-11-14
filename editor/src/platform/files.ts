// Platform file access (design 15.1.6 "Persistence and saving", 15.1.11).
//
// `.vact` documents are saved as the buffer text; the ExternalFile sidecar
// `<doc>.bindings.json` goes through `saveSidecar` (for `song.vact` the
// sidecar is `song.bindings.json`). Three implementations:
// - `BrowserFiles`: the File System Access API when present, else an
//   `<input type=file>` to open and a download to save;
// - `TauriFiles`: the dialog and fs plugins, loaded lazily, with `.vact` and
//   `.bindings.json` filters and text reads/writes only (dialog-scoped);
// - `MemoryFiles`: tests.

export interface OpenedFile {
  name: string;
  text: string;
}

export interface FileAccess {
  /** Asks the user for a `.vact` file; rejects with `FileCancelled`. */
  open(): Promise<OpenedFile>;
  save(name: string, text: string): Promise<void>;
  /** Writes `sidecarName(name)`. */
  saveSidecar(name: string, text: string): Promise<void>;
}

export class FileCancelled extends Error {
  constructor() {
    super('cancelled');
    this.name = 'FileCancelled';
  }
}

/** `dir/song.vact` -> `dir/song.bindings.json`. */
export function sidecarName(name: string): string {
  const stem = name.endsWith('.vact') ? name.slice(0, -'.vact'.length) : name;
  return `${stem}.bindings.json`;
}

function baseName(path: string): string {
  const i = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return i >= 0 ? path.slice(i + 1) : path;
}

// ------------------------------------------------------------------ memory

export class MemoryFiles implements FileAccess {
  readonly files = new Map<string, string>();
  private readonly toOpen: string[] = [];

  constructor(initial: Record<string, string> = {}) {
    for (const [k, v] of Object.entries(initial)) this.files.set(k, v);
  }

  /** The next `open()` returns `name`. */
  queueOpen(name: string): void {
    this.toOpen.push(name);
  }

  async open(): Promise<OpenedFile> {
    const name = this.toOpen.shift();
    const text = name === undefined ? undefined : this.files.get(name);
    if (name === undefined || text === undefined) throw new FileCancelled();
    return { name, text };
  }

  async save(name: string, text: string): Promise<void> {
    this.files.set(name, text);
  }

  async saveSidecar(name: string, text: string): Promise<void> {
    this.files.set(sidecarName(name), text);
  }
}

// ------------------------------------------------------------------- tauri

export interface DialogFilter {
  name: string;
  extensions: string[];
}

export const VACT_FILTER: DialogFilter = { name: 'Vactr source (.vact)', extensions: ['vact'] };
export const BINDINGS_FILTER: DialogFilter = {
  name: 'Vactr bindings (.bindings.json)',
  extensions: ['json'],
};

/** The plugin calls TauriFiles uses (the real modules satisfy these). */
export interface TauriDialog {
  open(opts: { multiple?: false; directory?: false; filters?: DialogFilter[] }): Promise<unknown>;
  save(opts: { defaultPath?: string; filters?: DialogFilter[] }): Promise<string | null>;
}

export interface TauriFs {
  readTextFile(path: string): Promise<string>;
  writeTextFile(path: string, text: string): Promise<void>;
}

export interface TauriPlugins {
  dialog: TauriDialog;
  fs: TauriFs;
}

async function loadTauriPlugins(): Promise<TauriPlugins> {
  const [dialog, fs] = await Promise.all([
    import('@tauri-apps/plugin-dialog'),
    import('@tauri-apps/plugin-fs'),
  ]);
  return { dialog: dialog as unknown as TauriDialog, fs: fs as unknown as TauriFs };
}

export class TauriFiles implements FileAccess {
  private readonly load: () => Promise<TauriPlugins>;
  private plugins: Promise<TauriPlugins> | null = null;
  /** Paths the user picked in a dialog this session (the fs scope). */
  private readonly chosen = new Set<string>();

  constructor(load: () => Promise<TauriPlugins> = loadTauriPlugins) {
    this.load = load;
  }

  private get p(): Promise<TauriPlugins> {
    this.plugins ??= this.load();
    return this.plugins;
  }

  async open(): Promise<OpenedFile> {
    const { dialog, fs } = await this.p;
    const picked = await dialog.open({ multiple: false, directory: false, filters: [VACT_FILTER] });
    if (typeof picked !== 'string') throw new FileCancelled();
    this.chosen.add(picked);
    return { name: picked, text: await fs.readTextFile(picked) };
  }

  save(name: string, text: string): Promise<void> {
    return this.write(name, text, VACT_FILTER);
  }

  saveSidecar(name: string, text: string): Promise<void> {
    return this.write(sidecarName(name), text, BINDINGS_FILTER);
  }

  private async write(path: string, text: string, filter: DialogFilter): Promise<void> {
    const { dialog, fs } = await this.p;
    let target: string | null = this.chosen.has(path) ? path : null;
    if (target === null) {
      target = await dialog.save({ defaultPath: path, filters: [filter] });
      if (target === null) throw new FileCancelled();
      this.chosen.add(target);
    }
    await fs.writeTextFile(target, text);
  }
}

// ----------------------------------------------------------------- browser

interface WritableLike {
  write(data: string): Promise<void>;
  close(): Promise<void>;
}

interface FileHandleLike {
  name: string;
  getFile(): Promise<File>;
  createWritable(): Promise<WritableLike>;
}

interface PickerType {
  description: string;
  accept: Record<string, string[]>;
}

interface FsAccessWindow {
  showOpenFilePicker?: (opts: { types: PickerType[]; multiple?: boolean }) => Promise<FileHandleLike[]>;
  showSaveFilePicker?: (opts: { suggestedName: string; types: PickerType[] }) => Promise<FileHandleLike>;
}

const VACT_TYPE: PickerType = { description: 'Vactr source', accept: { 'text/plain': ['.vact'] } };
const BINDINGS_TYPE: PickerType = {
  description: 'Vactr bindings',
  accept: { 'application/json': ['.json'] },
};

function isAbort(e: unknown): boolean {
  return e instanceof DOMException && e.name === 'AbortError';
}

export class BrowserFiles implements FileAccess {
  private readonly win: Window & FsAccessWindow;
  private readonly handles = new Map<string, FileHandleLike>();

  constructor(win: Window = window) {
    this.win = win as Window & FsAccessWindow;
  }

  async open(): Promise<OpenedFile> {
    const picker = this.win.showOpenFilePicker;
    if (picker) {
      let handles: FileHandleLike[];
      try {
        handles = await picker.call(this.win, { types: [VACT_TYPE], multiple: false });
      } catch (e) {
        if (isAbort(e)) throw new FileCancelled();
        throw e;
      }
      const h = handles[0];
      if (!h) throw new FileCancelled();
      this.handles.set(h.name, h);
      return { name: h.name, text: await (await h.getFile()).text() };
    }
    return this.openWithInput();
  }

  save(name: string, text: string): Promise<void> {
    return this.write(name, text, VACT_TYPE, 'text/plain');
  }

  saveSidecar(name: string, text: string): Promise<void> {
    return this.write(sidecarName(name), text, BINDINGS_TYPE, 'application/json');
  }

  private async write(name: string, text: string, type: PickerType, mime: string): Promise<void> {
    let h = this.handles.get(name);
    const picker = this.win.showSaveFilePicker;
    if (!h && picker) {
      try {
        h = await picker.call(this.win, { suggestedName: baseName(name), types: [type] });
      } catch (e) {
        if (isAbort(e)) throw new FileCancelled();
        throw e;
      }
      this.handles.set(name, h);
    }
    if (h) {
      const w = await h.createWritable();
      await w.write(text);
      await w.close();
      return;
    }
    this.download(baseName(name), text, mime);
  }

  private openWithInput(): Promise<OpenedFile> {
    const doc = this.win.document;
    return new Promise((resolve, reject) => {
      const input = doc.createElement('input');
      input.type = 'file';
      input.accept = '.vact';
      input.addEventListener('change', () => {
        const f = input.files?.[0];
        if (!f) {
          reject(new FileCancelled());
          return;
        }
        f.text().then((text) => resolve({ name: f.name, text }), reject);
      });
      input.addEventListener('cancel', () => reject(new FileCancelled()));
      input.click();
    });
  }

  private download(name: string, text: string, mime: string): void {
    const doc = this.win.document;
    const url = URL.createObjectURL(new Blob([text], { type: mime }));
    const a = doc.createElement('a');
    a.href = url;
    a.download = name;
    doc.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 0);
  }
}
