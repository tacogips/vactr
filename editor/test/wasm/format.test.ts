// @vitest-environment node

import { describe, expect, it } from 'vitest';
import { WasmFormatter } from '../../src/code/format';
import { loadVactrWasm } from '../support/wasm';

interface NodeFs {
  readFileSync(path: string, encoding: 'utf8'): string;
}

const proc = (globalThis as unknown as { process: { cwd(): string } }).process;
const root = proc.cwd();

async function readText(path: string): Promise<string> {
  const spec: string = 'node:fs';
  const fs = (await import(/* @vite-ignore */ spec)) as NodeFs;
  return fs.readFileSync(path, 'utf8');
}

async function readWasm(): Promise<ArrayBuffer> {
  const spec: string = 'node:fs';
  const fs = (await import(/* @vite-ignore */ spec)) as { readFileSync(path: string): Uint8Array<ArrayBuffer> };
  const bytes = fs.readFileSync(`${root}/../target/wasm32-unknown-unknown/debug/vactr.wasm`);
  return bytes.slice().buffer;
}

describe('real wasm formatter', () => {
  it('formats the continuation fixture and is idempotent without touching the session outbox', async () => {
    const wasm = await loadVactrWasm();
    const input = await readText(`${root}/../src/fmt/tests/fixtures/continuation.in`);
    const expected = await readText(`${root}/../src/fmt/tests/fixtures/continuation.out`);
    const status = wasm.callStr('fmt_source', input);
    expect(status).toBe(0);
    const exports = wasm.exports as Record<string, () => number>;
    const memory = (wasm.exports as { memory: WebAssembly.Memory }).memory;
    const output = (): string => {
      const bytes = new Uint8Array(memory.buffer).slice(exports.fmt_out_ptr(), exports.fmt_out_ptr() + exports.fmt_out_len());
      return new TextDecoder().decode(bytes);
    };
    expect(output()).toBe(expected);
    expect(wasm.callStr('fmt_source', output())).toBe(0);
    expect(output()).toBe(expected);
    expect(wasm.drainRecords()).toEqual([]);
  });

  it('returns refused input unchanged and reports non-UTF-8 input', async () => {
    const wasm = await loadVactrWasm();
    const exports = wasm.exports as Record<string, () => number>;
    const memory = (wasm.exports as { memory: WebAssembly.Memory }).memory;
    const output = (): Uint8Array =>
      new Uint8Array(memory.buffer).slice(exports.fmt_out_ptr(), exports.fmt_out_ptr() + exports.fmt_out_len());
    const invalid = 's "abc\n';
    expect(wasm.callStr('fmt_source', invalid)).toBe(1);
    expect(new TextDecoder().decode(output())).toBe(invalid);
    expect(wasm.withBytes(new Uint8Array([0xff]), (ptr, len) => wasm.call('fmt_source', ptr, len))).toBe(2);
    expect(output()).toEqual(new Uint8Array([0xff]));
  });

  it('loads lazily through WasmFormatter and formats via the public API', async () => {
    const bytes = await readWasm();
    let fetches = 0;
    const formatter = new WasmFormatter('test://vactr.wasm', async (url) => {
      fetches += 1;
      expect(url).toBe('test://vactr.wasm');
      return new Response(bytes);
    });
    expect(fetches).toBe(0);
    const input = await readText(`${root}/../src/fmt/tests/fixtures/continuation.in`);
    const expected = await readText(`${root}/../src/fmt/tests/fixtures/continuation.out`);
    await expect(formatter.format(input)).resolves.toEqual({ status: 0, text: expected });
    expect(fetches).toBe(1);
    await formatter.format(input);
    expect(fetches).toBe(1);
  });
});
