import { ChangeSet, Text } from '@codemirror/state';

export const HISTORY_LIMIT = 256;
export const HISTORY_UNDO_BYTES = 32 * 1024 * 1024;
export const INDEX_BYTES = 8 * 1024 * 1024;

/** Conservative retained-heap charge; count copies rather than assuming rope sharing. */
export function retainedBytes(value: unknown, seen = new Set<object>()): number {
  if (typeof value === 'string') return 32 + value.length * 2;
  if (value === null || value === undefined) return 0;
  if (typeof value !== 'object') return 16;
  if (seen.has(value)) return 0;
  seen.add(value);
  if (value instanceof Text) return 128 + value.length * 2 + value.lines * 64;
  if (ArrayBuffer.isView(value)) return 128 + value.byteLength;
  if (value instanceof ArrayBuffer) return 128 + value.byteLength;
  if (value instanceof Map) return 128 + [...value].reduce((n, [k, v]) => n + retainedBytes(k, seen) + retainedBytes(v, seen) + 64, 0);
  if (value instanceof Set) return 128 + [...value].reduce((n, v) => n + retainedBytes(v, seen) + 32, 0);
  return 128 + Object.values(value).reduce<number>((n, v) => n + 16 + retainedBytes(v, seen), 0);
}

/** Adapter for pinned @codemirror/commands 6.11.1; no serialized effects are lost. */
export function trimUndoHistory(value: unknown, budget: number, memo?: WeakMap<object, number>): { value: unknown; bytes: number; reduced: boolean } {
  const h = value as { done: readonly unknown[]; undone: readonly unknown[] };
  if (!h || !Array.isArray(h.done) || !Array.isArray(h.undone)) throw new Error('Unsupported CodeMirror history layout');
  const done = [...h.done], undone = [...h.undone];
  const charge = (event: unknown): number => {
    if (event === null || typeof event !== 'object') return retainedBytes(event);
    const known = memo?.get(event);
    if (known !== undefined) return known;
    const bytes = retainedBytes(event);
    memo?.set(event, bytes);
    return bytes;
  };
  const cost = (events: readonly unknown[]): number => events.reduce<number>((n, event) => n + charge(event), 0);
  let bytes = cost(done) + cost(undone);
  let reduced = false;
  while (bytes > budget && (done.length || undone.length)) {
    const branch = done.length ? done : undone;
    bytes -= charge(branch.shift());
    reduced = true;
  }
  if (!reduced) return { value, bytes, reduced };
  const replacement = Object.create(Object.getPrototypeOf(value));
  Object.defineProperties(replacement, Object.getOwnPropertyDescriptors(value));
  replacement.done = done;
  replacement.undone = undone;
  return { value: replacement, bytes, reduced };
}

/** True when an individual edit touches `[from, to)`. */
export function touches(changes: ChangeSet, from: number, to: number): boolean {
  let hit = false;
  changes.iterChangedRanges((fromA, toA) => {
    if (hit) return;
    if (from === to) hit = fromA < from && from < toA;
    else if (fromA === toA) hit = from < fromA && fromA < to;
    else hit = fromA < to && toA > from;
  });
  return hit;
}

