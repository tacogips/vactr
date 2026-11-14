// @vitest-environment node

import { describe, expect, it } from 'vitest';

interface NodeFs {
  readdirSync(path: string, options: { withFileTypes: true }): Array<{
    name: string;
    isDirectory(): boolean;
    isFile(): boolean;
  }>;
  readFileSync(path: string, encoding: 'utf8'): string;
}

interface NodePath {
  join(...parts: string[]): string;
  relative(from: string, to: string): string;
}

const proc = (globalThis as unknown as { process: { cwd(): string } }).process;
const root = proc.cwd();
const expectedTokens = [
  '--vt-bg', '--vt-bg-sunken', '--vt-surface', '--vt-raised', '--vt-raised-hover', '--vt-raised-active',
  '--vt-border', '--vt-border-strong', '--vt-text', '--vt-text-muted', '--vt-text-disabled',
  '--vt-accent', '--vt-accent-hover', '--vt-accent-active', '--vt-on-accent', '--vt-selection',
  '--vt-success', '--vt-warn', '--vt-danger', '--vt-danger-bg', '--vt-danger-text',
  '--vt-data-1', '--vt-data-2', '--vt-flash', '--vt-flash-error', '--vt-playing-bg', '--vt-playing-outline',
  '--vt-syn-comment', '--vt-syn-directive', '--vt-syn-keyword', '--vt-syn-number', '--vt-syn-string',
  '--vt-syn-head', '--vt-syn-bracket', '--vt-icon-chevron', '--vt-icon-check',
  '--vt-font-sans', '--vt-font-mono', '--vt-text-xs', '--vt-text-sm', '--vt-text-md', '--vt-text-lg',
  '--vt-space-1', '--vt-space-2', '--vt-space-3', '--vt-space-4', '--vt-space-5',
  '--vt-control-h', '--vt-control-h-compact', '--vt-header-h', '--vt-border-w', '--vt-focus-w',
  '--vt-focus-offset', '--vt-radius',
];
const colorProperties = /^(?:color|background|background-color|border(?:-[\w-]+)?|outline(?:-[\w-]+)?|fill|stroke|box-shadow|caret-color|text-decoration(?:-[\w-]+)?|-webkit-text-fill-color)$/i;
const namedColors = ['white', 'black', 'red', 'green', 'blue', 'gray', 'grey', 'yellow', 'orange', 'silver'];

async function loadModules(): Promise<{ fs: NodeFs; path: NodePath }> {
  const fsSpec: string = 'node:fs';
  const pathSpec: string = 'node:path';
  return {
    fs: (await import(/* @vite-ignore */ fsSpec)) as NodeFs,
    path: (await import(/* @vite-ignore */ pathSpec)) as NodePath,
  };
}

function stripComments(css: string): string {
  return css.replace(/\/\*[\s\S]*?\*\//g, '');
}

function declarations(css: string): Array<{ property: string; value: string }> {
  const found: Array<{ property: string; value: string }> = [];
  const pattern = /(?:^|[;{])\s*([\w-]+)\s*:\s*([^;{}]+)/g;
  for (const match of css.matchAll(pattern)) {
    found.push({ property: match[1].trim(), value: match[2].trim() });
  }
  return found;
}

function hasNamedColor(value: string): boolean {
  return namedColors.some((name) => {
    const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    return new RegExp(`(^|[^\\p{L}\\p{N}_-])${escaped}($|[^\\p{L}\\p{N}_-])`, 'iu').test(value);
  });
}

function collectCss(fs: NodeFs, path: NodePath, directory: string): string[] {
  const files: string[] = [];
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const fullPath = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...collectCss(fs, path, fullPath));
    else if (entry.isFile() && entry.name.endsWith('.css')) files.push(fullPath);
  }
  return files;
}

describe('UI style tokens', () => {
  it('uses only square border-radius declarations in every stylesheet', async () => {
    const { fs, path } = await loadModules();
    const files = collectCss(fs, path, path.join(root, 'src'));
    expect(files.length).toBeGreaterThanOrEqual(7);
    for (const file of files) {
      const css = stripComments(fs.readFileSync(file, 'utf8'));
      const radius = /(?:^|[;{])\s*(border(?:-[\w-]+)?-radius)\s*:\s*([^;{}]+)/gi;
      for (const match of css.matchAll(radius)) {
        expect(match[2].trim(), `${path.relative(root, file)}: ${match[1]}: ${match[2].trim()}`)
          .toMatch(/^(?:0|var\(--vt-radius\))$/);
      }
    }
  });

  it('keeps color literals out of component stylesheet values', async () => {
    const { fs, path } = await loadModules();
    const files = collectCss(fs, path, path.join(root, 'src'));
    for (const file of files) {
      if (path.relative(root, file) === 'src/app/theme.css') continue;
      const css = stripComments(fs.readFileSync(file, 'utf8'));
      for (const { property, value } of declarations(css)) {
        if (!colorProperties.test(property)) continue;
        const literal = /#[\da-f]{3,8}\b|\brgba?\s*\(|\bhsla?\s*\(/i.test(value) || hasNamedColor(value);
        expect(literal, `${path.relative(root, file)}: ${property}: ${value}`).toBe(false);
      }
    }
  });

  it('keeps generic font-family keywords out of component stylesheets', async () => {
    const { fs, path } = await loadModules();
    const files = collectCss(fs, path, path.join(root, 'src'));
    const generic = /\b(?:monospace|sans-serif|system-ui|ui-monospace|ui-sans-serif)\b/i;
    for (const file of files) {
      if (path.relative(root, file) === 'src/app/theme.css') continue;
      expect(generic.test(stripComments(fs.readFileSync(file, 'utf8'))), path.relative(root, file)).toBe(false);
    }
  });

  it('limits theme.css to root token blocks and declares every design token', async () => {
    const { fs, path } = await loadModules();
    const themePath = path.join(root, 'src/app/theme.css');
    const theme = stripComments(fs.readFileSync(themePath, 'utf8'));
    let remainder = theme
      .replace(/@media\s*\(pointer:\s*coarse\)\s*\{\s*:root\s*\{[^{}]*\}\s*\}/g, '')
      .replace(/:root\s*\{[^{}]*\}/g, '')
      .trim();
    expect(remainder).toBe('');
    const declared = new Set(declarations(theme).map(({ property }) => property));
    for (const token of expectedTokens) expect(declared.has(token), token).toBe(true);
  });

  it('loads the theme link before app.css', async () => {
    const { fs, path } = await loadModules();
    const html = fs.readFileSync(path.join(root, 'index.html'), 'utf8');
    const themeIndex = html.indexOf('./src/app/theme.css');
    const appIndex = html.indexOf('./src/app/app.css');
    expect(themeIndex).toBeGreaterThanOrEqual(0);
    expect(appIndex).toBeGreaterThan(themeIndex);
  });

  it('pairs transitions and animations with reduced-motion handling', async () => {
    const { fs, path } = await loadModules();
    const files = collectCss(fs, path, path.join(root, 'src'));
    const motionDeclaration = /(?:^|[;{])\s*(?:transition(?:-[\w-]+)?|animation(?:-[\w-]+)?)\s*:/i;
    for (const file of files) {
      const css = stripComments(fs.readFileSync(file, 'utf8'));
      if (motionDeclaration.test(css)) {
        expect(css, path.relative(root, file)).toMatch(/prefers-reduced-motion/i);
      }
    }
  });
});
