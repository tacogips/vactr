import type { LineToken } from '../types';
export function tokenizeJsonLine(text: string, state: string): { tokens: LineToken[]; state: string } {
  const original = state; const split = state.lastIndexOf('|'); let stack = state.slice(0, split); let flag = state.slice(split + 1);
  const tokens: LineToken[] = []; let i = 0;
  const emit = (from: number, tag: string) => tokens.push({ from, to: i, tag: `json-${tag}` });
  while (i < text.length) {
    if (/\s/.test(text[i])) { i++; continue; }
    const from = i; const ch = text[i];
    if (ch === '"') {
      i++; let closed = false;
      while (i < text.length) { if (text[i] === '\\') { i = Math.min(i + 2, text.length); continue; } if (text[i++] === '"') { closed = true; break; } }
      if (!closed) { emit(from, 'string'); return { tokens, state: original }; }
      const key = flag === 'k' && /^\s*:/.test(text.slice(i)); emit(from, key ? 'key' : 'string');
      if (!key) flag = 'v'; continue;
    }
    if ('{}[]:,'.includes(ch)) {
      i++; emit(from, 'punct');
      if (ch === '{' || ch === '[') { stack += ch; flag = ch === '{' ? 'k' : 'v'; }
      else if (ch === '}' || ch === ']') { stack = stack.slice(0, -1); flag = 'v'; }
      else if (ch === ',') flag = stack.endsWith('{') ? 'k' : 'v';
      else flag = 'v';
      continue;
    }
    const rest = text.slice(i); const match = /^(?:-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null)/.exec(rest);
    if (match) { i += match[0].length; emit(from, match[0] === 'null' ? 'null' : /^(true|false)$/.test(match[0]) ? 'bool' : 'number'); flag = 'v'; }
    else { i++; emit(from, 'invalid'); }
  }
  return { tokens, state: `${stack}|${flag}` };
}
