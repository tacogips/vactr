// @vitest-environment node
import { describe, expect, it, vi } from 'vitest';
import { CompletionService, WasmCompletionEngine, utf16ToUtf8, utf8ToUtf16 } from '../../src/code/completion';
import type { CompletionEngine, RawCompletion } from '../../src/code/completion-types';
import { ToolWasm, type ToolExports } from '../../src/code/tool-wasm';

interface NodeFs {
  readFileSync(path: string, encoding: 'utf8'): string;
}

interface NodeProcess {
  cwd(): string;
}

const nodeFsModule = 'node:fs';
const proc = (globalThis as unknown as { process: NodeProcess }).process;

function raw(overrides: Partial<RawCompletion> = {}): RawCompletion {
  return {
    v: 1,
    context: 'head',
    from: 0,
    to: 0,
    incomplete: false,
    items: [],
    ...overrides,
  };
}

function fakeCompletionWasm(result: RawCompletion, status = 0) {
  const encoder = new TextEncoder();
  const output = encoder.encode(JSON.stringify(result));
  const memory = new WebAssembly.Memory({ initial: 1 });
  const outputPtr = 1024;
  new Uint8Array(memory.buffer).set(output, outputPtr);
  const calls: unknown[][] = [];
  const wasm = {
    memory,
    alloc: (len: number) => 0,
    free: vi.fn(),
    complete_source: (...args: number[]) => {
      calls.push(args);
      return status;
    },
    complete_out_ptr: () => outputPtr,
    complete_out_len: () => output.length,
  } as unknown as ToolExports;
  return { wasm, calls, output };
}

describe('completion offset conversion', () => {
  it('converts ASCII offsets in both directions and clamps the cursor', () => {
    expect(utf16ToUtf8('abc', 2)).toBe(2);
    expect(utf8ToUtf16('abc', 2)).toBe(2);
    expect(utf16ToUtf8('abc', -4)).toBe(0);
    expect(utf16ToUtf8('abc', 99)).toBe(3);
  });

  it('maps Japanese UTF-16 and UTF-8 offsets', () => {
    const text = '# 日本\nsi';
    expect(text.length).toBe(7);
    expect(utf16ToUtf8(text, 7)).toBe(11);
    expect(utf8ToUtf16(text, 9)).toBe(5);
    expect(utf8ToUtf16(text, 11)).toBe(7);
    expect(utf16ToUtf8('音x', 1)).toBe(3);
    expect(utf8ToUtf16('音x', 3)).toBe(1);
  });

  it('steps back inside a surrogate pair and counts the complete pair as four bytes', () => {
    const text = '\u{20bb7}a';
    expect(utf16ToUtf8(text, 1)).toBe(utf16ToUtf8(text, 0));
    expect(utf16ToUtf8(text, 0)).toBe(0);
    expect(utf16ToUtf8(text, 2)).toBe(4);
    expect(utf8ToUtf16(text, 4)).toBe(2);
  });
});

describe('CompletionService', () => {
  it('maps candidate ranges and preserves item order and incomplete', async () => {
    const items = [
      { label: 'alpha', kind: 'local' as const, detail: 'parameter', insert: 'alpha' },
      { label: 'analog', kind: 'keyword' as const, detail: 'keyword', insert: ':analog' },
    ];
    const engine: CompletionEngine = { complete: vi.fn(async () => raw({ from: 9, to: 11, incomplete: true, items })) };
    const service = new CompletionService(engine);
    await expect(service.complete('# 日本\nsi', 7)).resolves.toEqual({
      context: 'head',
      from: 5,
      to: 7,
      incomplete: true,
      items,
    });
    expect(engine.complete).toHaveBeenCalledWith('# 日本\nsi', 11, 100);
    expect(service.available).toBe(true);
  });

  it('returns null for null results and the none context', async () => {
    const none = new CompletionService({ complete: async () => raw({ context: 'none' }) });
    const missing = new CompletionService({ complete: async () => null });
    await expect(none.complete('x', 1)).resolves.toBeNull();
    await expect(missing.complete('x', 1)).resolves.toBeNull();
    expect(none.available).toBe(true);
  });

  it('disables itself after the first engine failure without throwing', async () => {
    const complete = vi.fn(async () => { throw new Error('engine failed'); });
    const service = new CompletionService({ complete });
    await expect(service.complete('x', 1)).resolves.toBeNull();
    expect(service.available).toBe(false);
    await expect(service.complete('x', 1)).resolves.toBeNull();
    expect(complete).toHaveBeenCalledTimes(1);
  });

  it('uses the default item limit of 100', async () => {
    const complete = vi.fn(async () => raw());
    await new CompletionService({ complete }).complete('x', 1);
    expect(complete).toHaveBeenCalledWith('x', 1, 100);
    const limited = vi.fn(async () => raw());
    await new CompletionService({ complete: limited }, 25).complete('x', 1);
    expect(limited).toHaveBeenCalledWith('x', 1, 25);
  });
});

describe('WasmCompletionEngine', () => {
  it('calls the pinned exports, reads JSON from wasm memory, and frees the input', async () => {
    const result = raw({ from: 1, to: 3, items: [{ label: 'x', kind: 'local', detail: '', insert: 'x' }] });
    const { wasm, calls } = fakeCompletionWasm(result);
    const tool = { exports: async () => wasm } as unknown as ToolWasm;
    const engine = new WasmCompletionEngine(tool);
    await expect(engine.complete('abc', 2, 100)).resolves.toEqual(result);
    expect(calls).toEqual([[0, 3, 2, 100]]);
    expect(wasm.free).toHaveBeenCalledWith(0, 3);
  });

  it('returns null for nonzero ABI status and always frees input', async () => {
    const { wasm } = fakeCompletionWasm(raw(), 2);
    const engine = new WasmCompletionEngine({ exports: async () => wasm } as unknown as ToolWasm);
    await expect(engine.complete('x', 1, 100)).resolves.toBeNull();
    expect(wasm.free).toHaveBeenCalledWith(0, 1);
  });

  it('rejects when required wasm exports are absent', async () => {
    const wasm = { memory: new WebAssembly.Memory({ initial: 1 }), alloc: () => 0, free: vi.fn() } as unknown as ToolExports;
    const engine = new WasmCompletionEngine({ exports: async () => wasm } as unknown as ToolWasm);
    await expect(engine.complete('x', 1, 100)).rejects.toThrow('completion exports are unavailable');
  });

  it('lets the service disable itself when shared ToolWasm loading rejects', async () => {
    const tool = new ToolWasm('/missing.wasm', async () => { throw new Error('offline'); });
    const service = new CompletionService(new WasmCompletionEngine(tool));
    await expect(service.complete('x', 1)).resolves.toBeNull();
    expect(service.available).toBe(false);
  });

  it('contains no CodeMirror imports', async () => {
    const fs = (await import(/* @vite-ignore */ nodeFsModule)) as NodeFs;
    const source = fs.readFileSync(`${proc.cwd()}/src/code/completion.ts`, 'utf8');
    expect(source).not.toContain('@codemirror/');
  });
});
