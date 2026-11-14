import { Text } from '@codemirror/state';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { EditorDeps } from '../../src/app/deps';
import { buildLayout } from '../../src/app/layout';
import { CodeSurface } from '../../src/code/surface';
import { DocumentStorage, LocalTemplateStore, type StorageBackend } from '../../src/documents/storage';
import { mount } from '../../src/documents/mount';
import { DocumentSync } from '../../src/code/sync';
import type { DefaultTemplate, DocumentsOptions, ExampleSource } from '../../src/documents/types';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { MockClock } from '../support/clock';
import { RecordingTransport } from '../support/recording';

class MemoryBackend implements StorageBackend {
  private values = new Map<string, string>();
  get length(): number { return this.values.size; }
  key(index: number): string | null { return [...this.values.keys()][index] ?? null; }
  getItem(key: string): string | null { return this.values.get(key) ?? null; }
  setItem(key: string, value: string): void { this.values.set(key, value); }
  removeItem(key: string): void { this.values.delete(key); }
}

const examples: ExampleSource = {
  defaultId: 'showcase',
  list: () => [
    { id: 'showcase', title: 'Showcase', description: 'Default', category: 'Showcase', requires: [] },
    { id: 'acid', title: 'Acid', description: 'Bass', category: 'Examples', requires: [] },
  ],
  find(id) { return this.list().find((entry) => entry.id === id); },
  async load(id) { if (!this.find(id)) throw new Error('unknown example'); return id === 'acid' ? 'ACID' : 'SHOWCASE'; },
};

function setup(shell: 'web' | 'app' = 'web', search = '', initialTemplate?: DefaultTemplate) {
  const root = document.createElement('div');
  document.body.appendChild(root);
  buildLayout(root);
  const store = new Store();
  const transport = new RecordingTransport();
  const client = new Client(transport, { store });
  const surface = new CodeSurface({ sync: new DocumentSync(client.document('main.vact'), Text.of([''])) });
  const files = new MemoryFiles({ 'x.vact': 'FILE' });
  const deps = { client, store, clock: new MockClock(), tier: 'browser' as const, files,
    code: { surface } } as unknown as EditorDeps;
  const documents = new DocumentStorage(new MemoryBackend());
  if (initialTemplate) documents.writeTemplate(initialTemplate);
  const opts: DocumentsOptions = { shell, store: documents, templates: new LocalTemplateStore(documents), examples, search };
  const area = mount(root, deps, opts);
  return { root, area, surface, transport, files, storage: documents, dispose() { area.dispose(); surface.dispose(); root.remove(); } };
}

afterEach(() => { vi.useRealTimers(); });

