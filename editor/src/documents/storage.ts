import {
  DOC_MAX_CHARS,
  isDefaultTemplate,
  isDocSource,
  type DefaultTemplate,
  type DocumentStore,
  type Draft,
  type LibraryEntry,
  type StorageResult,
  type TemplateStore,
} from './types';

export type StorageBackend = Pick<
  Storage,
  'getItem' | 'setItem' | 'removeItem' | 'key' | 'length'
>;

export const DRAFT_KEY = 'vactr.doc.v1.draft';
export const ITEM_PREFIX = 'vactr.doc.v1.item.';
export const TEMPLATE_KEY = 'vactr.doc.v1.default-template';
export const PROBE_KEY = 'vactr.doc.v1.probe';

const CONTROL_CHARACTERS = /[\u0000-\u001f\u007f]/;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isLibraryEntry(value: unknown): value is LibraryEntry {
  return (
    isRecord(value) &&
    value.v === 1 &&
    typeof value.id === 'string' &&
    value.id.length > 0 &&
    typeof value.name === 'string' &&
    normalizeName(value.name) === value.name &&
    typeof value.text === 'string' &&
    value.text.length <= DOC_MAX_CHARS &&
    typeof value.createdAt === 'number' &&
    Number.isFinite(value.createdAt) &&
    typeof value.updatedAt === 'number' &&
    Number.isFinite(value.updatedAt)
  );
}

function isDraft(value: unknown): value is Draft & { v: 1 } {
  return (
    isRecord(value) &&
    value.v === 1 &&
    typeof value.text === 'string' &&
    value.text.length <= DOC_MAX_CHARS &&
    isDocSource(value.source) &&
    typeof value.savedAt === 'number' &&
    Number.isFinite(value.savedAt)
  );
}

