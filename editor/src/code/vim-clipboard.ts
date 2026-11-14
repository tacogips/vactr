import type { ClipboardAdapter } from './vim/types';

type BrowserClipboard = Pick<Clipboard, 'readText' | 'writeText'>;

export function createClipboardAdapter(clipboard: BrowserClipboard | null | undefined): ClipboardAdapter {
  return {
    read() {
      try {
        if (typeof clipboard?.readText !== 'function') return Promise.reject(new Error('clipboard unavailable'));
        return Promise.resolve(clipboard.readText());
      } catch (error) {
        return Promise.reject(error);
      }
    },
    write(text) {
      try {
        if (typeof clipboard?.writeText !== 'function') return Promise.reject(new Error('clipboard unavailable'));
        return Promise.resolve(clipboard.writeText(text));
      } catch (error) {
        return Promise.reject(error);
      }
    },
  };
}

export function browserClipboard(): ClipboardAdapter {
  try {
    return createClipboardAdapter(globalThis.navigator?.clipboard);
  } catch {
    return createClipboardAdapter(null);
  }
}
