// @vitest-environment node

import { describe, expect, it } from 'vitest';
import { loadVactrWasm } from '../support/wasm';

interface CompletionJson {
  v: number;
  context: string;
  from: number;
  to: number;
  incomplete: boolean;
  items: Array<{ label: string; kind: string; detail: string; insert: string }>;
}

const encoder = new TextEncoder();

function complete(wasm: Awaited<ReturnType<typeof loadVactrWasm>>, text: string, cursor: number) {
  const bytes = encoder.encode(text);
  const status = wasm.withBytes(bytes, (ptr, len) =>
    wasm.call('complete_source', ptr, len, cursor, 0),
  );
  const exports = wasm.exports as Record<string, () => number>;
  const memory = (wasm.exports as { memory: WebAssembly.Memory }).memory;
  const outputBytes = new Uint8Array(memory.buffer).slice(
    exports.complete_out_ptr(),
    exports.complete_out_ptr() + exports.complete_out_len(),
  );
  return { status, output: JSON.parse(new TextDecoder().decode(outputBytes)) as CompletionJson };
}

describe('real wasm completion', () => {
  it('returns a scoped local with UTF-8 byte replace offsets', async () => {
    const wasm = await loadVactrWasm();
    const text = 'fn f alpha:\n\tal';
    const cursor = encoder.encode(text).length;
    const { status, output } = complete(wasm, text, cursor);

    expect(status).toBe(0);
    expect(output.v).toBe(1);
    expect(output.context).toBe('head');
    expect(output.items[0]).toMatchObject({ label: 'alpha', kind: 'local' });
    expect(output.from).toBe(encoder.encode(text.slice(0, text.lastIndexOf('al'))).length);
    expect(output.to).toBe(cursor);
    expect(wasm.drainRecords()).toEqual([]);
  });

  it('reports UTF-8 byte offsets after a Japanese comment', async () => {
    const wasm = await loadVactrWasm();
    const text = '// 日本語\ns si';
    const bytes = encoder.encode(text);
    const cursor = bytes.length;
    const { status, output } = complete(wasm, text, cursor);

    expect(status).toBe(0);
    expect(output.from).toBe(encoder.encode(text.slice(0, text.lastIndexOf('si'))).length);
    expect(output.to).toBe(cursor);
    expect(wasm.drainRecords()).toEqual([]);
  });

  it('offers manifest keywords in keyword context', async () => {
    const wasm = await loadVactrWasm();
    const text = 's :an';
    const { status, output } = complete(wasm, text, encoder.encode(text).length);

    expect(status).toBe(0);
    expect(output.context).toBe('keyword');
    expect(output.items.some(({ label }) => label === ':analog')).toBe(true);
    expect(wasm.drainRecords()).toEqual([]);
  });

  it('returns status 2 for invalid UTF-8 and status 3 for an invalid cursor', async () => {
    const wasm = await loadVactrWasm();
    expect(wasm.withBytes(new Uint8Array([0xff]), (ptr, len) =>
      wasm.call('complete_source', ptr, len, 0, 0),
    )).toBe(2);
    expect(wasm.callStr('complete_source', 'abc', 99, 0)).toBe(3);
    expect(wasm.drainRecords()).toEqual([]);
  });
});
