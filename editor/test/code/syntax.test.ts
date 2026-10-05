// @vitest-environment node

import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { Language, Parser, Query } from 'web-tree-sitter';
import { EditorState } from '@codemirror/state';
import { CAPTURE_CLASSES, createVactSyntax, FallbackSpans, SyntaxSpans } from '../../src/code/syntax';
import { styleSpans } from '../../src/code/syntax-core';
import { HEADS } from '../../src/code/language';

interface Entry {
  name: string;
  isDirectory(): boolean;
  isFile(): boolean;
}

interface NodeFs {
  readFileSync(path: string): Uint8Array<ArrayBuffer> | string;
  readdirSync(path: string, options: { withFileTypes: true }): Entry[];
}

const proc = (globalThis as unknown as { process: { cwd(): string } }).process;
let fs: NodeFs;
let parser: Parser;
let query: Query;
let language: Awaited<ReturnType<typeof Language.load>>;
let highlightSource: string;

async function readFs(): Promise<NodeFs> {
  const spec: string = 'node:fs';
  return (await import(/* @vite-ignore */ spec)) as NodeFs;
}

function readText(path: string): string {
  const value = fs.readFileSync(path);
  if (typeof value === 'string') return value;
  return new TextDecoder().decode(value);
}

function vactFiles(path: string): string[] {
  return fs.readdirSync(path, { withFileTypes: true }).flatMap((entry) => {
    const child = `${path}/${entry.name}`;
    if (entry.isDirectory()) return vactFiles(child);
    return entry.isFile() && entry.name.endsWith('.vact') ? [child] : [];
  });
}

