import { describe, expect, it } from 'vitest';
import { COMPLETION_KEYS, COMPLETION_USER_EVENT, isTriggerChar } from '../../src/code/completion-types';

interface NodeFs {
  readFileSync(path: string, encoding: 'utf8'): string;
}

interface NodeProcess {
  cwd(): string;
}

const nodeFsModule = 'node:fs';
const proc = (globalThis as unknown as { process: NodeProcess }).process;

describe('completion contract types', () => {
  it.each(['a', 'Z', '7', '-', ':', '.'])('accepts trigger character %j', (ch) => {
    expect(isTriggerChar(ch)).toBe(true);
  });

  it.each([' ', '\t', '{', '"', '', 'ab', '\u3042', '\u65e5'])('rejects non-trigger input %j', (ch) => {
    expect(isTriggerChar(ch)).toBe(false);
  });

  it('exposes the pinned completion keys in order', () => {
    expect(COMPLETION_KEYS).toEqual([
      'ArrowUp',
      'ArrowDown',
      'PageUp',
      'PageDown',
      'Enter',
      'Tab',
      'Escape',
      'Ctrl-Space',
    ]);
    expect(COMPLETION_KEYS).not.toContain('Mod-Enter');
  });

  it('exposes the pinned completion user event', () => {
    expect(COMPLETION_USER_EVENT).toBe('input.complete');
  });

  it('keeps the completion contract independent of CodeMirror', async () => {
    const fs = (await import(/* @vite-ignore */ nodeFsModule)) as NodeFs;
    const source = fs.readFileSync(`${proc.cwd()}/src/code/completion-types.ts`, 'utf8');
    expect(source).not.toContain('@codemirror');
  });
});
