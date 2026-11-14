// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { CompletionService, WasmCompletionEngine } from '../../src/code/completion';
import { ToolWasm } from '../../src/code/tool-wasm';
import { loadVactrWasm } from '../support/wasm';

async function serviceFromArtifact(): Promise<CompletionService> {
  const artifact = await loadVactrWasm();
  const spec: string = 'node:fs';
  const fs = await import(/* @vite-ignore */ spec) as { readFileSync(path: string): Uint8Array<ArrayBuffer> };
  const bytes = fs.readFileSync(artifact.path);
  const tool = new ToolWasm('test://vactr.wasm', async () => new Response(bytes));
  return new CompletionService(new WasmCompletionEngine(tool));
}

describe('completion engine through the real wasm artifact', () => {
  it('completes a function parameter with UTF-16 range offsets', async () => {
    const source = 'fn f alpha:\n\tal';
    const result = await (await serviceFromArtifact()).complete(source, source.length);
    expect(result?.items[0]?.label).toBe('alpha');
    expect(result && [result.from, result.to]).toEqual([source.length - 2, source.length]);
  });

  it('keeps UTF-16 ranges correct after a Japanese comment', async () => {
    const source = '# 音の前書き\nfn f alpha:\n\tal';
    const result = await (await serviceFromArtifact()).complete(source, source.length);
    expect(result?.items[0]?.label).toBe('alpha');
    expect(result && [result.from, result.to]).toEqual([source.length - 2, source.length]);
  });
});
