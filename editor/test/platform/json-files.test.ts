import { afterEach, describe, expect, it, vi } from 'vitest';
import { FileCancelled, type TauriPlugins } from '../../src/platform/files';
import { BrowserJsonFiles, JSON_FILTER, JsonFileTooLarge, TauriJsonFiles } from '../../src/platform/json-files';

function plugins(open: (options: unknown) => Promise<unknown>, save: (options: unknown) => Promise<string | null>, text = '{"version":1}') {
  const readTextFile = vi.fn(async () => text);
  const writeTextFile = vi.fn(async () => undefined);
  const value: TauriPlugins = {
    dialog: { open, save },
    fs: { readTextFile, writeTextFile },
  };
  return { value, readTextFile, writeTextFile };
}

describe('TauriJsonFiles', () => {
  it('imports text through the JSON dialog filter', async () => {
    const open = vi.fn(async () => '/picked/keymap.json');
    const save = vi.fn(async () => null);
    const fake = plugins(open, save);
    const files = new TauriJsonFiles(async () => fake.value);
    await expect(files.importJson()).resolves.toBe('{"version":1}');
    expect(open).toHaveBeenCalledWith({ multiple: false, directory: false, filters: [JSON_FILTER] });
    expect(fake.readTextFile).toHaveBeenCalledWith('/picked/keymap.json');
  });

  it('reports an import dialog cancellation', async () => {
    const fake = plugins(async () => null, async () => null);
    await expect(new TauriJsonFiles(async () => fake.value).importJson()).rejects.toBeInstanceOf(FileCancelled);
    expect(fake.readTextFile).not.toHaveBeenCalled();
  });

  it('rejects imported UTF-8 text over the byte limit', async () => {
    const fake = plugins(async () => '/large.json', async () => null, 'x'.repeat(262_145));
    await expect(new TauriJsonFiles(async () => fake.value).importJson()).rejects.toBeInstanceOf(JsonFileTooLarge);
  });

  it('exports text to the path selected by the JSON save dialog', async () => {
    const open = vi.fn(async () => null);
    const save = vi.fn(async () => '/picked/out.json');
    const fake = plugins(open, save);
    await new TauriJsonFiles(async () => fake.value).exportJson('keymap.json', 'contents');
    expect(save).toHaveBeenCalledWith({ defaultPath: 'keymap.json', filters: [JSON_FILTER] });
    expect(fake.writeTextFile).toHaveBeenCalledWith('/picked/out.json', 'contents');
  });

  it('reports an export dialog cancellation', async () => {
    const fake = plugins(async () => null, async () => null);
    await expect(new TauriJsonFiles(async () => fake.value).exportJson('x.json', 'x')).rejects.toBeInstanceOf(FileCancelled);
    expect(fake.writeTextFile).not.toHaveBeenCalled();
  });
});

describe('BrowserJsonFiles', () => {
  afterEach(() => vi.restoreAllMocks());

  it('imports through the file picker with the JSON type', async () => {
    const handle = { getFile: async () => ({ text: async () => '{"version":1}' }) };
    const showOpenFilePicker = vi.fn(async () => [handle]);
    const win = { document, showOpenFilePicker } as unknown as Window;
    await expect(new BrowserJsonFiles(win).importJson()).resolves.toBe('{"version":1}');
    expect(showOpenFilePicker).toHaveBeenCalledWith({
      types: [{ description: 'JSON', accept: { 'application/json': ['.json'] } }],
      multiple: false,
    });
  });

  it('maps picker abort to FileCancelled', async () => {
    const showOpenFilePicker = vi.fn(async () => { throw new DOMException('cancel', 'AbortError'); });
    const win = { document, showOpenFilePicker } as unknown as Window;
    await expect(new BrowserJsonFiles(win).importJson()).rejects.toBeInstanceOf(FileCancelled);
  });

  it('rejects oversized browser picker imports', async () => {
    const handle = { getFile: async () => ({ text: async () => 'x'.repeat(262_145) }) };
    const win = { document, showOpenFilePicker: async () => [handle] } as unknown as Window;
    await expect(new BrowserJsonFiles(win).importJson()).rejects.toBeInstanceOf(JsonFileTooLarge);
  });

  it('exports through the file picker and closes the writable', async () => {
    const write = vi.fn(async () => undefined);
    const close = vi.fn(async () => undefined);
    const handle = { createWritable: async () => ({ write, close }) };
    const showSaveFilePicker = vi.fn(async () => handle);
    const win = { document, showSaveFilePicker } as unknown as Window;
    await new BrowserJsonFiles(win).exportJson('keymap.json', 'contents');
    expect(showSaveFilePicker).toHaveBeenCalledWith({
      suggestedName: 'keymap.json',
      types: [{ description: 'JSON', accept: { 'application/json': ['.json'] } }],
    });
    expect(write).toHaveBeenCalledWith('contents');
    expect(close).toHaveBeenCalledOnce();
  });

  it('maps save picker abort to FileCancelled', async () => {
    const showSaveFilePicker = vi.fn(async () => { throw new DOMException('cancel', 'AbortError'); });
    const win = { document, showSaveFilePicker } as unknown as Window;
    await expect(new BrowserJsonFiles(win).exportJson('x.json', 'x')).rejects.toBeInstanceOf(FileCancelled);
  });

  it('downloads JSON when the save picker is unavailable', async () => {
    const names: string[] = [];
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(function (this: HTMLAnchorElement) {
      names.push(this.download);
    });
    const createObjectURL = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:json');
    vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => undefined);
    await new BrowserJsonFiles(window).exportJson('keymap.json', 'contents');
    expect(names).toEqual(['keymap.json']);
    expect(click).toHaveBeenCalledOnce();
    expect(createObjectURL).toHaveBeenCalledOnce();
  });

  it('cancels when the fallback file input emits cancel', async () => {
    vi.spyOn(HTMLInputElement.prototype, 'click').mockImplementation(function (this: HTMLInputElement) {
      this.dispatchEvent(new Event('cancel'));
    });
    await expect(new BrowserJsonFiles(window).importJson()).rejects.toBeInstanceOf(FileCancelled);
  });
});
