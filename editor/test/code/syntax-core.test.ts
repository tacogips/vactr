// @vitest-environment node

import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

vi.mock('@codemirror/view', () => {
  throw new Error('syntax-core must not load @codemirror/view');
});
vi.mock('@codemirror/state', () => {
  throw new Error('syntax-core must not load @codemirror/state');
});

import { Language, Parser, Query } from 'web-tree-sitter';
import { createVactSyntax, styleSpans } from '../../src/code/syntax-core';

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

describe('CodeMirror-free tree-sitter syntax core', () => {
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
      expect(readText(`${proc.cwd()}/src/code/syntax-core.ts`)).not.toContain('@codemirror/');
    } finally {
      parsed.delete();
    }
  });
});
