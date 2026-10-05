// @vitest-environment node

import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

vi.mock('@codemirror/view', () => {
  throw new Error('syntax-core must not load @codemirror/view');
});

import { Text } from '@codemirror/state';
import { Language, Parser, Query } from 'web-tree-sitter';
import { createVactSyntax, styleSpans, treeEditFromDocs, treeEdits, type VactSyntax } from '../../src/code/syntax-core';

interface NodeFs {
  readFileSync(path: string): Uint8Array<ArrayBuffer> | string;
}

const proc = (globalThis as unknown as { process: { cwd(): string } }).process;
let fs: NodeFs;
let parser: Parser;
let query: Query;
let runtimePath: string;

async function readFs(): Promise<NodeFs> {
  const spec: string = 'node:fs';
  return (await import(/* @vite-ignore */ spec)) as NodeFs;
}

function readText(path: string): string {
  const value = fs.readFileSync(path);
  if (typeof value === 'string') return value;
  return new TextDecoder().decode(value);
}

describe('EditorView-free tree-sitter syntax core', () => {
  beforeAll(async () => {
    fs = await readFs();
    const root = proc.cwd();
    runtimePath = `${root}/node_modules/web-tree-sitter/web-tree-sitter.wasm`;
    const grammarPath = `${root}/../tree-sitter-vact/tree-sitter-vact.wasm`;
    const runtime = fs.readFileSync(runtimePath);
    const grammar = fs.readFileSync(grammarPath);
    if (typeof runtime === 'string' || typeof grammar === 'string') throw new Error('WASM must be read as bytes');
    await Parser.init({ locateFile: () => runtimePath });
    const language = await Language.load(grammar);
    parser = new Parser();
    parser.setLanguage(language);
    query = new Query(language, readText(`${root}/../tree-sitter-vact/queries/highlights.scm`));
  });

  afterAll(() => {
    query?.delete();
    parser?.delete();
  });

  it('returns sorted UI-agnostic style spans without loading CodeMirror', () => {
    const text = 'let a 1\n# \u65e5\u672c\ns :bd > d1 "x" [1]\n';
    const parsed = createVactSyntax(parser, query).parse(text);
    try {
      const spans = styleSpans(parsed, 0, text.length);
      const findSpan = (cls: string, from: number) => spans.find((span) => span.cls === cls && span.from === from);
      const line3Start = text.indexOf('s :bd');
      const commentStart = text.indexOf('# ');
      const commentEnd = text.indexOf('\n', commentStart);

      expect(findSpan('vact-tok-head', text.indexOf('let'))).toMatchObject({
        from: text.indexOf('let'),
        to: text.indexOf('let') + 3,
      });
      expect(findSpan('vact-tok-comment', commentStart)).toMatchObject({ from: commentStart, to: commentEnd });
      expect(findSpan('vact-tok-keyword', text.indexOf(':bd'))).toBeDefined();
      expect(findSpan('vact-tok-string', text.indexOf('"x"'))).toBeDefined();
      expect(findSpan('vact-tok-bracket', text.indexOf('['))).toBeDefined();
      expect(findSpan('vact-tok-bracket', text.indexOf(']'))).toBeDefined();
      expect(findSpan('vact-tok-number', text.lastIndexOf('1'))).toBeDefined();
      expect(spans).toEqual([...spans].sort((a, b) => a.from - b.from || a.to - b.to));
      expect(new Set(spans.map(({ from, to }) => `${from}:${to}`)).size).toBe(spans.length);
      expect(styleSpans(parsed, line3Start, text.length).every((span) => span.to >= line3Start)).toBe(true);
      expect(readText(`${proc.cwd()}/src/code/syntax-core.ts`)).not.toContain('@codemirror/view');
    } finally {
      parsed.delete();
    }
  });

  it('keeps incremental tree edits equivalent to a fresh parse over 200 edits', () => {
    const syntax: VactSyntax = createVactSyntax(parser, query);
    let text = Array.from({ length: 2000 }, (_, line) => `let value${line} ${line} # 日本`).join('\n');
    let parsed = syntax.parse(text);
    let seed = 0x12345678;
    const random = (max: number): number => { seed = (1664525 * seed + 1013904223) >>> 0; return seed % max; };
    try {
      for (let i = 0; i < 200; i += 1) {
        const lines = text.split('\n');
        const line = random(lines.length);
        const from = lines.slice(0, line).join('\n').length + (line ? 1 : 0);
        const insert = `# edit ${i} 日本\n`;
        const next = `${text.slice(0, from)}${insert}${text.slice(from)}`;
        const incremental = syntax.reparse?.(parsed, next, [{ from, to: from, insert }]);
        if (!incremental) throw new Error('syntax provider does not support incremental reparse');
        parsed.delete(); parsed = incremental; text = next;
        const fresh = syntax.parse(text);
        try { expect(styleSpans(parsed, 0, text.length)).toEqual(styleSpans(fresh, 0, text.length)); }
        finally { fresh.delete(); }
      }
    } finally { parsed.delete(); }
  }, 240_000);

  it('derives Text tree edits equal to the existing string edit algorithm', () => {
    const cases = [
      { old: 'abc def', from: 2, to: 4, insert: 'XY' },
      { old: 'let 日本 1', from: 5, to: 7, insert: '🎹' },
      { old: 'one\ntwo\nthree', from: 4, to: 8, insert: '二\n三\n' },
      { old: '🎹\n日本語', from: 2, to: 5, insert: '' },
    ];
    for (const item of cases) {
      const base = Text.of(item.old.split('\n'));
      const next = base.replace(item.from, item.to, Text.of(item.insert.split('\n')));
      const actual = treeEditFromDocs(base, next, item.from, item.to, item.from, item.from + item.insert.length);
      expect(actual).toEqual(treeEdits(item.old, [{ from: item.from, to: item.to, insert: item.insert }])[0]);
    }
  });

  it('parses Text in bounded chunks and returns the same captures as the string path', () => {
    const syntax = createVactSyntax(parser, query);
    const source = `${Array.from({ length: 20_000 }, (_, index) => `let value${index} ${index} # 日本`).join('\n')}\n`;
    const doc = Text.of(source.split('\n'));
    const proto = Object.getPrototypeOf(doc) as { sliceString: (from: number, to?: number) => string };
    const slice = vi.spyOn(proto, 'sliceString');
    const parsed = syntax.parseDoc(doc, null);
    const stringParsed = syntax.parse(source);
    try {
      expect(slice.mock.calls.every(([from, to]) => (to ?? doc.length) - from <= 16_384)).toBe(true);
      expect(styleSpans(parsed, 0, doc.length)).toEqual(styleSpans(stringParsed, 0, source.length));
    } finally {
      parsed.delete();
      stringParsed.delete();
      slice.mockRestore();
    }
  }, 30_000);
});
