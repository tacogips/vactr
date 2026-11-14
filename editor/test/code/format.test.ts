import { defaultKeymap, historyKeymap } from '@codemirror/commands';
import { Text, Transaction } from '@codemirror/state';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { EditorDeps } from '../../src/app/deps';
import { buildLayout } from '../../src/app/layout';
import { mount } from '../../src/code/mount';
import { formatDocument, formatKeymap, FORMAT_KEY, minimalChange, type Formatter } from '../../src/code/format';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { MockClock } from '../support/clock';
import { RecordingTransport } from '../support/recording';

const views: CodeSurface[] = [];

interface NodeFs {
  readFileSync(path: string): Uint8Array<ArrayBuffer> | string;
}

const proc = (globalThis as unknown as { process: { cwd(): string } }).process;

afterEach(() => {
  for (const view of views.splice(0)) view.dispose();
  document.body.replaceChildren();
});

function viewWith(text: string): CodeSurface {
  const client = new Client(new RecordingTransport(), { store: new Store() });
  const sync = new DocumentSync(client.document('main.vact'), Text.of(text.split('\n')));
  const view = new CodeSurface({ sync });
  const transactions: { changes: Transaction['changes']; userEvent: string | null }[] = [];
  view.subscribe((update) => { if (update.docChanged) transactions.push({ changes: update.changes, userEvent: update.userEvent }); });
  (view as CodeSurface & { seenTransactions: typeof transactions }).seenTransactions = transactions;
  views.push(view);
  return view;
}

function seen(view: CodeSurface): { changes: Transaction['changes']; userEvent: string | null }[] {
  return (view as CodeSurface & { seenTransactions: { changes: Transaction['changes']; userEvent: string | null }[] }).seenTransactions;
}

function deps(): EditorDeps {
  const store = new Store();
  return {
    client: new Client(new RecordingTransport(), { store }),
    store,
    clock: new MockClock(),
    tier: 'native',
    files: new MemoryFiles(),
  };
}

describe('formatDocument', () => {
  it('dispatches one minimal format transaction with the format user event', async () => {
    const view = viewWith('a\n\t\t\tb\n');
    const formatter: Formatter = { format: async () => ({ status: 0, text: 'a\n\tb\n' }) };
    await expect(formatDocument(view, formatter)).resolves.toBe(true);
    expect(view.state.doc.toString()).toBe('a\n\tb\n');
    expect(seen(view)).toHaveLength(1);
    expect(seen(view)[0]?.userEvent).toBe('format');
    const changes: { from: number; to: number; insert: string }[] = [];
    seen(view)[0]?.changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
      changes.push({ from: fromA, to: toA, insert: inserted.toString() });
    });
    expect(changes).toEqual([{ from: 3, to: 5, insert: '' }]);
  });

  it('does not dispatch a refused or unchanged result', async () => {
    const view = viewWith('let x 1');
    await expect(formatDocument(view, { format: async (text) => ({ status: 1, text }) })).resolves.toBe(false);
    await expect(formatDocument(view, { format: async (text) => ({ status: 0, text }) })).resolves.toBe(false);
    expect(seen(view)).toHaveLength(0);
  });

  it('does not apply a result after the document changes while formatting', async () => {
    const view = viewWith('let x 1');
    let finish!: (value: { status: number; text: string }) => void;
    const pending = formatDocument(view, { format: () => new Promise((resolve) => (finish = resolve)) });
    view.dispatch({ changes: { from: 0, insert: '# changed\n' } });
    finish({ status: 0, text: 'let x 1\n' });
    await expect(pending).resolves.toBe(false);
    expect(view.state.doc.toString()).toBe('# changed\nlet x 1');
    expect(seen(view)).toHaveLength(1);
  });

  it('catches formatter rejection', async () => {
    const view = viewWith('let x 1');
    await expect(formatDocument(view, { format: async () => Promise.reject(new Error('offline')) })).resolves.toBe(false);
    expect(seen(view)).toHaveLength(0);
  });
});

describe('formatKeymap', () => {
  it('does not conflict with the default or history keymaps and reports unavailable without a formatter', () => {
    expect(FORMAT_KEY).toBe('Shift-Alt-f');
    expect([...defaultKeymap, ...historyKeymap].some((binding) => binding.key === FORMAT_KEY)).toBe(false);
    const view = viewWith('');
    const dispose = formatKeymap(view, () => undefined);
    views.push(view);
    const event = new KeyboardEvent('keydown', { key: 'f', altKey: true, shiftKey: true, bubbles: true });
    expect(view.runKeymaps(event)).toBe(false);
    dispose();
  });

  it('formats through the mounted code pane keymap', async () => {
    const root = document.createElement('div');
    document.body.append(root);
    buildLayout(root);
    const editorDeps = deps();
    const format = vi.fn(async (text: string) => ({ status: 0, text: 'let x 1\n' }));
    editorDeps.formatter = { format };
    const mounted = mount(root, editorDeps);
    const view = editorDeps.code?.surface as CodeSurface | undefined;
    expect(view).toBeDefined();
    view?.dispatch({ changes: { from: 0, insert: 'let x 1' } });
    const event = new KeyboardEvent('keydown', { key: 'f', altKey: true, shiftKey: true, bubbles: true });
    expect(view?.runKeymaps(event)).toBe(true);
    await vi.waitFor(() => expect(view?.state.doc.toString()).toBe('let x 1\n'));
    expect(format).toHaveBeenCalledWith('let x 1');
    mounted.dispose();
    if (view) views.splice(views.indexOf(view), 1);
  });
});

describe('minimalChange', () => {
  it('returns null for identical text and the smallest range for changed text', () => {
    expect(minimalChange('abc', 'abc')).toBeNull();
    const change = minimalChange('a  b', 'a b');
    expect(change).toEqual({ from: 2, to: 3, insert: '' });
    expect(change && 'a  b'.slice(0, change.from) + change.insert + 'a  b'.slice(change.to)).toBe('a b');
  });

  it('keeps Japanese text around the minimal replacement intact', () => {
    const before = '前の音量後';
    const after = '前の音高後';
    const change = minimalChange(before, after);
    expect(change).not.toBeNull();
    expect(before.slice(0, change!.from) + change!.insert + before.slice(change!.to)).toBe(after);
  });

  it('keeps the extracted core modules free of CodeMirror imports', async () => {
    const spec: string = 'node:fs';
    const fs = (await import(/* @vite-ignore */ spec)) as NodeFs;
    const readText = (path: string) => {
      const value = fs.readFileSync(path);
      return typeof value === 'string' ? value : new TextDecoder().decode(value);
    };
    const root = proc.cwd();
    expect(readText(`${root}/src/code/format-core.ts`)).not.toContain('@codemirror/');
    expect(readText(`${root}/src/code/tool-wasm.ts`)).not.toContain('@codemirror/');
  });
});
