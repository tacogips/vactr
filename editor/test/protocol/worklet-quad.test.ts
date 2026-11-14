import { afterEach, describe, expect, it, vi } from 'vitest';

type QuadProcessor = {
  port: { onmessage: ((e: { data: unknown }) => void) | null; posted: unknown[] };
  init(bytes: ArrayBuffer): Promise<void>;
  process(inputs: unknown[], outputs: Float32Array[][]): boolean;
};

describe('worklet four-channel output', () => {
  afterEach(() => vi.restoreAllMocks());

  it('initializes the quad ABI and copies planar wasm lanes 0..3 independently', async () => {
    const spec: string = 'node:fs';
    const fs = (await import(/* @vite-ignore */ spec)) as {
      readFileSync(path: string, encoding: 'utf8'): string;
    };
    const source = fs.readFileSync('worklet/processor.js', 'utf8');
    let Processor: (new (options: unknown) => QuadProcessor) | undefined;
    class BaseProcessor {
      port = { onmessage: null, posted: [] as unknown[], postMessage: (m: unknown) => this.port.posted.push(m) };
    }
    const register = (_name: string, ctor: new (options: unknown) => QuadProcessor) => {
      Processor = ctor;
    };
    const memory = { buffer: new ArrayBuffer(8192) };
    const expected = [
      [0.1, 0.2, 0.3, 0.4],
      [1.1, 1.2, 1.3, 1.4],
      [2.1, 2.2, 2.3, 2.4],
      [3.1, 3.2, 3.3, 3.4],
    ];
    let quadInit = 0;
    let stereoInit = 0;
    const seenInput: number[][] = [];
    const x = {
      memory,
      worklet_init_quad: () => { quadInit += 1; },
      worklet_init: () => { stereoInit += 1; },
      staging_ptr: () => 512,
      input_ptr: () => 4096,
      report_ptr: () => 256,
      report_len: () => 2,
      process: () => {
        seenInput.push(Array.from(new Float32Array(memory.buffer, 4096, 8)));
        new Float32Array(memory.buffer, 0, 16).set(expected.flat());
        return 0;
      },
      outbox_len: () => 0,
      worklet_now: () => 0,
    };
    const wasm = { instantiate: vi.fn(async () => ({ instance: { exports: x } })) };
    new Function('AudioWorkletProcessor', 'registerProcessor', 'sampleRate', 'currentFrame', 'WebAssembly', source)(
      BaseProcessor,
      register,
      48000,
      0,
      wasm,
    );
    expect(Processor).toBeDefined();
    const processor = new Processor!({ processorOptions: { outputChannels: 4 } });
    await processor.init(new ArrayBuffer(8));
    const lanes = Array.from({ length: 4 }, () => new Float32Array(4));
    expect(processor.process([], [lanes])).toBe(true);
    expect(quadInit).toBe(1);
    expect(stereoInit).toBe(0);
    expect(lanes.map((lane) => Array.from(lane))).toEqual(expected.map((lane) => lane.map(Math.fround)));
    expect(seenInput[0]).toEqual(Array(8).fill(0));
    const left = Float32Array.from([0.1, 0.2, Number.NaN, 0.4]);
    const right = Float32Array.from([1.1, 1.2, 1.3, Number.POSITIVE_INFINITY]);
    processor.process([[left, right]], [lanes]);
    expect(seenInput[1]).toEqual([0.1, 1.1, 0.2, 1.2, 0, 1.3, 0.4, 0].map(Math.fround));
    processor.process([[left]], [lanes]);
    expect(seenInput[2]).toEqual([0.1, 0.1, 0.2, 0.2, 0, 0, 0.4, 0.4].map(Math.fround));
    processor.process([], [lanes]);
    expect(seenInput[3]).toEqual(Array(8).fill(0));
    expect(processor.port.posted).toContainEqual(expect.objectContaining({ type: 'ready' }));
  });
});