describe('tree-sitter syntax WASM', () => {
  beforeAll(async () => {
    fs = await readFs();
    const root = proc.cwd();
    const runtimePath = `${root}/node_modules/web-tree-sitter/web-tree-sitter.wasm`;
    const grammarPath = `${root}/../tree-sitter-vact/tree-sitter-vact.wasm`;
    const runtime = fs.readFileSync(runtimePath);
    const grammar = fs.readFileSync(grammarPath);
    if (typeof runtime === 'string' || typeof grammar === 'string') throw new Error('WASM must be read as bytes');
    await Parser.init({ locateFile: () => runtimePath });
    language = await Language.load(grammar);
    parser = new Parser();
    parser.setLanguage(language);
    highlightSource = readText(`${root}/../tree-sitter-vact/queries/highlights.scm`);
    query = new Query(language, highlightSource);
  });

  afterAll(() => {
    query?.delete();
    parser?.delete();
  });

  it('parses statements, continuations, blocks, pairs and interpolation nodes', () => {
    const pairTree = parser.parse('slot :drums gain: 0.8:\n\tlet x 1');
    const continuationTree = parser.parse('play voice\n\t\t> filter:\n\t\t\tlet text "hi {name}"');
    if (!pairTree || !continuationTree) throw new Error('Tree-sitter returned no parse tree');
    const pairShape = pairTree.rootNode.toString();
    const continuationShape = continuationTree.rootNode.toString();
    expect(pairShape).toContain('(statement');
    expect(pairShape).toContain('(block');
    expect(pairShape).toContain('(pair');
    expect(continuationShape).toContain('(continuation');
    expect(continuationShape).toContain('(block');
    expect(continuationShape).toContain('(interpolation');
    pairTree.delete();
    continuationTree.delete();
  });

  it('parses every committed example and prelude source without errors', () => {
    const root = proc.cwd();
    const files = [...vactFiles(`${root}/../examples`), ...vactFiles(`${root}/../src/prelude`)];
    expect(files.length).toBeGreaterThanOrEqual(10);
    for (const file of files) {
      const tree = parser.parse(readText(file));
      if (!tree) throw new Error(`No parse tree for ${file}`);
      expect(tree.rootNode.hasError, file).toBe(false);
      tree.delete();
    }
  });

  it('produces the expected highlight captures and UTF-16 offsets', () => {
    const captureNames = (source: string) => {
      const tree = parser.parse(source);
      if (!tree) throw new Error('Tree-sitter returned no parse tree');
      const captures = query.captures(tree.rootNode).map((capture) => ({
        name: capture.name,
        from: capture.node.startIndex,
        to: capture.node.endIndex,
      }));
      tree.delete();
      return captures;
    };

    const code = captureNames('let x 1 # c');
    expect(code.map((capture) => capture.name)).toEqual(expect.arrayContaining(['keyword', 'number', 'comment']));
    expect(captureNames('#@ gain 0.5').map((capture) => capture.name)).toContain('comment.directive');
    expect(captureNames(':bd').map((capture) => capture.name)).toContain('string.special.symbol');
    expect(captureNames('./a.wav').map((capture) => capture.name)).toContain('string.special.path');
    expect(captureNames('# é\nlet x 1').find((capture) => capture.name === 'keyword')?.from).toBe(4);
  });

  it('ranges captures in UTF-16 code units across a document with non-ASCII text', () => {
    const doc = '# é\nlet x 1 :bd ./a.wav\n#@ gain 0.5\nlet y 2\n';
    const parsed = createVactSyntax(parser, query).parse(doc);
    try {
      const lastLet = doc.indexOf('let y');
      const directive = doc.indexOf('#@');
      const all = parsed.captures(0, doc.length);
      expect(all).toContainEqual({ name: 'comment.directive', from: directive, to: doc.indexOf('\n', directive) });
      expect(all).toContainEqual({ name: 'keyword', from: lastLet, to: lastLet + 3 });
      expect(all).toContainEqual({ name: 'number', from: doc.indexOf('2\n'), to: doc.indexOf('2\n') + 1 });

      const tail = parsed.captures(lastLet, doc.length);
      expect(tail.map((capture) => capture.name)).toEqual(expect.arrayContaining(['keyword', 'number']));
      expect(tail).toContainEqual({ name: 'keyword', from: lastLet, to: lastLet + 3 });
      expect(tail.every((capture) => capture.to > lastLet)).toBe(true);
      expect(tail.some((capture) => capture.name === 'comment.directive')).toBe(false);
    } finally {
      parsed.delete();
    }
  });

  it('maps every highlight capture and keeps the keyword list aligned with HEADS', () => {
    const captureNames = [...new Set([...highlightSource.matchAll(/@([\w.]+)/g)].map((match) => match[1]))];
    expect(Object.keys(CAPTURE_CLASSES).sort()).toEqual(captureNames.sort());
    const keywordLists = [...highlightSource.matchAll(/#any-of\? @keyword((?:\s+"[^"]+")*)/g)].map((match) =>
      [...match[1].matchAll(/"([^"]+)"/g)].map((word) => word[1]),
    );
    expect(keywordLists).toHaveLength(2);
    for (const keywords of keywordLists) expect(keywords).toEqual(HEADS);
  });

  it('produces head and number spans through both fallback and tree-sitter providers', () => {
    const state = EditorState.create({ doc: 'let x 1' });
    const expected = expect.arrayContaining(['vact-tok-head', 'vact-tok-number']);
    expect(new FallbackSpans().spans(state, 0, state.doc.length, 32).spans.map((span) => span.className)).toEqual(expected);
    const provider = new SyntaxSpans(createVactSyntax(parser, query));
    try { expect(provider.spans(state, 0, state.doc.length, 32).spans.map((span) => span.className)).toEqual(expected); }
    finally { provider.dispose(); }
  });

  it('keeps SyntaxSpans equivalent to a fresh parse across 200 mixed edits', () => {
    const syntax = createVactSyntax(parser, query);
    const reparse = syntax.reparse!;
    const parse = syntax.parse;
    let incrementalEdits = 0, fullParses = 0;
    const observedSyntax = { ...syntax, reparse: (...args: Parameters<NonNullable<typeof syntax.reparse>>) => {
      incrementalEdits += 1;
      return reparse(...args);
    }, parse: (text: string) => { fullParses += 1; return parse(text); } };
    const provider = new SyntaxSpans(observedSyntax);
    let state = EditorState.create({ doc: Array.from({ length: 2000 }, (_, line) => `let value${line} ${line} # 日本`).join('\n') });
    let seed = 0x12345678;
    const random = (max: number): number => { seed = (1664525 * seed + 1013904223) >>> 0; return seed % max; };
    const inserts = ['x', '日本', '\nlet fresh 7\n', '日本\n'];
    try {
      for (let i = 0; i < 200; i += 1) {
        const length = state.doc.length;
        const from = random(length + 1);
        const kind = i % 3;
        const to = kind === 0 ? from : Math.min(length, from + 1 + random(Math.max(1, Math.min(8, length - from))));
        const insert = kind === 1 ? '' : inserts[random(inserts.length)]!;
        const tr = state.update({ changes: { from, to, insert } });
        provider.noteChanges(tr.changes, tr.state);
        state = tr.state;
        if (i % 17 === 0 && i < 199) {
          const second = state.update({ changes: { from: 0, insert: '# coalesced 日本\n' } });
          provider.noteChanges(second.changes, second.state);
          state = second.state;
        }
        const actual = provider.spans(state, 0, state.doc.length, 100_000).spans;
        const fresh = syntax.parse(state.doc.toString());
        try {
          const expectedSpans = styleSpans(fresh, 0, state.doc.length).map((span) => ({ from: span.from, to: span.to, className: span.cls }));
          expect(actual.map(({ from: start, to: end, className }) => ({ from: start, to: end, className }))).toEqual(expectedSpans);
        } finally { fresh.delete(); }
      }
      expect(incrementalEdits).toBeGreaterThan(100);
      expect(fullParses).toBeGreaterThan(10);
    } finally { provider.dispose(); }
  }, 30_000);
});
