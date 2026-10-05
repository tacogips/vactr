// @vitest-environment node

import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

vi.mock('@codemirror/view', () => {
  throw new Error('syntax-core must not load @codemirror/view');
});
vi.mock('@codemirror/state', () => {
  throw new Error('syntax-core must not load @codemirror/state');
});

import { Language, Parser, Query } from 'web-tree-sitter';
import { createVactSyntax, styleSpans, type VactSyntax } from '../../src/code/syntax-core';

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
  }, 30_000);
});
