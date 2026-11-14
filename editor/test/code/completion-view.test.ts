// @vitest-environment jsdom
import { Text, Transaction } from '@codemirror/state';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CompletionEngine, RawCompletion } from '../../src/code/completion-types';
import { attachCompletion } from '../../src/code/completion-view';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import type { EditorDeps } from '../../src/app/deps';
import { buildLayout } from '../../src/app/layout';
import { mount } from '../../src/code/mount';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { MockClock } from '../support/clock';
import { RecordingTransport } from '../support/recording';

const views: CodeSurface[] = [];
const attachments: Array<{ dispose(): void }> = [];
const fsSpec: string = 'node:fs';
const fs = await import(/* @vite-ignore */ fsSpec) as { readFileSync(path: string): Uint8Array<ArrayBuffer> | string };
const proc = (globalThis as unknown as { process: { cwd(): string } }).process;

afterEach(() => {
  for (const attachment of attachments.splice(0)) attachment.dispose();
  for (const view of views.splice(0)) view.dispose();
  document.body.replaceChildren();
});

function engine(result: (text: string) => RawCompletion | null = (text) => ({
  v: 1,
  context: 'head',
  from: Math.max(0, text.length - 1),
  to: text.length,
  incomplete: false,
  items: [{ label: 'alpha', kind: 'local', detail: 'parameter', insert: 'alpha' }],
})): CompletionEngine {
  return { complete: vi.fn(async (text) => result(text)) };
}

function viewWith(text = ''): CodeSurface {
  const client = new Client(new RecordingTransport(), { store: new Store() });
  const sync = new DocumentSync(client.document('main.vact'), Text.of(text.split('\n')));
  const view = new CodeSurface({ sync });
  view.attachBridge({ focus() {}, posAtCoords: () => 0, coordsAtPos: () => ({ left: 0, right: 0, top: 0, bottom: 0 }) });
  views.push(view);
  return view;
}

function type(view: CodeSurface, text: string): void {
  const end = view.state.selection.main.head;
  view.dispatch({ changes: { from: end, insert: text }, selection: { anchor: end + text.length }, annotations: Transaction.userEvent.of('input.type') });
}

async function flushPopup(): Promise<void> {
  await vi.waitFor(() => expect(document.querySelector('.vact-completion')).not.toBeNull());
}

function key(view: CodeSurface, name: string): boolean {
  return view.runKeymaps(new KeyboardEvent('keydown', { key: name }));
}

describe('completion EditorView adapter', () => {
  it('shows candidates while typing, accepts Enter while open, and inserts a newline while closed', async () => {
    const view = viewWith();
    attachments.push(attachCompletion(view, engine()));
    type(view, 'a');
    await flushPopup();
    expect(key(view, 'Enter')).toBe(true);
    expect(view.state.doc.toString()).toBe('alpha');
    expect(view.state.doc.toString()).not.toContain('\n');
    expect(document.querySelector('.vact-completion')).toBeNull();

    expect(key(view, 'Enter')).toBe(false);
    view.dispatch({ changes: { from: view.state.doc.length, insert: '\n' } });
    expect(view.state.doc.toString()).toBe('alpha\n');
  });

  it('registers only the completion keys and keeps unrelated eval/format keys out', () => {
    const view = viewWith();
    const addKeymap = vi.spyOn(view, 'addKeymap');
    attachments.push(attachCompletion(view, engine()));
    const added = addKeymap.mock.calls.flatMap(([bindings]) => bindings.map((binding) => binding.key));
    expect(added).toEqual(['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Enter', 'Tab', 'Escape', 'Ctrl-Space']);
    expect(addKeymap.mock.calls.every(([, precedence]) => precedence === 'highest')).toBe(true);
    expect(added).not.toContain('Mod-Enter');
    expect(added).not.toContain('Mod-Shift-Enter');
    expect(added).not.toContain('Mod-.');
    expect(added).not.toContain('Shift-Alt-f');
  });

  it('ignores an inverted stale replacement range instead of dispatching an invalid transaction', async () => {
    const view = viewWith();
    attachments.push(attachCompletion(view, engine((text) => ({
      v: 1, context: 'head', from: 1, to: text.length, incomplete: false,
      items: [{ label: 'alpha', kind: 'local', detail: '', insert: 'alpha' }],
    }))));
    type(view, 'a');
    await flushPopup();
    view.dispatch({ changes: { from: 0, insert: 'x' }, selection: { anchor: 0 } });
    expect(() => key(view, 'Enter')).not.toThrow();
    expect(view.state.doc.toString()).toBe('xa');
  });

  it('disposes the popup and stops listening for later typing', async () => {
    const view = viewWith();
    const attachment = attachCompletion(view, engine());
    attachments.push(attachment);
    type(view, 'a');
    await flushPopup();
    attachment.dispose();
    expect(document.querySelector('.vact-completion')).toBeNull();
    type(view, 'b');
    await Promise.resolve();
    expect(document.querySelector('.vact-completion')).toBeNull();
  });

  it('keeps editing available when the completion engine rejects', async () => {
    const failing: CompletionEngine = { complete: async () => { throw new Error('offline'); } };
    const view = viewWith();
    attachments.push(attachCompletion(view, failing));
    type(view, 'a');
    await vi.waitFor(() => expect(view.state.doc.toString()).toBe('a'));
    await Promise.resolve();
    expect(document.querySelector('.vact-completion')).toBeNull();
    type(view, 'b');
    expect(view.state.doc.toString()).toBe('ab');
  });

  it('wires optional completion through mount and disposes without error', () => {
    const root = document.createElement('div');
    document.body.append(root);
    buildLayout(root);
    const store = new Store();
    const deps: EditorDeps = {
      client: new Client(new RecordingTransport(), { store }),
      store,
      clock: new MockClock(),
      tier: 'native',
      files: new MemoryFiles(),
      completion: engine(),
    };
    const mounted = mount(root, deps);
    const view = deps.code?.surface;
    expect(view).toBeDefined();
    expect(() => mounted.dispose()).not.toThrow();
  });

  it('keeps completion service, popup and types free of EditorView imports', () => {
    const paths = ['completion.ts', 'completion-popup.ts', 'completion-types.ts'];
    for (const path of paths) {
      const source = fs.readFileSync(`${proc.cwd()}/src/code/${path}`);
      const text = typeof source === 'string' ? source : new TextDecoder().decode(source);
      expect(text).not.toContain('@codemirror/view');
    }
  });
});