describe('documents area', () => {
  const clickButton = (root: HTMLElement, label: string) => {
    const button = [...root.querySelectorAll<HTMLButtonElement>('button')]
      .find((candidate) => candidate.textContent?.trim() === label);
    if (!button) throw new Error(`Missing button: ${label}`);
    button.click();
  };

  it('starts on the showcase without evaluating and opens another example as one undoable replacement', async () => {
    const env = setup();
    try {
      await env.area.ready;
      expect(env.surface.state.doc.toString()).toBe('SHOWCASE');
      expect(env.root.querySelector('.vact-document-title')?.textContent).toBe('Showcase');
      expect(env.root.querySelector('.vact-document-dirty')).toBeNull();
      expect(env.root.querySelector('[data-doc-picker]')).not.toBeNull();
      expect(env.transport.kinds()).not.toContain('eval');
      env.root.querySelector<HTMLButtonElement>('.vact-doc-picker-toggle')?.click();
      [...env.root.querySelectorAll<HTMLButtonElement>('.vact-doc-picker-item')]
        .find((button) => button.textContent?.includes('Acid'))?.click();
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(env.surface.state.doc.toString()).toBe('ACID');
      expect(env.root.querySelector('.vact-document-title')?.textContent).toBe('Acid');
      expect(env.root.querySelector('.vact-document-dirty')).toBeNull();
      expect(env.surface.undo()).toBe(true);
      expect(env.root.querySelector('.vact-document-dirty')?.getAttribute('aria-label')).toBe('Unsaved changes');
      expect(env.surface.state.doc.toString()).toBe('SHOWCASE');
    } finally { env.dispose(); }
  });

  it('confirms dirty replacement and keeps the previous buffer recoverable by undo', async () => {
    const env = setup();
    try {
      await env.area.ready;
      env.surface.dispatch({ changes: { from: 0, to: env.surface.state.doc.length, insert: 'EDITED' } });
      env.root.querySelector<HTMLButtonElement>('.vact-doc-picker-toggle')?.click();
      const acid = [...env.root.querySelectorAll<HTMLButtonElement>('.vact-doc-picker-item')]
        .find((button) => button.textContent?.includes('Acid'));
      acid?.click();
      expect(env.surface.state.doc.toString()).toBe('EDITED');
      expect(env.root.textContent).toContain('Replace the current document?');
      [...env.root.querySelectorAll<HTMLButtonElement>('button')].find((button) => button.textContent === 'Replace')?.click();
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(env.surface.state.doc.toString()).toBe('ACID');
      expect(env.surface.undo()).toBe(true);
      expect(env.surface.state.doc.toString()).toBe('EDITED');
    } finally { env.dispose(); }
  });

  it('opens a queued file after dirty confirmation as an undoable file source', async () => {
    const env = setup();
    try {
      await env.area.ready;
      env.surface.dispatch({ changes: { from: 0, to: env.surface.state.doc.length, insert: 'EDITED' } });
      env.files.queueOpen('x.vact');
      clickButton(env.root, 'Documents');
      clickButton(env.root, 'Open file...');
      expect(env.surface.state.doc.toString()).toBe('EDITED');
      expect(env.root.querySelector('.vact-doc-picker-confirm')).not.toBeNull();

      clickButton(env.root, 'Replace');
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(env.surface.state.doc.toString()).toBe('FILE');

      clickButton(env.root, 'Documents');
      clickButton(env.root, 'Use as default template');
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(env.storage.readTemplate()).toEqual({ kind: 'snapshot', name: 'x.vact', text: 'FILE' });
      expect(env.surface.undo()).toBe(true);
      expect(env.surface.state.doc.toString()).toBe('EDITED');
    } finally { env.dispose(); }
  });

  it('silently clears the dirty replacement confirmation when file open is cancelled', async () => {
    const env = setup();
    try {
      await env.area.ready;
      env.surface.dispatch({ changes: { from: 0, to: env.surface.state.doc.length, insert: 'EDITED' } });
      clickButton(env.root, 'Documents');
      clickButton(env.root, 'Open file...');
      expect(env.root.querySelector('.vact-doc-picker-confirm')).not.toBeNull();
      clickButton(env.root, 'Replace');
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(env.surface.state.doc.toString()).toBe('EDITED');
      expect(env.root.querySelector('.vact-doc-picker-status')?.textContent).toBe('');
      expect(env.root.querySelector('.vact-doc-picker-confirm')).toBeNull();
    } finally { env.dispose(); }
  });

  it('New falls back to the showcase when the saved template example is unregistered', async () => {
    const env = setup('web', '', { kind: 'example', id: 'missing' });
    try {
      await env.area.ready;
      clickButton(env.root, 'Documents');
      [...env.root.querySelectorAll<HTMLButtonElement>('.vact-doc-picker-item')]
        .find((button) => button.textContent?.includes('Acid'))?.click();
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(env.surface.state.doc.toString()).toBe('ACID');
      clickButton(env.root, 'Documents');
      clickButton(env.root, 'New');
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(env.surface.state.doc.toString()).toBe('SHOWCASE');
      expect(env.root.querySelector('.vact-doc-picker-status')?.textContent)
        .toContain('Default template example missing is no longer available');
    } finally { env.dispose(); }
  });

  it('saves, renames and deletes library entries and sets, uses and resets a template', async () => {
    const env = setup();
    try {
      await env.area.ready;
      clickButton(env.root, 'Documents');
      clickButton(env.root, 'Save as...');
      const name = env.root.querySelector<HTMLInputElement>('[aria-label="Document name"]');
      expect(name).not.toBeNull();
      name!.value = 'one';
      name!.dispatchEvent(new Event('input', { bubbles: true }));
      env.root.querySelector<HTMLFormElement>('.vact-doc-picker-form')?.querySelector<HTMLButtonElement>('button[type="submit"]')?.click();
      expect(env.storage.list().entries.map((entry) => entry.name)).toEqual(['one']);

      env.surface.dispatch({ changes: { from: 0, to: env.surface.state.doc.length, insert: 'UPDATED' } });
      clickButton(env.root, 'Save');
      const entry = env.storage.list().entries[0];
      expect(entry?.text).toBe('UPDATED');
      const row = env.root.querySelector('.vact-doc-picker-library-row');
      row?.querySelector<HTMLButtonElement>('button')?.nextElementSibling?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      const rename = env.root.querySelector<HTMLInputElement>('[aria-label="New document name"]');
      expect(rename).not.toBeNull();
      rename!.value = 'renamed';
      rename!.dispatchEvent(new Event('input', { bubbles: true }));
      env.root.querySelector<HTMLFormElement>('.vact-doc-picker-form')?.querySelector<HTMLButtonElement>('button[type="submit"]')?.click();
      expect(env.storage.list().entries[0]?.name).toBe('renamed');

      const libraryRow = env.root.querySelector('.vact-doc-picker-library-row');
      [...(libraryRow?.querySelectorAll<HTMLButtonElement>('button') ?? [])]
        .find((button) => button.textContent === 'Delete')?.click();
      clickButton(env.root, 'Confirm delete');
      expect(env.storage.list().entries).toEqual([]);

      clickButton(env.root, 'Use as default template');
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(env.storage.readTemplate()).toEqual({ kind: 'snapshot', name: 'Untitled', text: 'UPDATED' });
      clickButton(env.root, 'New');
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(env.surface.state.doc.toString()).toBe('UPDATED');
      clickButton(env.root, 'Documents');
      clickButton(env.root, 'Reset default template');
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(env.storage.readTemplate()).toBeNull();
    } finally { env.dispose(); }
  });

  it('autosaves dirty web text and clears the draft after a clean example load', async () => {
    vi.useFakeTimers();
    const env = setup();
    try {
      await env.area.ready;
      env.surface.dispatch({ changes: { from: 0, to: env.surface.state.doc.length, insert: 'DRAFT' } });
      await vi.advanceTimersByTimeAsync(1000);
      expect(env.storage.readDraft()?.text).toBe('DRAFT');
      clickButton(env.root, 'Documents');
      [...env.root.querySelectorAll<HTMLButtonElement>('.vact-doc-picker-item')]
        .find((button) => button.textContent?.includes('Acid'))?.click();
      clickButton(env.root, 'Replace');
      await vi.advanceTimersByTimeAsync(0);
      expect(env.surface.state.doc.toString()).toBe('ACID');
      expect(env.storage.readDraft()).toBeNull();
    } finally { env.dispose(); }
  });

  it('omits web library UI and draft autosave in the app shell', async () => {
    vi.useFakeTimers();
    const env = setup('app');
    try {
      await env.area.ready;
      env.surface.dispatch({ changes: { from: 0, to: env.surface.state.doc.length, insert: 'EDITED' } });
      await vi.advanceTimersByTimeAsync(2000);
      expect(env.root.textContent).not.toContain('Saved in this browser');
      expect(env.storage.readDraft()).toBeNull();
    } finally { env.dispose(); }
  });

  it('resolves without rendering when the code area has no API', async () => {
    const root = document.createElement('div');
    document.body.appendChild(root);
    buildLayout(root);
    const store = new Store();
    const deps = { store } as unknown as EditorDeps;
    const area = mount(root, deps, { shell: 'app', store: new DocumentStorage(null),
      templates: new LocalTemplateStore(new DocumentStorage(null)), examples, search: '' });
    try {
      await expect(area.ready).resolves.toBeUndefined();
      expect(root.querySelector('[data-doc-picker]')).toBeNull();
    } finally { area.dispose(); root.remove(); }
  });
});
