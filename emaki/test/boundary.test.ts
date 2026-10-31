// @vitest-environment node
import { describe, expect, it } from 'vitest';

interface DirEntry { name: string; isDirectory(): boolean; isFile(): boolean }
interface NodeFs { readdirSync(path: string, options: { withFileTypes: true }): DirEntry[]; readFileSync(path: string, encoding: 'utf8'): string }
interface NodeProcess { cwd(): string }
const processApi = (globalThis as unknown as { process: NodeProcess }).process;
const forbidden = ['solid' + '-js', 'vac' + 'tr', '/' + 'editor/'];
const commonPackages = ['@codemirror/state', '@codemirror/commands'];
const testPackages = [...commonPackages, 'vitest', 'jsdom', 'playwright'];

function classify(file: string, source: string): string[] {
  const issues: string[] = [];
  for (const text of forbidden) if (source.includes(text)) issues.push(`forbidden text: ${text}`);
  const imports = /\b(?:import\s*(?:[^'";]*?\sfrom\s*)?|export\s+[^'";]*?\sfrom\s*|import\s*\(\s*)['"]([^'"]+)['"]/g;
  for (const match of source.matchAll(imports)) {
    const spec = match[1];
    if (spec.startsWith('.')) {
      const parts = file.split('/').slice(0, -1);
      for (const component of spec.split('/')) {
        if (component === '..') { if (parts.length === 0) { issues.push(`escaping import: ${spec}`); break; } parts.pop(); }
        else if (component !== '.' && component !== '') parts.push(component);
      }
    } else {
      const allowed = file.startsWith('src/') ? commonPackages : file.endsWith('.config.ts') ? [...testPackages, 'vite', 'vitest/config'] : testPackages;
      if (!allowed.includes(spec) && !(spec.startsWith('node:') && !file.startsWith('src/'))) issues.push(`disallowed import: ${spec}`);
    }
  }
  return issues;
}

describe('package boundary', () => {
  it('classifies escaping paths, forbidden text and external packages', () => {
    expect(classify('src/a.ts', "im" + "port x from '../../" + 'editor/src/a' + "' ")).toContain("escaping import: ../../" + 'editor/src/a');
    expect(classify('src/a.ts', "const x = 'solid" + "-js'")).not.toEqual([]);
    expect(classify('src/a.ts', "im" + "port x from '@codemirror/" + "view'")).not.toEqual([]);
    expect(classify('src/a.ts', "im" + "port x from './ok'")).toEqual([]);
  });

  it('keeps the package independent', async () => {
    const spec: string = 'node:fs';
    const fs = await import(/* @vite-ignore */ spec) as NodeFs;
    const root = processApi.cwd();
    const violations: string[] = [];
    const walk = (relative: string): void => {
      for (const entry of fs.readdirSync(`${root}/${relative}`, { withFileTypes: true })) {
        if (entry.name === 'node_modules' || entry.name === 'dist') continue;
        const path = relative ? `${relative}/${entry.name}` : entry.name;
        if (entry.isDirectory()) walk(path);
        else if (entry.isFile() && /\.(?:ts|mjs|html)$/.test(entry.name)) {
          for (const issue of classify(path, fs.readFileSync(`${root}/${path}`, 'utf8'))) violations.push(`${path}: ${issue}`);
        }
      }
    };
    walk('');
    expect(violations, violations.join('\n')).toEqual([]);
  });
});
