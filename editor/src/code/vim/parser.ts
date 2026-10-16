import { COUNT_LIMIT, type MotionKind, type MotionSpec, type OperatorKind, type TextObjectKind, type TextObjectSpec, type VimKey, type VimMode } from './types';

export type ActionKind = 'insert' | 'append' | 'insert-line-start' | 'append-line-end' | 'open-below' | 'open-above'
  | 'replace-mode' | 'visual' | 'visual-line' | 'visual-swap' | 'put-after' | 'put-before' | 'replace-char'
  | 'toggle-case' | 'join' | 'undo' | 'redo' | 'repeat' | 'record' | 'play' | 'set-mark' | 'prompt' | 'escape';
export type Target =
  | { kind: 'motion'; motion: MotionSpec }
  | { kind: 'textobject'; spec: TextObjectSpec }
  | { kind: 'search'; which: 'n' | 'N' | '*' | '#' }
  | { kind: 'mark'; name: string; exact: boolean }
  | { kind: 'line' };
export interface ParsedCommand { register: string | null; count: number | null; operator: OperatorKind | null; target: Target | null; action: ActionKind | null; arg: string | null }
export type ParseResult = { kind: 'pending' } | { kind: 'invalid'; message: string | null } | { kind: 'command'; command: ParsedCommand };

