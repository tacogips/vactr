import { history, redo, undo } from '@codemirror/commands';
import { EditorState, Transaction, type TransactionSpec } from '@codemirror/state';
import { vi } from 'vitest';
import type { ClipboardAdapter, Platform, VimHost, VimKeyInput, VimUpdate, VimViewport } from '../../../src/code/vim/types';

export interface TestClipboard extends ClipboardAdapter { fail: boolean; value: string }
export interface TestHost extends VimHost { messages: { message: string; assertive: boolean }[]; scrolls: number[]; memoryClipboard: TestClipboard }

export function createTestHost(text: string, opts: { platform?: Platform; viewport?: VimViewport; defaultKeymaps?: (input: VimKeyInput) => boolean; clipboard?: ClipboardAdapter } = {}): TestHost {
  const marker = text.indexOf('|');
  if (marker !== text.lastIndexOf('|')) throw new Error('Test fixture must contain at most one cursor marker');
  const source = marker < 0 ? text : `${text.slice(0, marker)}${text.slice(marker + 1)}`;
  let state = EditorState.create({ doc: source, selection: marker < 0 ? undefined : { anchor: marker }, extensions: [history()] });
  const listeners = new Set<(update: VimUpdate) => void>();
  const messages: { message: string; assertive: boolean }[] = [];
  const scrolls: number[] = [];
  let dispatching = false;
  const memoryClipboard: TestClipboard = {
    value: '', fail: false,
    async read() { if (this.fail) throw new Error('Clipboard unavailable'); return this.value; },
    async write(value: string) { if (this.fail) throw new Error('Clipboard unavailable'); this.value = value; },
  };
  const clipboard = opts.clipboard ?? memoryClipboard;
  const host: TestHost = {
    state: () => state,
    dispatch(spec: TransactionSpec) {
      if (dispatching) throw new Error('Reentrant test host dispatch');
      dispatching = true;
      try {
        const tr = state.update(spec);
        state = tr.state;
        const update: VimUpdate = { state, changes: tr.changes, docChanged: tr.docChanged, selectionSet: tr.selection !== undefined, userEvent: tr.annotation(Transaction.userEvent) ?? null };
        for (const listener of [...listeners]) listener(update);
      } finally { dispatching = false; }
    },
    subscribe(cb) { listeners.add(cb); return () => listeners.delete(cb); },
    undo: () => undo({ state, dispatch: (tr) => host.dispatch(tr) }),
    redo: () => redo({ state, dispatch: (tr) => host.dispatch(tr) }),
    historyReduced: () => false,
    viewport: () => opts.viewport ?? { topLine: 1, rows: 10 },
    scrollLines: (lines) => { scrolls.push(lines); },
    scrollCaret: vi.fn(),
    clipboard,
    memoryClipboard,
    actions: { evalAtCursor: vi.fn(), evalAll: vi.fn(), hush: vi.fn(), format: vi.fn() },
    closePopupWithEscape: () => false,
    announce(message, assertive = false) { messages.push({ message, assertive }); },
    platform: opts.platform ?? 'other',
    matchesDefaultKeymap: (input) => opts.defaultKeymaps?.(input) ?? false,
    compositionActive: () => false,
    messages,
    scrolls,
  };
  return host;
}

export function show(state: EditorState): string {
  const pos = state.selection.main.head;
  return `${state.doc.sliceString(0, pos)}|${state.doc.sliceString(pos)}`;
}