function isQuotaError(error: unknown): boolean {
  if (!isRecord(error)) return false;
  return (
    error.name === 'QuotaExceededError' ||
    error.name === 'NS_ERROR_DOM_QUOTA_REACHED' ||
    error.code === 22 ||
    error.code === 1014
  );
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function pageBackend(win: Window | null): StorageBackend | null {
  try {
    return win?.localStorage ?? null;
  } catch {
    return null;
  }
}

export function normalizeName(name: string): string | null {
  const normalized = name.trim();
  if (normalized.length < 1 || normalized.length > 80 || CONTROL_CHARACTERS.test(normalized)) {
    return null;
  }
  return normalized;
}

export class DocumentStorage implements DocumentStore {
  readonly available: boolean;
  private readonly backend: StorageBackend | null;
  private readonly now: () => number;
  private readonly newId?: () => string;

  constructor(backend: StorageBackend | null, now: () => number = Date.now, newId?: () => string) {
    this.backend = backend;
    this.now = now;
    this.newId = newId;

    if (!backend) {
      this.available = false;
      return;
    }

    try {
      backend.setItem(PROBE_KEY, '1');
      backend.removeItem(PROBE_KEY);
      this.available = true;
    } catch (error) {
      this.available = isQuotaError(error);
      if (this.available) {
        try {
          backend.removeItem(PROBE_KEY);
        } catch {
          // Quota means reads are still available; cleanup is best-effort.
        }
      }
    }
  }

  list(): { entries: LibraryEntry[]; unreadable: number } {
    if (!this.available || !this.backend) return { entries: [], unreadable: 0 };
    try {
      const keys: string[] = [];
      const length = this.backend.length;
      for (let index = 0; index < length; index += 1) {
        const key = this.backend.key(index);
        if (key?.startsWith(ITEM_PREFIX)) keys.push(key);
      }

      const entries: LibraryEntry[] = [];
      let unreadable = 0;
      for (const key of keys) {
        try {
          const raw = this.backend.getItem(key);
          if (raw === null) continue;
          const parsed: unknown = JSON.parse(raw);
          if (isLibraryEntry(parsed) && key === ITEM_PREFIX + parsed.id) {
            entries.push(this.toEntry(parsed));
          } else {
            unreadable += 1;
          }
        } catch {
          unreadable += 1;
        }
      }
      entries.sort((left, right) => right.updatedAt - left.updatedAt);
      return { entries, unreadable };
    } catch {
      return { entries: [], unreadable: 0 };
    }
  }

  read(id: string): LibraryEntry | null {
    try {
      const raw = this.backend?.getItem(ITEM_PREFIX + id);
      if (raw === null || raw === undefined) return null;
      const parsed: unknown = JSON.parse(raw);
      return isLibraryEntry(parsed) && parsed.id === id ? this.toEntry(parsed) : null;
    } catch {
      return null;
    }
  }

  create(name: string, text: string): StorageResult<LibraryEntry> {
    try {
      const normalized = normalizeName(name);
      if (normalized === null) return { ok: false, reason: 'invalid-name' };
      if (typeof text !== 'string' || text.length > DOC_MAX_CHARS) {
        return { ok: false, reason: 'too-large' };
      }
      if (this.list().entries.some((entry) => entry.name === normalized)) {
        return { ok: false, reason: 'name-taken' };
      }
      if (!this.backend || !this.available) return { ok: false, reason: 'unavailable' };
      const id = this.createId();
      if (!id) return { ok: false, reason: 'error', message: 'Could not create a document id.' };
      if (this.backend.getItem(ITEM_PREFIX + id) !== null) {
        return { ok: false, reason: 'error', message: 'Document id already exists.' };
      }
      const timestamp = this.timestamp();
      const entry: LibraryEntry = {
        id,
        name: normalized,
        text,
        createdAt: timestamp,
        updatedAt: timestamp,
      };
      return this.writeItem(entry);
    } catch (error) {
      return this.failure(error);
    }
  }

  update(id: string, text: string): StorageResult<LibraryEntry> {
    try {
      if (typeof text !== 'string' || text.length > DOC_MAX_CHARS) {
        return { ok: false, reason: 'too-large' };
      }
      if (!this.backend || !this.available) return { ok: false, reason: 'unavailable' };
      const current = this.read(id);
      if (!current) return { ok: false, reason: 'missing' };
      return this.writeItem({ ...current, text, updatedAt: this.timestamp() });
    } catch (error) {
      return this.failure(error);
    }
  }

  rename(id: string, name: string): StorageResult<LibraryEntry> {
    try {
      const normalized = normalizeName(name);
      if (normalized === null) return { ok: false, reason: 'invalid-name' };
      if (!this.backend || !this.available) return { ok: false, reason: 'unavailable' };
      const current = this.read(id);
      if (!current) return { ok: false, reason: 'missing' };
      if (
        normalized !== current.name &&
        this.list().entries.some((entry) => entry.id !== id && entry.name === normalized)
      ) {
        return { ok: false, reason: 'name-taken' };
      }
      return this.writeItem({ ...current, name: normalized, updatedAt: this.timestamp() });
    } catch (error) {
      return this.failure(error);
    }
  }

  remove(id: string): StorageResult {
    try {
      if (!this.backend || !this.available) return { ok: false, reason: 'unavailable' };
      if (!this.read(id)) return { ok: false, reason: 'missing' };
      return this.removeKey(ITEM_PREFIX + id);
    } catch (error) {
      return this.failure(error);
    }
  }

  readDraft(): Draft | null {
    try {
      const raw = this.backend?.getItem(DRAFT_KEY);
      if (raw === null || raw === undefined) return null;
      const parsed: unknown = JSON.parse(raw);
      if (!isDraft(parsed)) return null;
      return { text: parsed.text, source: parsed.source, savedAt: parsed.savedAt };
    } catch {
      return null;
    }
  }

  writeDraft(draft: Draft): StorageResult {
    try {
      if (typeof draft.text !== 'string' || draft.text.length > DOC_MAX_CHARS) {
        return { ok: false, reason: 'too-large' };
      }
      if (!isDocSource(draft.source) || !Number.isFinite(draft.savedAt)) {
        return { ok: false, reason: 'error', message: 'Invalid draft.' };
      }
      return this.setValue(DRAFT_KEY, { v: 1, ...draft });
    } catch (error) {
      return this.failure(error);
    }
  }

  clearDraft(): void {
    try {
      this.backend?.removeItem(DRAFT_KEY);
    } catch {
      // Clearing a stale draft is best-effort.
    }
  }

  readTemplate(): DefaultTemplate | null {
    try {
      const raw = this.backend?.getItem(TEMPLATE_KEY);
      if (raw === null || raw === undefined) return null;
      const parsed: unknown = JSON.parse(raw);
      if (!isRecord(parsed) || parsed.v !== 1 || !isDefaultTemplate(parsed.template)) return null;
      return parsed.template;
    } catch {
      return null;
    }
  }

  writeTemplate(template: DefaultTemplate): StorageResult {
    try {
      if (
        isRecord(template) &&
        template.kind === 'snapshot' &&
        typeof template.text === 'string' &&
        template.text.length > DOC_MAX_CHARS
      ) {
        return { ok: false, reason: 'too-large' };
      }
      if (!isDefaultTemplate(template)) return { ok: false, reason: 'invalid-name' };
      return this.setValue(TEMPLATE_KEY, { v: 1, template });
    } catch (error) {
      return this.failure(error);
    }
  }

  resetTemplate(): StorageResult {
    return this.removeKey(TEMPLATE_KEY);
  }

  private createId(): string {
    if (this.newId) return this.newId();
    try {
      if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
        return crypto.randomUUID();
      }
    } catch {
      // Use the time/random fallback when crypto is unavailable.
    }
    return `${this.timestamp().toString(36)}${Math.random().toString(36).slice(2)}`;
  }

  private timestamp(): number {
    return this.now();
  }

  private toEntry(value: LibraryEntry & { v?: number }): LibraryEntry {
    return {
      id: value.id,
      name: value.name,
      text: value.text,
      createdAt: value.createdAt,
      updatedAt: value.updatedAt,
    };
  }

  private writeItem(entry: LibraryEntry): StorageResult<LibraryEntry> {
    const result = this.setValue(ITEM_PREFIX + entry.id, { v: 1, ...entry });
    return result.ok ? { ok: true, value: entry } : result;
  }

  private setValue(key: string, value: unknown): StorageResult {
    if (!this.backend || !this.available) return { ok: false, reason: 'unavailable' };
    try {
      this.backend.setItem(key, JSON.stringify(value));
      return { ok: true };
    } catch (error) {
      return this.failure(error);
    }
  }

  private removeKey(key: string): StorageResult {
    if (!this.backend || !this.available) return { ok: false, reason: 'unavailable' };
    try {
      this.backend.removeItem(key);
      return { ok: true };
    } catch (error) {
      return this.failure(error);
    }
  }

  private failure<T = void>(error: unknown): StorageResult<T> {
    if (!this.backend || !this.available) return { ok: false, reason: 'unavailable' };
    if (isQuotaError(error)) return { ok: false, reason: 'quota', message: errorMessage(error) };
    return { ok: false, reason: 'error', message: errorMessage(error) };
  }
}

export class LocalTemplateStore implements TemplateStore {
  constructor(private readonly storage: DocumentStorage) {}

  async get(): Promise<DefaultTemplate | null> {
    return this.storage.readTemplate();
  }

  async set(template: DefaultTemplate): Promise<StorageResult> {
    return this.storage.writeTemplate(template);
  }

  async reset(): Promise<StorageResult> {
    return this.storage.resetTemplate();
  }
}
