// @vitest-environment node

import { describe, expect, it } from 'vitest';

interface DirEntry { name: string; isDirectory(): boolean; isFile(): boolean }
interface NodeFs {
  readdirSync(path: string, options: { withFileTypes: true }): DirEntry[];
  readFileSync(path: string, encoding: 'utf8'): string;
}
interface NodeProcess { cwd(): string }

const processApi = (globalThis as unknown as { process: NodeProcess }).process;

describe('production canvas surface boundary', () => {
  it('contains no CodeMirror view-bound modules or EditorView identifiers', async () => {
    const spec: string = 'node:fs';
    const fs = (await import(/* @vite-ignore */ spec)) as NodeFs;
    const root = `${processApi.cwd()}/src`;
    const files: string[] = [];
    const visit = (directory: string): void => {
      for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
        const path = `${directory}/${entry.name}`;
        if (entry.isDirectory()) visit(path);
        else if (entry.isFile() && /\.tsx?$/.test(entry.name)) files.push(path);
      }
    };
    visit(root);
    const violations: string[] = [];
    for (const file of files) {
      const lines = fs.readFileSync(file, 'utf8').split(/\r?\n/);
      lines.forEach((line, index) => {
        if (/from\s+['"]@codemirror\/(?:view|lint|language|autocomplete)['"]/.test(line) || /\bEditorView\b/.test(line)) {
          violations.push(`${file}:${index + 1}: ${line.trim()}`);
        }
      });
    }
    expect(violations, violations.join('\n')).toEqual([]);
  });
});
