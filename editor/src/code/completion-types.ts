export type CompletionContextKind = 'none' | 'keyword' | 'qualified' | 'pipe' | 'head' | 'pair-key' | 'argument';

export type CandidateKind =
  | 'local'
  | 'function'
  | 'value'
  | 'variable'
  | 'type'
  | 'keyword'
  | 'control'
  | 'key'
  | 'module'
  | 'qualified';

export interface CompletionItem {
  label: string;
  kind: CandidateKind;
  detail: string;
  insert: string;
}

export interface RawCompletion {
  v: 1;
  context: CompletionContextKind;
  from: number;
  to: number;
  incomplete: boolean;
  items: CompletionItem[];
}

export interface CompletionResult {
  context: CompletionContextKind;
  from: number;
  to: number;
  incomplete: boolean;
  items: CompletionItem[];
}

export interface CompletionEngine {
  complete(text: string, byteCursor: number, limit: number): Promise<RawCompletion | null>;
}

export interface CompletionSource {
  readonly available: boolean;
  complete(text: string, cursor16: number): Promise<CompletionResult | null>;
}

export type CompletionKey =
  | 'ArrowUp'
  | 'ArrowDown'
  | 'PageUp'
  | 'PageDown'
  | 'Enter'
  | 'Tab'
  | 'Escape'
  | 'Ctrl-Space';

export interface SurfaceChange {
  docChanged: boolean;
  selectionChanged: boolean;
  userEvent: string | null;
  inserted: string;
}

export interface CaretRect {
  left: number;
  top: number;
  bottom: number;
}

export interface CompletionSurface {
  text(): string;
  version(): unknown;
  selection(): { anchor: number; head: number };
  replace(from: number, to: number, insert: string): void;
  caretRect(pos: number): CaretRect | null;
  isComposing(): boolean;
  onChange(listener: (change: SurfaceChange) => void): () => void;
  onKey(handler: (key: CompletionKey) => boolean): () => void;
  onBlur(listener: () => void): () => void;
  onCompositionStart(listener: () => void): () => void;
  popupHost(): HTMLElement;
}

export const COMPLETION_KEYS: readonly CompletionKey[] = [
  'ArrowUp',
  'ArrowDown',
  'PageUp',
  'PageDown',
  'Enter',
  'Tab',
  'Escape',
  'Ctrl-Space',
];

export const COMPLETION_USER_EVENT = 'input.complete';
export const COMPLETION_DEBOUNCE_MS = 150;

export function isTriggerChar(ch: string): boolean {
  return ch.length === 1 && /[A-Za-z0-9\-:.]/.test(ch);
}
