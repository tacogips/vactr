import { describe, expect, it } from 'vitest';
import {
  DRAFT_KEY,
  DocumentStorage,
  ITEM_PREFIX,
  LocalTemplateStore,
  normalizeName,
  pageBackend,
  PROBE_KEY,
  TEMPLATE_KEY,
  type StorageBackend,
} from '../../src/documents/storage';
import { DOC_MAX_CHARS, type DefaultTemplate, type Draft } from '../../src/documents/types';

class MemoryBackend implements StorageBackend {
  readonly values = new Map<string, string>();

  get length(): number {
    return this.values.size;
  }

  getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value);
  }

  removeItem(key: string): void {
    this.values.delete(key);
  }

  key(index: number): string | null {
    return [...this.values.keys()][index] ?? null;
  }
}

const makeStore = (backend = new MemoryBackend(), tick = 10) => {
  let current = tick;
  let id = 0;
  return {
    backend,
    store: new DocumentStorage(backend, () => current++, () => `id-${++id}`),
  };
};

const quotaError = () => new DOMException('full', 'QuotaExceededError');

function throwingBackend(error: Error): StorageBackend {
  return {
    get length(): number {
      throw error;
    },
    getItem() {
      throw error;
    },
    setItem() {
      throw error;
    },
    removeItem() {
      throw error;
    },
    key() {
      throw error;
    },
  };
}

