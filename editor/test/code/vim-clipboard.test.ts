import { describe, expect, it, vi } from 'vitest';
import { browserClipboard, createClipboardAdapter } from '../../src/code/vim-clipboard';

describe('Vim clipboard adapter', () => {
  it('resolves reads and writes through the browser API', async () => {
    const readText = vi.fn(() => Promise.resolve('copied'));
    const writeText = vi.fn(() => Promise.resolve());
    const adapter = createClipboardAdapter({ readText, writeText });
    await expect(adapter.read()).resolves.toBe('copied');
    await expect(adapter.write('value')).resolves.toBeUndefined();
    expect(writeText).toHaveBeenCalledWith('value');
  });
  it('rejects when the clipboard API is absent', async () => {
    const adapter = createClipboardAdapter(null);
    await expect(adapter.read()).rejects.toThrow('clipboard unavailable');
    await expect(adapter.write('value')).rejects.toThrow('clipboard unavailable');
  });
  it('converts a synchronous API throw to a rejection', async () => {
    const adapter = createClipboardAdapter({ readText: () => { throw new Error('denied'); }, writeText: async () => undefined });
    await expect(adapter.read()).rejects.toThrow('denied');
  });
  it('propagates an asynchronous write rejection', async () => {
    const adapter = createClipboardAdapter({ readText: async () => '', writeText: () => Promise.reject(new Error('denied')) });
    await expect(adapter.write('value')).rejects.toThrow('denied');
  });
  it('browserClipboard rejects gracefully when jsdom has no clipboard', async () => {
    await expect(browserClipboard().read()).rejects.toThrow('clipboard unavailable');
  });
});
