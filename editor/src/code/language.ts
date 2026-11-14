// The `.vact` mode (design 15.1.5): a CodeMirror `StreamLanguage`
// tokenizer for HIGHLIGHTING ONLY. It never decides bindings, sites or
// forms; those come from the session. There is no Lezer grammar.
//
// Tokens: `#@` directive lines (distinct from `#` comments), `:keyword`s,
// numbers (int, float, ratio), strings, path/url literals (6.5.8) and the
// definition heads.

import { HighlightStyle, StreamLanguage, StringStream, syntaxHighlighting, type StreamParser } from '@codemirror/language';
import type { Extension } from '@codemirror/state';
import { Tag, tags } from '@lezer/highlight';

/** The token class names the tokenizer emits. */
export type VactToken =
  | 'directive'
  | 'comment'
  | 'keyword'
  | 'number'
  | 'string'
  | 'path'
  | 'head'
  | 'bracket'
  | 'name';

export const HEADS: readonly string[] = [
  'let',
  'var',
  'upd',
  'fn',
  'inst',
  'bus',
  'look',
  'master',
  'import',
  'slot',
  'if',
];

const HEAD_SET = new Set(HEADS);

/** A `#@` directive comment (a sub-tag of `meta`). */
export const directiveTag = Tag.define(tags.meta);

const TOKEN_TABLE: Record<VactToken, Tag> = {
  directive: directiveTag,
  comment: tags.lineComment,
  keyword: tags.atom,
  number: tags.number,
  string: tags.string,
  path: tags.url,
  head: tags.definitionKeyword,
  bracket: tags.bracket,
  name: tags.variableName,
};

interface State {
  /** Inside a string that continues on the next line. */
  inString: boolean;
}

const PATH_RE = /^(?:\.\.\/|\.\/|~\/|\/)[A-Za-z0-9._~-]+(?:\/[A-Za-z0-9._~-]+)*/;
const URL_RE = /^[a-z][a-z0-9+.-]*:\/\/[^\s"#{}[\]\\<>|^`]+/;
const NUMBER_RE = /^-?\d+(?:\.\d+)?(?:\/\d+)?(?:e[+-]?\d+)?/i;
const NAME_RE = /^[A-Za-z_][A-Za-z0-9_.-]*/;

/** A path or url may start only at a line start or after whitespace, `{` or `[` (6.5.8). */
function atTokenBoundary(stream: StringStream): boolean {
  if (stream.pos === 0) return true;
  const prev = stream.string.charAt(stream.pos - 1);
  return /\s/.test(prev) || prev === '{' || prev === '[';
}

function readString(stream: StringStream, state: State): VactToken {
  while (!stream.eol()) {
    const c = stream.next();
    if (c === '\\') stream.next();
    else if (c === '"') {
      state.inString = false;
      return 'string';
    }
  }
  state.inString = true;
  return 'string';
}

export const vactParser: StreamParser<State> = {
  name: 'vactr',
  startState: () => ({ inString: false }),
  token(stream, state): VactToken | null {
    if (state.inString) return readString(stream, state);
    if (stream.eatSpace()) return null;
    const ch = stream.peek();
    if (ch === '#') {
      const directive = stream.string.startsWith('#@', stream.pos);
      stream.skipToEnd();
      return directive ? 'directive' : 'comment';
    }
    if (ch === '"') {
      stream.next();
      return readString(stream, state);
    }
    if (atTokenBoundary(stream)) {
      if (stream.match(URL_RE)) return 'path';
      // A path ends at the first non-path character (6.5.8).
      if (stream.match(PATH_RE)) return 'path';
    }
    if (ch === ':' && /[A-Za-z]/.test(stream.string.charAt(stream.pos + 1))) {
      stream.next();
      stream.match(NAME_RE);
      return 'keyword';
    }
    if (/\d/.test(ch ?? '') || (ch === '-' && /\d/.test(stream.string.charAt(stream.pos + 1)) && atTokenBoundary(stream))) {
      if (stream.match(NUMBER_RE)) return 'number';
    }
    if (ch !== undefined && '{}[]'.includes(ch)) {
      stream.next();
      return 'bracket';
    }
    const m = stream.match(NAME_RE);
    if (m && typeof m !== 'boolean') {
      const word = m[0];
      // A head only as the first word of its line (after indentation).
      const lead = stream.string.slice(0, stream.start).trim() === '';
      return lead && HEAD_SET.has(word) ? 'head' : 'name';
    }
    stream.next();
    return null;
  },
  blankLine(state) {
    // A string never spans a blank line in highlighting.
    state.inString = false;
  },
  tokenTable: TOKEN_TABLE,
  languageData: { commentTokens: { line: '#' } },
};

export const vactHighlightStyle = HighlightStyle.define([
  { tag: directiveTag, class: 'vact-tok-directive' },
  { tag: tags.lineComment, class: 'vact-tok-comment' },
  { tag: tags.atom, class: 'vact-tok-keyword' },
  { tag: tags.number, class: 'vact-tok-number' },
  { tag: tags.string, class: 'vact-tok-string' },
  { tag: tags.url, class: 'vact-tok-path' },
  { tag: tags.definitionKeyword, class: 'vact-tok-head' },
  { tag: tags.bracket, class: 'vact-tok-bracket' },
]);

/** The `.vact` language plus its highlight style. */
export function vactLanguage(): Extension {
  return [StreamLanguage.define(vactParser), syntaxHighlighting(vactHighlightStyle)];
}

/** One highlighted token of a line (tests and diagnostics of the mode). */
export interface LineToken {
  text: string;
  type: VactToken | null;
}

/** Tokenizes `lines` with the stream parser, carrying state across lines. */
export function tokenize(lines: readonly string[]): LineToken[][] {
  const state = vactParser.startState?.(2) ?? { inString: false };
  const out: LineToken[][] = [];
  for (const line of lines) {
    const toks: LineToken[] = [];
    if (line.length === 0) vactParser.blankLine?.(state, 2);
    const stream = new StringStream(line, 4, 2);
    while (!stream.eol()) {
      stream.start = stream.pos;
      const type = vactParser.token(stream, state) as VactToken | null;
      if (stream.pos === stream.start) stream.next();
      toks.push({ text: line.slice(stream.start, stream.pos), type });
    }
    out.push(toks.filter((t) => t.text.trim() !== '' || t.type !== null));
  }
  return out;
}

/** GPU presentation spans in document UTF-16 coordinates; never semantic site identities. */
export interface TokenSpan { from: number; to: number; type: VactToken; className: string }
export function tokenizerSpans(doc: import('@codemirror/state').Text): TokenSpan[] {
  const state = vactParser.startState?.(2) ?? { inString: false };
  const spans: TokenSpan[] = [];
  for (let i = 1; i <= doc.lines; i += 1) {
    const line = doc.line(i);
    if (!line.length) vactParser.blankLine?.(state, 2);
    const stream = new StringStream(line.text, 4, 2);
    while (!stream.eol()) {
      stream.start = stream.pos;
      const type = vactParser.token(stream, state) as VactToken | null;
      if (stream.pos === stream.start) stream.next();
      if (type) spans.push({ from: line.from + stream.start, to: line.from + stream.pos, type, className: `vact-tok-${type}` });
    }
  }
  return spans;
}
