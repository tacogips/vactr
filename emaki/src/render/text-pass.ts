import type { ShapedLine } from '../layout/layout';
import type { TokenSpan } from '../types';

export function tokenKey(tokens: readonly TokenSpan[]): string {
  return tokens.map(token => `${token.from}:${token.to}:${token.tag}`).join(';');
}
export function tokenStyleHash(line: ShapedLine, tokens: readonly TokenSpan[], from: number, to: number): string {
  return tokens.filter(token => token.from < to && token.to > from)
    .map(token => `${Math.max(0, token.from - line.from)}:${Math.min(line.to, token.to) - line.from}:${token.tag}`).join(';');
}
