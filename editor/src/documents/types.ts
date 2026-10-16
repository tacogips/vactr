// Shared contracts for the example registry, document storage, default template and picker
// (design-example-picker-and-persistence.md).
import type { Accessor } from 'solid-js';

export type Shell = 'web' | 'app';
export type Requirement = 'audio-input' | 'midi';

export interface ExampleEntry {
  id: string;
  title: string;
  description: string;
  category: string;
  requires: readonly Requirement[];
}

export interface ExampleSource {
  readonly defaultId: string;
  list(): readonly ExampleEntry[];
  find(id: string): ExampleEntry | undefined;
  load(id: string): Promise<string>;
}

export type DocSource =
  | { kind: 'example'; id: string }
  | { kind: 'library'; id: string; name: string }
  | { kind: 'template' }
  | { kind: 'file'; name: string }
  | { kind: 'new' };

export type DefaultTemplate =
  | { kind: 'example'; id: string }
  | { kind: 'snapshot'; name: string; text: string };

export interface Draft {
  text: string;
  source: DocSource;
  savedAt: number;
}

export interface LibraryEntry {
  id: string;
  name: string;
  text: string;
  createdAt: number;
  updatedAt: number;
}

export type StorageFailure =
  | 'unavailable'
  | 'quota'
  | 'too-large'
  | 'invalid-name'
  | 'name-taken'
  | 'missing'
  | 'error';

export type StorageResult<T = void> =
  | ([T] extends [void] ? { ok: true } : { ok: true; value: T })
  | { ok: false; reason: StorageFailure; message?: string };

export interface DraftStore {
  readDraft(): Draft | null;
  writeDraft(draft: Draft): StorageResult;
  clearDraft(): void;
}

export interface DocumentStore extends DraftStore {
  readonly available: boolean;
  list(): { entries: LibraryEntry[]; unreadable: number };
  read(id: string): LibraryEntry | null;
  create(name: string, text: string): StorageResult<LibraryEntry>;
  update(id: string, text: string): StorageResult<LibraryEntry>;
  rename(id: string, name: string): StorageResult<LibraryEntry>;
  remove(id: string): StorageResult;
}

export interface TemplateStore {
  get(): Promise<DefaultTemplate | null>;
  set(template: DefaultTemplate): Promise<StorageResult>;
  reset(): Promise<StorageResult>;
}

export interface DocumentsOptions {
  shell: Shell;
  store: DocumentStore;
  templates: TemplateStore;
  examples: ExampleSource;
  search: string;
}

export const DOC_MAX_CHARS = 262144;
export const EXAMPLE_ID_PATTERN = /^[a-z0-9][a-z0-9/-]{0,95}$/;

export interface LibraryView {
  available: boolean;
  entries: readonly LibraryEntry[];
  unreadable: number;
}

export interface PickerActions {
  toggle(): void;
  close(): void;
  openExample(id: string): void;
  openLibrary(id: string): void;
  openFile(): void;
  newDocument(): void;
  save(): void;
  saveAs(name: string): string | null;
  rename(id: string, name: string): string | null;
  remove(id: string): void;
  useAsTemplate(): void;
  resetTemplate(): void;
  confirmReplace(): void;
  cancelReplace(): void;
}

export interface PickerProps {
  shell: Shell;
  open: Accessor<boolean>;
  examples: readonly ExampleEntry[];
  library: Accessor<LibraryView>;
  templateExampleId: Accessor<string | null>;
  hasTemplate: Accessor<boolean>;
  canSave: Accessor<boolean>;
  saveTarget: Accessor<string | null>;
  status: Accessor<string>;
  pendingReplace: Accessor<string | null>;
  actions: PickerActions;
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

export function isDocSource(value: unknown): value is DocSource {
  if (!isRecord(value) || typeof value.kind !== 'string') return false;

  switch (value.kind) {
    case 'example':
      return typeof value.id === 'string' && EXAMPLE_ID_PATTERN.test(value.id);
    case 'library':
      return typeof value.id === 'string' && typeof value.name === 'string';
    case 'template':
    case 'new':
      return true;
    case 'file':
      return typeof value.name === 'string';
    default:
      return false;
  }
}

export function isDefaultTemplate(value: unknown): value is DefaultTemplate {
  if (!isRecord(value) || typeof value.kind !== 'string') return false;

  if (value.kind === 'example') {
    return typeof value.id === 'string' && EXAMPLE_ID_PATTERN.test(value.id);
  }

  if (value.kind !== 'snapshot') return false;
  return (
    typeof value.name === 'string' &&
    value.name.length >= 1 &&
    value.name.length <= 120 &&
    !/[\u0000-\u001f\u007f]/.test(value.name) &&
    typeof value.text === 'string' &&
    value.text.length <= DOC_MAX_CHARS
  );
}
