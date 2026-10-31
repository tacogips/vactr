import type { Range } from '../types';
import { myers } from './myers';

export function diffWords(a: string, b: string): { a: Range[]; b: Range[] } {
  if (a.length > 2000 || b.length > 2000) return { a: a.length ? [{ from: 0, to: a.length }] : [], b: b.length ? [{ from: 0, to: b.length }] : [] };
  const tokens = (s: string) => Array.from(s.matchAll(/\w+|\s+|[^\w\s]/gu), m => ({ text: m[0], from: m.index, to: m.index + m[0].length }));
  const left = tokens(a), right = tokens(b);
  const script = myers(left.map(t => t.text), right.map(t => t.text), left.length + right.length)!;
  const result: { a: Range[]; b: Range[] } = { a: [], b: [] };
  const add = (ranges: Range[], range: Range) => {
    const last = ranges[ranges.length - 1];
    if (last && last.to === range.from) last.to = range.to;
    else ranges.push({ from: range.from, to: range.to });
  };
  let ai = 0, bi = 0;
  for (const op of script) {
    if (op === 'equal') { ai++; bi++; }
    else if (op === 'delete') add(result.a, left[ai++]!);
    else add(result.b, right[bi++]!);
  }
  return result;
}
