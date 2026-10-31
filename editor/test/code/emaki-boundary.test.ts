// @vitest-environment node

import { describe, expect, it } from 'vitest';

interface Entry { name: string; isDirectory(): boolean; isFile(): boolean }
interface Fs {
  readdirSync(path: string, options: { withFileTypes: true }): Entry[];
  readFileSync(path: string, encoding: 'utf8'): string;
  existsSync(path: string): boolean;
}
const processApi = (globalThis as unknown as { process: { cwd(): string } }).process;
const publicEntries = new Set(['emaki', 'emaki/engine', 'emaki/diff', 'emaki/json']);

function invalidSpecifier(specifier: string): boolean {
  return (specifier === 'emaki' || specifier.startsWith('emaki/'))
    ? !publicEntries.has(specifier)
    : specifier.startsWith('.') && /(?:^|\/)emaki\/|node_modules\/emaki(?:\/|$)/.test(specifier);
}

function specifiers(source: string): string[] {
  return [...source.matchAll(/(?:\bfrom\s*|\bimport\s*\(\s*|\bimport\s*)['"]([^'"]+)['"]/g)]
    .map((match) => match[1]);
}

describe('emaki consumer boundary', () => {
  it('classifies public and private import specifiers', () => {
    for (const name of publicEntries) expect(invalidSpecifier(name)).toBe(false);
    for (const name of ['emaki/src/render/atlas', '../emaki/src/index', '../../node_modules/emaki/src', 'emaki/private']) {
      expect(invalidSpecifier(name)).toBe(true);
    }
    expect(invalidSpecifier('./renderer')).toBe(false);
    expect(specifiers("import 'emaki'; import { x } from 'emaki/engine'; import('emaki/json')"))
      .toEqual(['emaki', 'emaki/engine', 'emaki/json']);
  });

  it('keeps engine implementation and private imports out of the editor', async () => {
    const moduleName: string = 'node:fs';
    const fs = await import(/* @vite-ignore */ moduleName) as Fs;
    const root = processApi.cwd();
    const violations: string[] = [];
    const visit = (dir: string): void => {
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const path = `${dir}/${entry.name}`;
        if (entry.isDirectory()) visit(path);
        else if (entry.isFile() && /\.[cm]?[jt]sx?$/.test(entry.name)) {
          const source = fs.readFileSync(path, 'utf8');
          for (const specifier of specifiers(source)) {
            if (invalidSpecifier(specifier)) violations.push(`${path}: ${specifier}`);
          }
          if (path.startsWith(`${root}/src/code/`) && /getContext\(\s*['"]webgl2['"]|#version 300 es|class GlyphAtlas/.test(source)) {
            violations.push(`${path}: engine implementation`);
          }
        }
      }
    };
    visit(`${root}/src`);
    const removed = ['atlas', 'frame', 'geometry', 'segments', 'palette', 'cursor-shape', 'resources',
      'renderer-types', 'layout', 'advances', 'keyboard', 'pointer', 'accessibility', 'view-host'];
    const movedTests = ['code/atlas-cells', 'code/glyph-placement', 'code/text-cells', 'canvas/frame', 'canvas/cursor-shape'];
    for (const name of removed) if (fs.existsSync(`${root}/src/code/${name}.ts`)) violations.push(`source remains: ${name}`);
    for (const name of movedTests) if (fs.existsSync(`${root}/test/${name}.test.ts`)) violations.push(`test remains: ${name}`);
    expect(violations, violations.join('\n')).toEqual([]);
  });
});
