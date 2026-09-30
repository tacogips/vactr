// @vitest-environment node

import { describe, expect, it, vi } from 'vitest';
import { ToolWasm } from '../../src/code/tool-wasm';

const emptyWasm = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]);

describe('ToolWasm', () => {
  it('loads lazily and shares one instance across exports calls', async () => {
    const fetchFn = vi.fn(async () => new Response(emptyWasm));
    const tool = new ToolWasm('/vactr.wasm', fetchFn);
    expect(fetchFn).not.toHaveBeenCalled();

    const first = await tool.exports();
    const second = await tool.exports();
    expect(fetchFn).toHaveBeenCalledTimes(1);
    expect(second).toBe(first);
  });

  it('forgets a rejected load and retries on the next call', async () => {
    const fetchFn = vi
      .fn<() => Promise<Response>>()
      .mockResolvedValueOnce(new Response('', { status: 404 }))
      .mockResolvedValueOnce(new Response(emptyWasm));
    const tool = new ToolWasm('/vactr.wasm', fetchFn);

    await expect(tool.exports()).rejects.toThrow('404');
    await expect(tool.exports()).resolves.toBeDefined();
    expect(fetchFn).toHaveBeenCalledTimes(2);
  });
});
