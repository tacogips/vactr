import { Edit, type Language, type Parser, type Query } from 'web-tree-sitter';
import type { Text } from '@codemirror/state';

export interface SyntaxCapture {
  name: string;
  from: number;
  to: number;
}

export interface ParsedVact {
  captures(from: number, to: number): SyntaxCapture[];
  changedRanges?(previous: ParsedVact): { from: number; to: number }[];
  delete(): void;
}

export interface VactSyntax {
  parse(text: string): ParsedVact;
  reparse?(parsed: ParsedVact, text: string, edits: readonly SyntaxEdit[]): ParsedVact;
  parseDoc(doc: Text, old: ParsedVact | null): ParsedVact;
  edit(parsed: ParsedVact, edit: SyntaxTreeEdit): void;
}

export interface SyntaxEdit { from: number; to: number; insert: string }

interface TreePoint { row: number; column: number }
export interface SyntaxTreeEdit { startIndex: number; oldEndIndex: number; newEndIndex: number; startPosition: TreePoint; oldEndPosition: TreePoint; newEndPosition: TreePoint }

function treeIndexLength(text: string): number { return text.length; }
function pointAt(text: string, utf16: number): TreePoint {
  const prefix = text.slice(0, utf16);
  const row = (prefix.match(/\n/g) ?? []).length;
  const columnText = prefix.slice(prefix.lastIndexOf('\n') + 1);
  return { row, column: treeIndexLength(columnText) };
}

/** Convert UTF-16 document edits to web-tree-sitter JavaScript indices and points. */
export function treeEdits(oldText: string, edits: readonly SyntaxEdit[]): SyntaxTreeEdit[] {
  return [...edits].sort((a, b) => b.from - a.from).map((edit) => {
    const startIndex = edit.from;
    const oldEndIndex = edit.to;
    const oldEndPosition = pointAt(oldText, edit.to);
    const insertedLines = edit.insert.split('\n');
    const startPosition = pointAt(oldText, edit.from);
    const newEndPosition = insertedLines.length === 1
      ? { row: startPosition.row, column: startPosition.column + treeIndexLength(edit.insert) }
      : { row: startPosition.row + insertedLines.length - 1, column: treeIndexLength(insertedLines[insertedLines.length - 1] ?? '') };
    return { startIndex, oldEndIndex, newEndIndex: startIndex + treeIndexLength(edit.insert), startPosition, oldEndPosition, newEndPosition };
  });
}

function pointAtDoc(doc: Text, pos: number): TreePoint {
  const line = doc.lineAt(pos);
  return { row: line.number - 1, column: pos - line.from };
}

/** Convert one UTF-16 transaction change using CodeMirror line coordinates. */
export function treeEditFromDocs(base: Text, next: Text, fromA: number, toA: number, fromB: number, toB: number): SyntaxTreeEdit {
  const startPosition = pointAtDoc(base, fromA);
  const inserted = next.sliceString(fromB, toB);
  const insertedLines = inserted.split('\n');
  const newEndPosition = insertedLines.length === 1
    ? { row: startPosition.row, column: startPosition.column + inserted.length }
    : { row: startPosition.row + insertedLines.length - 1, column: insertedLines[insertedLines.length - 1]!.length };
  return {
    startIndex: fromA,
    oldEndIndex: toA,
    newEndIndex: fromA + inserted.length,
    startPosition,
    oldEndPosition: pointAtDoc(base, toA),
    newEndPosition,
  };
}

export type SyntaxLoader = () => Promise<VactSyntax>;

export interface StyleSpan {
  from: number;
  to: number;
  cls: string;
}

export const CAPTURE_CLASSES: Readonly<Record<string, string>> = {
  comment: 'vact-tok-comment',
  'comment.directive': 'vact-tok-directive',
  string: 'vact-tok-string',
  number: 'vact-tok-number',
  'string.special.symbol': 'vact-tok-keyword',
  'string.special.path': 'vact-tok-path',
  keyword: 'vact-tok-head',
  'punctuation.bracket': 'vact-tok-bracket',
};