describe('DocumentStorage', () => {
  it('creates, lists, reads, updates, renames, and removes named documents', () => {
    const { store } = makeStore();
    const created = store.create(' a ', 'first');
    expect(created.ok).toBe(true);
    if (!created.ok) return;
    expect(created.value.name).toBe('a');
    expect(store.list().entries).toEqual([created.value]);
    expect(store.read(created.value.id)?.text).toBe('first');

    expect(store.create('a', 'duplicate')).toEqual({ ok: false, reason: 'name-taken' });
    expect(store.update(created.value.id, 'updated').ok).toBe(true);
    const newer = store.create('b', 'newer');
    expect(newer.ok).toBe(true);
    expect(store.list().entries[0]?.name).toBe('b');
    expect(store.update('missing', 'x')).toEqual({ ok: false, reason: 'missing' });
    expect(store.rename(created.value.id, 'b')).toEqual({ ok: false, reason: 'name-taken' });
    expect(store.rename(created.value.id, ' a ').ok).toBe(true);
    expect(store.remove(created.value.id).ok).toBe(true);
    expect(store.remove(created.value.id)).toEqual({ ok: false, reason: 'missing' });
  });

  it('normalizes names and rejects empty, long, and control-character names', () => {
    expect(normalizeName('  name  ')).toBe('name');
    expect(storeName('   ')).toEqual({ ok: false, reason: 'invalid-name' });
    expect(storeName('x'.repeat(81))).toEqual({ ok: false, reason: 'invalid-name' });
    expect(storeName('bad\nname')).toEqual({ ok: false, reason: 'invalid-name' });

    function storeName(name: string) {
      return makeStore().store.create(name, 'text');
    }
  });

  it('rejects documents over DOC_MAX_CHARS for create, update, and draft writes', () => {
    const { store } = makeStore();
    const oversized = 'x'.repeat(DOC_MAX_CHARS + 1);
    expect(store.create('large', oversized)).toEqual({ ok: false, reason: 'too-large' });
    const created = store.create('small', 'x');
    expect(created.ok).toBe(true);
    if (created.ok) expect(store.update(created.value.id, oversized)).toEqual({ ok: false, reason: 'too-large' });
    const draft: Draft = { text: oversized, source: { kind: 'new' }, savedAt: 1 };
    expect(store.writeDraft(draft)).toEqual({ ok: false, reason: 'too-large' });
  });

  it('skips corrupt and foreign-version items without deleting their raw values', () => {
    const { backend, store } = makeStore();
    backend.setItem(`${ITEM_PREFIX}bad-json`, '{');
    backend.setItem(`${ITEM_PREFIX}foreign`, JSON.stringify({ v: 2, id: 'foreign' }));
    expect(store.list()).toEqual({ entries: [], unreadable: 2 });
    expect(backend.getItem(`${ITEM_PREFIX}bad-json`)).toBe('{');
    expect(backend.getItem(`${ITEM_PREFIX}foreign`)).toBe(JSON.stringify({ v: 2, id: 'foreign' }));
  });

  it('ignores an invalid draft without deleting it and round-trips a valid draft', () => {
    const { backend, store } = makeStore();
    const invalid = JSON.stringify({ v: 1, text: 'x', source: { kind: 'bogus' }, savedAt: 1 });
    backend.setItem(DRAFT_KEY, invalid);
    expect(store.readDraft()).toBeNull();
    expect(backend.getItem(DRAFT_KEY)).toBe(invalid);

    const draft: Draft = { text: 'draft', source: { kind: 'example', id: 'showcase' }, savedAt: 42 };
    expect(store.writeDraft(draft)).toEqual({ ok: true });
    expect(store.readDraft()).toEqual(draft);
    store.clearDraft();
    expect(store.readDraft()).toBeNull();
  });

  it('maps quota failures and keeps storage available', () => {
    const backend = new MemoryBackend();
    const { store } = makeStore(backend);
    let shouldFail = false;
    const original = backend.setItem.bind(backend);
    backend.setItem = (key, value) => {
      if (shouldFail && key.startsWith(ITEM_PREFIX)) throw quotaError();
      original(key, value);
    };
    shouldFail = true;
    expect(store.create('quota', 'text')).toEqual({ ok: false, reason: 'quota', message: 'QuotaExceededError: full' });
    expect(store.available).toBe(true);
    expect(store.writeDraft({ text: 'x', source: { kind: 'new' }, savedAt: 1 }).ok).toBe(true);
    expect(store.writeDraft({ text: 'x'.repeat(DOC_MAX_CHARS + 1), source: { kind: 'new' }, savedAt: 1 })).toEqual({ ok: false, reason: 'too-large' });
  });

  it('treats throwing SecurityError storage as unavailable and keeps calls safe', () => {
    const store = new DocumentStorage(throwingBackend(new DOMException('denied', 'SecurityError')));
    expect(store.available).toBe(false);
    expect(store.list()).toEqual({ entries: [], unreadable: 0 });
    expect(store.create('name', 'text')).toEqual({ ok: false, reason: 'unavailable' });
    expect(store.update('id', 'text')).toEqual({ ok: false, reason: 'unavailable' });
    expect(store.rename('id', 'name')).toEqual({ ok: false, reason: 'unavailable' });
    expect(store.remove('id')).toEqual({ ok: false, reason: 'unavailable' });
    expect(store.writeDraft({ text: 'x', source: { kind: 'new' }, savedAt: 1 })).toEqual({ ok: false, reason: 'unavailable' });
    expect(store.readDraft()).toBeNull();
    store.clearDraft();
  });

  it('handles a null backend without throwing', () => {
    const store = new DocumentStorage(null);
    expect(store.available).toBe(false);
    expect(store.list()).toEqual({ entries: [], unreadable: 0 });
    expect(store.create('name', 'text')).toEqual({ ok: false, reason: 'unavailable' });
    expect(store.update('id', 'text')).toEqual({ ok: false, reason: 'unavailable' });
    expect(store.rename('id', 'name')).toEqual({ ok: false, reason: 'unavailable' });
    expect(store.remove('id')).toEqual({ ok: false, reason: 'unavailable' });
    expect(store.writeDraft({ text: 'x', source: { kind: 'new' }, savedAt: 1 })).toEqual({ ok: false, reason: 'unavailable' });
    store.clearDraft();
  });

  it('returns null when reading a window whose localStorage getter throws', () => {
    const win = Object.defineProperty({}, 'localStorage', {
      get() {
        throw new DOMException('denied', 'SecurityError');
      },
    }) as Window;
    expect(pageBackend(win)).toBeNull();
    expect(pageBackend(null)).toBeNull();
  });

  it('uses only the v1 keys and probes storage with the exported probe key', () => {
    const { backend, store } = makeStore();
    expect(store.available).toBe(true);
    expect(backend.getItem(PROBE_KEY)).toBeNull();
    expect(DRAFT_KEY).toBe('vactr.doc.v1.draft');
    expect(ITEM_PREFIX).toBe('vactr.doc.v1.item.');
    expect(TEMPLATE_KEY).toBe('vactr.doc.v1.default-template');
  });
});

describe('LocalTemplateStore', () => {
  it('sets, gets, validates, resets, and handles unavailable storage', async () => {
    const { backend, store } = makeStore();
    const templates = new LocalTemplateStore(store);
    const example: DefaultTemplate = { kind: 'example', id: 'showcase' };
    expect(await templates.set(example)).toEqual({ ok: true });
    expect(await templates.get()).toEqual(example);

    const snapshot: DefaultTemplate = { kind: 'snapshot', name: 'Saved', text: 'x'.repeat(DOC_MAX_CHARS + 1) };
    expect(await templates.set(snapshot)).toEqual({ ok: false, reason: 'too-large' });
    expect(await templates.get()).toEqual(example);

    backend.setItem(TEMPLATE_KEY, '{');
    expect(await templates.get()).toBeNull();
    expect(backend.getItem(TEMPLATE_KEY)).toBe('{');
    expect(await templates.reset()).toEqual({ ok: true });
    expect(await templates.get()).toBeNull();

    const unavailable = new LocalTemplateStore(new DocumentStorage(null));
    expect(await unavailable.get()).toBeNull();
    expect(await unavailable.set(example)).toEqual({ ok: false, reason: 'unavailable' });
    expect(await unavailable.reset()).toEqual({ ok: false, reason: 'unavailable' });
  });
});