const pending: ParseResult = { kind: 'pending' };
const invalid = (message: string | null = null): ParseResult => ({ kind: 'invalid', message });
const command = (register: string | null, count: number | null, operator: OperatorKind | null, target: Target | null, action: ActionKind | null = null, arg: string | null = null): ParseResult => ({ kind: 'command', command: { register, count, operator, target, action, arg } });
const motions: Record<string, MotionKind> = { h: 'h', l: 'l', j: 'j', k: 'k', '0': '0', '^': '^', '$': '$', w: 'w', b: 'b', e: 'e', W: 'W', B: 'B', E: 'E', G: 'G', '{': '{', '}': '}', '%': '%', H: 'H', M: 'M', L: 'L', ';': ';', ',': ',' };
const aliases: Record<string, MotionKind> = { '<Left>': 'h', '<Right>': 'l', '<Up>': 'k', '<Down>': 'j', '<Home>': '0', '<End>': '$', '<BS>': 'BS', '<CR>': '+', '<C-d>': 'C-d', '<C-u>': 'C-u', '<C-f>': 'C-f', '<C-b>': 'C-b' };
const printable = (key: VimKey): boolean => [...key].length === 1 && !key.startsWith('<');
const registerName = (key: string): boolean => /^[a-zA-Z0-9_+*"]$/.test(key);
const alpha = (key: string): boolean => /^[a-zA-Z]$/.test(key);
const markName = (key: string): boolean => /^[a-z]$/.test(key);
const textObject: Record<string, TextObjectKind> = { w: 'w', W: 'W', '"': '"', "'": "'", '`': '`', '(': '(', ')': '(', b: '(', '[': '[', ']': '[', '{': '{', '}': '{', B: '{', '<': '<', '>': '<' };
const cap = (n: number): number => Math.min(COUNT_LIMIT, n);
const targetMotion = (kind: MotionKind, char?: string): Target => ({ kind: 'motion', motion: char === undefined ? { kind } : { kind, char } });

function parseTarget(keys: VimKey[], at: number): { target?: Target; used?: number; wait?: boolean; error?: string | null } {
  const key = keys[at];
  if (key === undefined) return { wait: true };
  if (key === 'g' || key === 'i' || key === 'a' || key === 'f' || key === 'F' || key === 't' || key === 'T' || key === "'" || key === '`') {
    const next = keys[at + 1];
    if (next === undefined) return { wait: true };
    if (key === 'g') {
      if (next === 'g') return { target: targetMotion('gg'), used: 2 };
      if (next === 'e') return { target: targetMotion('ge'), used: 2 };
      return { error: null };
    }
    if (key === 'i' || key === 'a') {
      const kind = textObject[next];
      return kind ? { target: { kind: 'textobject', spec: { around: key === 'a', kind } }, used: 2 } : { error: null };
    }
    if (key === "'" || key === '`') {
      return /^[a-z]$/.test(next) || next === "'" || next === '`'
        ? { target: { kind: 'mark', name: next, exact: key === '`' }, used: 2 }
        : { error: null };
    }
    return printable(next) ? { target: targetMotion(key as MotionKind, next), used: 2 } : { error: null };
  }
  if (key === 'n' || key === 'N' || key === '*' || key === '#') return { target: { kind: 'search', which: key }, used: 1 };
  const motion = motions[key] ?? aliases[key];
  return motion ? { target: targetMotion(motion), used: 1 } : { error: null };
}

export class CommandParser {
  private keys: VimKey[] = [];
  feed(key: VimKey, mode: VimMode): ParseResult {
    if (key === '<Esc>') {
      const last = this.keys[this.keys.length - 1];
      const cancelArgument = ['f', 'F', 't', 'T', 'r'].includes(last);
      this.reset();
      return cancelArgument ? invalid() : command(null, null, null, null, 'escape');
    }
    this.keys.push(key);
    const result = this.parse(mode);
    if (result.kind !== 'pending') this.reset();
    return result;
  }
  reset(): void { this.keys = []; }
  pending(): string { return this.keys.join(''); }
  operatorPending(): boolean {
    const k = this.keys;
    let i = 0;
    while (i < k.length && /^[0-9]$/.test(k[i])) i++;
    if (k[i] === '"') i += 2;
    if (i === 0 && k[0] === '"') i = 2;
    while (i < k.length && /^[0-9]$/.test(k[i])) i++;
    const first = k[i];
    if (['d', 'c', 'y', '>', '<'].includes(first)) return true;
    return first === 'g' && ['~', 'u', 'U'].includes(k[i + 1]);
  }
  private parse(mode: VimMode): ParseResult {
    const k = this.keys;
    let i = 0;
    let register: string | null = null;
    let prefixCount: number | null = null;
    const readCount = (): number | null => {
      if (k[i] === undefined || !/^[1-9]$/.test(k[i])) return null;
      let value = 0;
      while (i < k.length && /^[0-9]$/.test(k[i])) { value = Math.min(COUNT_LIMIT, value * 10 + Number(k[i])); i++; }
      return cap(value);
    };
    prefixCount = readCount();
    if (k[i] === '"') {
      if (k[i + 1] === undefined) return pending;
      if (!registerName(k[i + 1])) return invalid('E354 Invalid register name');
      register = k[i + 1]; i += 2;
    }
    const registerCount = readCount();
    if (registerCount !== null) prefixCount = prefixCount === null ? registerCount : cap(prefixCount * registerCount);
    if (i === k.length) return pending;
    const first = k[i];
    const visual = mode === 'visual' || mode === 'visual-line';
    let op: OperatorKind | null = null;
    let opCount: number | null = null;
    let opLength = 1;
    if (first === 'g') {
      if (k[i + 1] === undefined) return pending;
      const second = k[i + 1];
      if (second === '~' || second === 'u' || second === 'U') { op = `g${second}` as OperatorKind; opLength = 2; }
      else if (second === 'g' || second === 'e') {
        if (visual) return command(register, prefixCount, null, targetMotion(second === 'g' ? 'gg' : 'ge'));
        if (second === 'g') return command(register, prefixCount, null, targetMotion('gg'));
        return command(register, prefixCount, null, targetMotion('ge'));
      } else return invalid();
    } else if (['d', 'c', 'y', '>', '<'].includes(first)) op = first as OperatorKind;

    if (visual) return this.parseVisual(k.slice(i), register, prefixCount, first);
    if (op) {
      i += opLength;
      opCount = readCount();
      if (opLength === 1 && k[i] === first || opLength === 2 && (k[i] === op[1] || k[i] === 'g' && k[i + 1] === op[1])) return command(register, prefixCount === null && opCount === null ? null : cap((prefixCount ?? 1) * (opCount ?? 1)), op, { kind: 'line' });
      if (i === k.length) return pending;
      if (k[i] === '/' || k[i] === '?') return invalid('search prompt cannot follow an operator');
      const parsed = parseTarget(k, i);
      if (parsed.wait) return pending;
      if (parsed.error !== undefined) return invalid(parsed.error);
      const count = prefixCount === null && opCount === null ? null : cap((prefixCount ?? 1) * (opCount ?? 1));
      return command(register, count, op, parsed.target ?? null);
    }
    if (first === '0' && prefixCount === null) return command(register, null, null, targetMotion('0'));
    if (prefixCount !== null || /^[1-9]$/.test(first)) {
      // The count was consumed above; continue with the following command key.
      if (i >= k.length) return pending;
    }
    if (first === 'g') return pending;
    if (first === 'f' || first === 'F' || first === 't' || first === 'T' || first === "'" || first === '`') {
      const parsed = parseTarget(k, i);
      if (parsed.wait) return pending;
      return parsed.error !== undefined ? invalid(parsed.error) : command(register, prefixCount, null, parsed.target ?? null);
    }
    if (first === 'i' || first === 'a') return command(register, prefixCount, null, null, first === 'i' ? 'insert' : 'append');
    if (first === 'x' || first === '<Del>') return command(register, prefixCount, 'd', targetMotion('l'));
    if (first === 'X') return command(register, prefixCount, 'd', targetMotion('h'));
    if (first === 's') return command(register, prefixCount, 'c', targetMotion('l'));
    if (first === 'S') return command(register, prefixCount, 'c', { kind: 'line' });
    if (first === 'D') return command(register, prefixCount, 'd', targetMotion('$'));
    if (first === 'C') return command(register, prefixCount, 'c', targetMotion('$'));
    if (first === 'Y') return command(register, prefixCount, 'y', { kind: 'line' });
    const direct: Record<string, ActionKind> = { I: 'insert-line-start', A: 'append-line-end', o: 'open-below', O: 'open-above', R: 'replace-mode', v: 'visual', V: 'visual-line', p: 'put-after', P: 'put-before', '~': 'toggle-case', J: 'join', u: 'undo', '<C-r>': 'redo', '.': 'repeat', ':': 'prompt', '/': 'prompt', '?': 'prompt' };
    if (direct[first]) return command(register, prefixCount, null, null, direct[first], first === ':' || first === '/' || first === '?' ? first : null);
    if (first === 'r' || first === 'q' || first === '@' || first === 'm') {
      if (k[i + 1] === undefined) return pending;
      const arg = k[i + 1];
      if (first === 'r') return printable(arg) || arg === '<CR>' ? command(register, prefixCount, null, null, 'replace-char', arg === '<CR>' ? '\n' : arg) : invalid();
      if (first === 'q' && alpha(arg)) return command(register, prefixCount, null, null, 'record', arg);
      if (first === '@' && (markName(arg) || arg === '@')) return command(register, prefixCount, null, null, 'play', arg);
      if (first === 'm' && markName(arg)) return command(register, prefixCount, null, null, 'set-mark', arg);
      return invalid();
    }
    const parsed = parseTarget(k, i);
    if (parsed.wait) return pending;
    if (parsed.error !== undefined) return invalid(parsed.error);
    return parsed.target ? command(register, prefixCount, null, parsed.target) : invalid();
  }
  private parseVisual(keys: VimKey[], register: string | null, count: number | null, first: string): ParseResult {
    if (keys.length === 0) return pending;
    if (first === 'i' || first === 'a') {
      if (keys[1] === undefined) return pending;
      const kind = textObject[keys[1]];
      return kind ? command(register, count, null, { kind: 'textobject', spec: { around: first === 'a', kind } }) : invalid();
    }
    if (['d', 'x', '<Del>', 'c', 's', 'y', '>', '<', '~'].includes(first)) {
      const op: OperatorKind = first === 'x' || first === '<Del>' ? 'd' : first === '~' ? 'g~' : first as OperatorKind;
      return command(register, count, op, null);
    }
    const lineOps: Record<string, OperatorKind> = { D: 'd', X: 'd', C: 'c', S: 'c', R: 'c', Y: 'y' };
    if (lineOps[first]) return command(register, count, lineOps[first], { kind: 'line' });
    if (first === 'g') {
      if (keys[1] === undefined) return pending;
      if (keys[1] === '~' || keys[1] === 'u' || keys[1] === 'U') return command(register, count, `g${keys[1]}` as OperatorKind, null);
    }
    if (first === 'r') {
      if (keys[1] === undefined) return pending;
      return printable(keys[1]) || keys[1] === '<CR>' ? command(register, count, null, null, 'replace-char', keys[1] === '<CR>' ? '\n' : keys[1]) : invalid();
    }
    const visualActions: Record<string, ActionKind> = { J: 'join', p: 'put-after', P: 'put-before', o: 'visual-swap', v: 'visual', V: 'visual-line', ':': 'prompt' };
    if (visualActions[first]) return command(register, count, null, null, visualActions[first], first === ':' ? ':' : null);
    const parsed = parseTarget(keys, 0);
    if (parsed.wait) return pending;
    return parsed.error !== undefined ? invalid(parsed.error) : command(register, count, null, parsed.target ?? null);
  }
}