interface TreeSitterModule {
  Parser: typeof Parser;
  Language: typeof Language;
  Query: typeof Query;
}

let parserInit: Promise<void> | null = null;

export async function loadVactSyntax(base: string): Promise<VactSyntax> {
  const treeSitter = (await import('web-tree-sitter')) as TreeSitterModule;
  parserInit ??= treeSitter.Parser.init({ locateFile: (name: string) => new URL(name, base).href });
  await parserInit;

  const language = await treeSitter.Language.load(new URL('tree-sitter-vact.wasm', base));
  const response = await fetch(new URL('highlights.scm', base));
  if (!response.ok) throw new Error(`Unable to load highlights.scm: ${response.status}`);
  const query = new treeSitter.Query(language, await response.text());
  const parser = new treeSitter.Parser();
  parser.setLanguage(language);

  return createVactSyntax(parser, query);
}

export function createVactSyntax(parser: Parser, query: Query): VactSyntax {
  const trees = new WeakMap<ParsedVact, NonNullable<ReturnType<Parser['parse']>>>();
  const sources = new WeakMap<ParsedVact, string>();
  const wrap = (tree: NonNullable<ReturnType<Parser['parse']>>, source: string | Text): ParsedVact => {
      const parsed: ParsedVact = {
        changedRanges(previous) {
          const previousTree = trees.get(previous);
          return previousTree ? previousTree.getChangedRanges(tree).map((range) => ({ from: range.startIndex, to: range.endIndex })) : [];
        },
        captures(from, to) {
          const startPosition = typeof source === 'string' ? pointAt(source, from) : pointAtDoc(source, from);
          const endPosition = typeof source === 'string' ? pointAt(source, to) : pointAtDoc(source, to);
          return query.captures(tree.rootNode, { startPosition, endPosition }).map((capture) => ({ name: capture.name,
            from: capture.node.startIndex, to: capture.node.endIndex }))
            .filter((capture) => capture.from < to && (capture.to > from || capture.from >= from));
        },
        delete: () => tree.delete(),
      };
      trees.set(parsed, tree);
      if (typeof source === 'string') sources.set(parsed, source);
      return parsed;
  };
  return {
    parse(text: string): ParsedVact {
      const tree = parser.parse(text);
      if (!tree) throw new Error('Tree-sitter could not parse the document');
      return wrap(tree, text);
    },
    parseDoc(doc: Text, old: ParsedVact | null): ParsedVact {
      const oldTree = old ? trees.get(old) : undefined;
      const tree = parser.parse((index) => index < doc.length ? doc.sliceString(index, Math.min(doc.length, index + 16_384)) : undefined, oldTree);
      if (!tree) throw new Error('Tree-sitter could not parse the document');
      return wrap(tree, doc);
    },
    edit(parsed, edit) {
      const tree = trees.get(parsed);
      if (!tree) throw new Error('Unknown parsed syntax tree');
      tree.edit(new Edit(edit));
    },
    reparse(parsed, text, edits) {
      const tree = trees.get(parsed);
      if (!tree) return this.parse(text);
      const oldText = sources.get(parsed);
      if (oldText === undefined) return this.parse(text);
      for (const edit of treeEdits(oldText, edits)) tree.edit(new Edit(edit));
      const next = parser.parse(text, tree);
      if (!next) throw new Error('Tree-sitter could not incrementally parse the document');
      return wrap(next, text);
    },
  };
}

export function styleSpans(parsed: ParsedVact, from: number, to: number): StyleSpan[] {
  const spans: StyleSpan[] = [];
  const seen = new Set<string>();
  for (const capture of parsed.captures(from, to)) {
    const cls = CAPTURE_CLASSES[capture.name];
    if (!cls || capture.from >= capture.to) continue;
    const key = `${capture.from}:${capture.to}`;
    if (seen.has(key)) continue;
    seen.add(key);
    spans.push({ from: capture.from, to: capture.to, cls });
  }
  return spans.sort((a, b) => a.from - b.from || a.to - b.to);
}
