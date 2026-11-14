import {
  FileCancelled,
  downloadText,
  isAbort,
  loadTauriPlugins,
  type DialogFilter,
  type TauriPlugins,
} from './files';
const MAX_BYTES = 262_144;
export const JSON_FILTER: DialogFilter = { name: 'JSON (.json)', extensions: ['json'] };
export class JsonFileTooLarge extends Error {
  constructor() {
    super('JSON file exceeds 262144 bytes');
    this.name = 'JsonFileTooLarge';
  }
}
export interface JsonFiles {
  importJson(): Promise<string>;
  exportJson(name: string, text: string): Promise<void>;
}
interface PickerFile {
  text(): Promise<string>;
}
interface PickerHandle {
  getFile(): Promise<PickerFile>;
  createWritable(): Promise<{ write(data: string): Promise<void>; close(): Promise<void> }>;
}
interface JsonPickerWindow extends Window {
  showOpenFilePicker?: (opts: {
    types: { description: string; accept: Record<string, string[]> }[];
    multiple?: boolean;
  }) => Promise<PickerHandle[]>;
  showSaveFilePicker?: (opts: {
    suggestedName: string;
    types: { description: string; accept: Record<string, string[]> }[];
  }) => Promise<PickerHandle>;
}
const JSON_TYPE = { description: 'JSON', accept: { 'application/json': ['.json'] } };
function tooLarge(text: string): boolean {
  return new TextEncoder().encode(text).byteLength > MAX_BYTES;
}
export class BrowserJsonFiles implements JsonFiles {
  private readonly win: JsonPickerWindow;

  constructor(win: Window = window) {
    this.win = win as JsonPickerWindow;
  }

  async importJson(): Promise<string> {
    const picker = this.win.showOpenFilePicker;
    if (picker) {
      let handles: PickerHandle[];
      try {
        handles = await picker.call(this.win, { types: [JSON_TYPE], multiple: false });
      } catch (error) {
        if (isAbort(error)) throw new FileCancelled();
        throw error;
      }
      const handle = handles[0];
      if (!handle) throw new FileCancelled();
      return this.read(handle);
    }
    return this.importWithInput();
  }

  async exportJson(name: string, text: string): Promise<void> {
    const picker = this.win.showSaveFilePicker;
    if (picker) {
      let handle: PickerHandle;
      try {
        handle = await picker.call(this.win, { suggestedName: name, types: [JSON_TYPE] });
      } catch (error) {
        if (isAbort(error)) throw new FileCancelled();
        throw error;
      }
      const writable = await handle.createWritable();
      await writable.write(text);
      await writable.close();
      return;
    }
    downloadText(this.win.document, name, text, 'application/json');
  }

  private async read(handle: PickerHandle): Promise<string> {
    const text = await (await handle.getFile()).text();
    if (tooLarge(text)) throw new JsonFileTooLarge();
    return text;
  }

  private importWithInput(): Promise<string> {
    const doc = this.win.document;
    return new Promise((resolve, reject) => {
      const input = doc.createElement('input');
      input.type = 'file';
      input.accept = '.json';
      input.addEventListener('change', () => {
        const file = input.files?.[0];
        if (!file) {
          reject(new FileCancelled());
          return;
        }
        file.text().then((text) => {
          if (tooLarge(text)) reject(new JsonFileTooLarge());
          else resolve(text);
        }, reject);
      });
      input.addEventListener('cancel', () => reject(new FileCancelled()));
      input.click();
    });
  }
}
export class TauriJsonFiles implements JsonFiles {
  private readonly load: () => Promise<TauriPlugins>;
  private plugins: Promise<TauriPlugins> | null = null;

  constructor(load: () => Promise<TauriPlugins> = loadTauriPlugins) {
    this.load = load;
  }

  async importJson(): Promise<string> {
    const { dialog, fs } = await this.p;
    const picked = await dialog.open({ multiple: false, directory: false, filters: [JSON_FILTER] });
    if (typeof picked !== 'string') throw new FileCancelled();
    const text = await fs.readTextFile(picked);
    if (tooLarge(text)) throw new JsonFileTooLarge();
    return text;
  }

  async exportJson(name: string, text: string): Promise<void> {
    const { dialog, fs } = await this.p;
    const target = await dialog.save({ defaultPath: name, filters: [JSON_FILTER] });
    if (target === null) throw new FileCancelled();
    await fs.writeTextFile(target, text);
  }

  private get p(): Promise<TauriPlugins> {
    this.plugins ??= this.load();
    return this.plugins;
  }
}
