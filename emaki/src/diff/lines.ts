import { myers } from './myers';
import type { Hunk } from './index';

export function diffLines(a: readonly string[], b: readonly string[], opts: { maxCost?: number } = {}): Hunk[] {
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let aEnd = a.length, bEnd = b.length;
  while (aEnd > start && bEnd > start && a[aEnd - 1] === b[bEnd - 1]) { aEnd--; bEnd--; }
  if (start === aEnd && start === bEnd) return [];
  const ids = new Map<string, number>();
  const intern = (s: string): number => {
    let id = ids.get(s);
    if (id === undefined) { id = ids.size; ids.set(s, id); }
    return id;
  };
  const script = myers(a.slice(start, aEnd).map(intern), b.slice(start, bEnd).map(intern), opts.maxCost ?? 2000);
  if (!script) return [{ aFrom: start, aTo: aEnd, bFrom: start, bTo: bEnd }];
  const hunks: Hunk[] = [];
  let ai = start, bi = start, current: Hunk | undefined;
  for (const op of script) {
    if (op === 'equal') { if (current) { hunks.push(current); current = undefined; } ai++; bi++; }
    else {
      current ??= { aFrom: ai, aTo: ai, bFrom: bi, bTo: bi };
      if (op === 'delete') current.aTo = ++ai;
      else current.bTo = ++bi;
    }
  }
  if (current) hunks.push(current);
  return hunks;
}
