import type { Language, Parser, Query } from 'web-tree-sitter';

export interface SyntaxCapture {
  name: string;
  from: number;
  to: number;
}

export interface ParsedVact {
  captures(from: number, to: number): SyntaxCapture[];
  delete(): void;
}

export interface VactSyntax {
  parse(text: string): ParsedVact;
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
  return {
    parse(text: string): ParsedVact {
      const tree = parser.parse(text);
      if (!tree) throw new Error('Tree-sitter could not parse the document');
      return {
        captures(from, to) {
          // web-tree-sitter interprets QueryOptions startIndex/endIndex as byte offsets
          // (2 per UTF-16 code unit), so range in code units by filtering instead.
          return query
            .captures(tree.rootNode)
            .filter(
              (capture) =>
                capture.node.startIndex < to && (capture.node.endIndex > from || capture.node.startIndex >= from),
            )
            .map((capture) => ({
              name: capture.name,
              from: capture.node.startIndex,
              to: capture.node.endIndex,
            }));
        },
        delete: () => tree.delete(),
      };
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
