import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  BINDINGS_FILTER,
  BrowserFiles,
  FileCancelled,
  MemoryFiles,
  TauriFiles,
  VACT_FILTER,
  sidecarName,
} from '../../src/platform/files';

const dialog = vi.hoisted(() => ({
  open: vi.fn(),
  save: vi.fn(),
  ask: vi.fn(),
  message: vi.fn(),
}));
const fs = vi.hoisted(() => ({
  readTextFile: vi.fn(),
  writeTextFile: vi.fn(),
  readFile: vi.fn(),
  writeFile: vi.fn(),
  remove: vi.fn(),
  readDir: vi.fn(),
  mkdir: vi.fn(),
}));
vi.mock('@tauri-apps/plugin-dialog', () => dialog);
vi.mock('@tauri-apps/plugin-fs', () => fs);

describe('sidecarName', () => {
  it('replaces the .vact extension', () => {
    expect(sidecarName('dir/song.vact')).toBe('dir/song.bindings.json');
    expect(sidecarName('untitled')).toBe('untitled.bindings.json');
  });
});

describe('MemoryFiles', () => {
  it('round-trips documents and sidecars', async () => {
    const files = new MemoryFiles({ 'a.vact': '(s "bd")' });
    files.queueOpen('a.vact');
    expect(await files.open()).toEqual({ name: 'a.vact', text: '(s "bd")' });
    await files.save('a.vact', '(s "sn")');
    await files.saveSidecar('a.vact', '{"v":1,"bindings":[]}');
    expect(files.files.get('a.vact')).toBe('(s "sn")');
    expect(files.files.get('a.bindings.json')).toBe('{"v":1,"bindings":[]}');
    await expect(files.open()).rejects.toBeInstanceOf(FileCancelled);
  });
});

describe('TauriFiles', () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  function onlyTextCalls(): void {
    for (const name of ['readFile', 'writeFile', 'remove', 'readDir', 'mkdir'] as const) {
      expect(fs[name]).not.toHaveBeenCalled();
    }
    expect(dialog.ask).not.toHaveBeenCalled();
    expect(dialog.message).not.toHaveBeenCalled();
  }

  it('opens through the dialog with the .vact filter and reads text', async () => {
    dialog.open.mockResolvedValue('/home/u/song.vact');
    fs.readTextFile.mockResolvedValue('(s "bd")');
    const files = new TauriFiles();
    expect(await files.open()).toEqual({ name: '/home/u/song.vact', text: '(s "bd")' });
    expect(dialog.open).toHaveBeenCalledWith({ multiple: false, directory: false, filters: [VACT_FILTER] });
    expect(fs.readTextFile).toHaveBeenCalledWith('/home/u/song.vact');
    // The opened path is in scope: saving writes without another dialog.
    await files.save('/home/u/song.vact', 'x');
    expect(dialog.save).not.toHaveBeenCalled();
    expect(fs.writeTextFile).toHaveBeenCalledWith('/home/u/song.vact', 'x');
    onlyTextCalls();
  });

  it('saves the sidecar through a dialog with the bindings filter', async () => {
    dialog.save.mockResolvedValue('/home/u/song.bindings.json');
    const files = new TauriFiles();
    await files.saveSidecar('/home/u/song.vact', '{"v":1}');
    expect(dialog.save).toHaveBeenCalledWith({
      defaultPath: '/home/u/song.bindings.json',
      filters: [BINDINGS_FILTER],
    });
    expect(fs.writeTextFile).toHaveBeenCalledWith('/home/u/song.bindings.json', '{"v":1}');
    onlyTextCalls();
  });

  it('reports a cancelled dialog as FileCancelled', async () => {
    dialog.open.mockResolvedValue(null);
    dialog.save.mockResolvedValue(null);
    const files = new TauriFiles();
    await expect(files.open()).rejects.toBeInstanceOf(FileCancelled);
    await expect(files.save('new.vact', 'x')).rejects.toBeInstanceOf(FileCancelled);
    expect(fs.writeTextFile).not.toHaveBeenCalled();
  });
});

describe('BrowserFiles', () => {
  it('uses the File System Access API when present', async () => {
    const written: string[] = [];
    const handle = {
      name: 'song.vact',
      getFile: async () => new File(['(s "bd")'], 'song.vact'),
      createWritable: async () => ({
        write: async (d: string) => {
          written.push(d);
        },
        close: async () => {},
      }),
    };
    const pickers = {
      document,
      showOpenFilePicker: vi.fn(async () => [handle]),
      showSaveFilePicker: vi.fn(async () => ({ ...handle, name: 'song.bindings.json' })),
    };
    const win = pickers as unknown as Window & typeof pickers;
    const files = new BrowserFiles(win);
    expect(await files.open()).toEqual({ name: 'song.vact', text: '(s "bd")' });
    await files.save('song.vact', 'new text');
    await files.saveSidecar('song.vact', '{"v":1}');
    expect(written).toEqual(['new text', '{"v":1}']);
    expect(win.showSaveFilePicker).toHaveBeenCalledTimes(1);
  });

  it('falls back to a download without the API', async () => {
    const clicks: string[] = [];
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(function (this: HTMLAnchorElement) {
      clicks.push(this.download);
    });
    const createUrl = vi.fn(() => 'blob:x');
    const saved = { create: URL.createObjectURL, revoke: URL.revokeObjectURL };
    URL.createObjectURL = createUrl;
    URL.revokeObjectURL = vi.fn();
    try {
      const files = new BrowserFiles(window);
      await files.save('dir/song.vact', 'text');
      expect(clicks).toEqual(['song.vact']);
      expect(createUrl).toHaveBeenCalledTimes(1);
    } finally {
      click.mockRestore();
      URL.createObjectURL = saved.create;
      URL.revokeObjectURL = saved.revoke;
    }
  });
});
