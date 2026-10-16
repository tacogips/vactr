import { Text } from '@codemirror/state';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { RecordingTransport } from '../support/recording';
import type { DefaultTemplate, Draft, DraftStore, ExampleSource, StorageResult, TemplateStore } from '../../src/documents/types';
import { DocumentSession } from '../../src/documents/session';
import { DraftAutosave } from '../../src/documents/autosave';
import { applyStartup, resolveStartup, type StartupInput } from '../../src/documents/startup';

const entries = ['acid-techno', 'fm6-algorithms', 'showcase'];
const draft: Draft = { text: 'D', source: { kind: 'library', id: 'saved-1', name: 'Draft' }, savedAt: 7 };
function examples(overrides: Partial<ExampleSource> = {}): ExampleSource {
  return {
    defaultId: 'showcase',
    list: () => entries.map((id) => ({ id, title: id, description: '', category: 'Examples', requires: [] })),
    find: (id) => entries.includes(id) ? { id, title: id, description: '', category: 'Examples', requires: [] } : undefined,
    load: async (id) => id.toUpperCase(),
    ...overrides,
  };
}
function makeDrafts(value: Draft | null = null) {
  const writes: Draft[] = [];
  let cleared = 0;
  const store: DraftStore = {
    readDraft: () => value,
    writeDraft: (item) => { writes.push(item); return { ok: true }; },
    clearDraft: () => { cleared++; value = null; },
  };
  return { store, writes, get cleared() { return cleared; } };
}
function templates(value: DefaultTemplate | null = null, get?: () => Promise<DefaultTemplate | null>): TemplateStore {
  return { get: get ?? (async () => value), set: async (): Promise<StorageResult> => ({ ok: true }), reset: async (): Promise<StorageResult> => ({ ok: true }) };
}
function input(overrides: Partial<StartupInput> = {}): StartupInput {
  return { shell: 'web', search: '', examples: examples(), drafts: makeDrafts().store, templates: templates(), notice: vi.fn(), ...overrides };
}
function makeSurface(): CodeSurface {
  const client = new Client(new RecordingTransport(), { store: new Store() });
  return new CodeSurface({ sync: new DocumentSync(client.document('main.vact'), Text.of([''])) });
}

describe('resolveStartup', () => {
  it('loads a URL example over a web draft while retaining the draft as underlying', async () => {
    const result = await resolveStartup(input({ search: '?example=acid-techno', drafts: makeDrafts(draft).store }));
    expect(result).toEqual({ text: 'ACID-TECHNO', source: { kind: 'example', id: 'acid-techno' }, restored: false, underlying: draft });
  });

  it('does not read an app draft when resolving URL or ordinary startup', async () => {
    const drafts = makeDrafts(draft);
    const result = await resolveStartup(input({ shell: 'app', search: '?example=acid-techno', drafts: drafts.store }));
    expect(result?.underlying).toBeUndefined();
    expect(result?.source).toEqual({ kind: 'example', id: 'acid-techno' });
    expect(await resolveStartup(input({ shell: 'app', drafts: drafts.store }))).toEqual({ text: 'SHOWCASE', source: { kind: 'example', id: 'showcase' }, restored: false });
  });

  it('falls through unknown or rejected URL examples to the web draft', async () => {
    const notice = vi.fn();
    const unknown = await resolveStartup(input({ search: '?example=nope', drafts: makeDrafts(draft).store, notice }));
    expect(unknown).toEqual({ text: 'D', source: draft.source, restored: true });
    expect(notice).toHaveBeenCalledWith('Unknown example nope');
    const empty = await resolveStartup(input({ search: '?example=', notice }));
    expect(empty?.source).toEqual({ kind: 'example', id: 'showcase' });
    expect(notice).toHaveBeenCalledWith('Unknown example ');
    const rejected = await resolveStartup(input({
      search: '?example=acid-techno', drafts: makeDrafts(draft).store,
      examples: examples({ load: async () => { throw new Error('chunk'); } }), notice,
    }));
    expect(rejected).toEqual({ text: 'D', source: draft.source, restored: true });
  });

  it('uses a valid web draft before templates and resolves example or snapshot templates', async () => {
    expect(await resolveStartup(input({ drafts: makeDrafts(draft).store }))).toEqual({ text: 'D', source: draft.source, restored: true });
    expect(await resolveStartup(input({ templates: templates({ kind: 'example', id: 'fm6-algorithms' }) }))).toEqual({
      text: 'FM6-ALGORITHMS', source: { kind: 'example', id: 'fm6-algorithms' }, restored: false,
    });
    expect(await resolveStartup(input({ templates: templates({ kind: 'snapshot', name: 'Frozen', text: 'snapshot' }) }))).toEqual({
      text: 'snapshot', source: { kind: 'template' }, restored: false,
    });
  });

  it('falls through invalid template examples and rejected template stores to showcase', async () => {
    const notice = vi.fn();
    expect(await resolveStartup(input({ templates: templates({ kind: 'example', id: 'gone' }), notice }))).toEqual({
      text: 'SHOWCASE', source: { kind: 'example', id: 'showcase' }, restored: false,
    });
    expect(notice).toHaveBeenCalledWith('Default template example gone is no longer available');
    expect(await resolveStartup(input({ templates: templates(null, async () => { throw new Error('store'); }) }))).toEqual({
      text: 'SHOWCASE', source: { kind: 'example', id: 'showcase' }, restored: false,
    });
  });

  it('returns null and a notice rather than rejecting when every source fails', async () => {
    const notice = vi.fn();
    const result = await resolveStartup(input({ examples: examples({ load: async () => { throw new Error('chunk'); } }), notice }));
    expect(result).toBeNull();
    expect(notice).toHaveBeenCalledWith('No startup document could be loaded');
  });
});

describe('applyStartup', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('loads draft then URL example as one undoable step and autosaves draft after undo', () => {
    const surface = makeSurface();
    const session = new DocumentSession(surface);
    const drafts = makeDrafts(draft);
    const autosave = new DraftAutosave({ surface, session, drafts: drafts.store, win: null, now: () => 99, notice: vi.fn() });
    const notice = vi.fn();
    const result = applyStartup(session, { text: 'A', source: { kind: 'example', id: 'acid-techno' }, restored: false, underlying: draft }, notice);
    expect(result).toBe(true);
    expect(surface.state.doc.toString()).toBe('A');
    expect(surface.retentionStatus.undoDepth).toBe(1);
    expect(notice).toHaveBeenCalledWith('Opened acid-techno; Undo (Mod-Z) restores your unsaved draft');
    expect(drafts.cleared).toBe(1);
    expect(surface.undo()).toBe(true);
    expect(surface.state.doc.toString()).toBe('D');
    expect(session.dirty()).toBe(true);
    vi.advanceTimersByTime(1000);
    expect(drafts.writes).toEqual([{ text: 'D', source: session.source, savedAt: 99 }]);
    autosave.dispose(); session.dispose(); surface.dispose();
  });

  it('refuses to overwrite edits made before startup application', () => {
    const surface = makeSurface();
    const session = new DocumentSession(surface);
    surface.dispatch({ changes: { from: 0, insert: 'typed' } });
    const notice = vi.fn();
    expect(applyStartup(session, { text: 'A', source: { kind: 'example', id: 'acid-techno' }, restored: false, underlying: draft }, notice)).toBe(false);
    expect(surface.state.doc.toString()).toBe('typed');
    expect(notice).not.toHaveBeenCalled();
    surface.dispose(); session.dispose();
  });
});
