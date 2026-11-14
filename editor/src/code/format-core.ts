import { ToolWasm, type ToolExports } from './tool-wasm';

export interface FormatResult {
  status: number;
  text: string;
}

export interface Formatter {
  format(text: string): Promise<FormatResult>;
}

interface FormatterExports extends ToolExports {
  fmt_source(ptr: number, len: number): number;
  fmt_out_ptr(): number;
  fmt_out_len(): number;
}

export class WasmFormatter implements Formatter {
  private readonly tool: ToolWasm;

  constructor(source: string | ToolWasm, fetchFn?: (url: string) => Promise<Response>) {
    this.tool = typeof source === 'string' ? new ToolWasm(source, fetchFn) : source;
  }

  async format(text: string): Promise<FormatResult> {
    const wasm = (await this.tool.exports()) as FormatterExports;
    const input = new TextEncoder().encode(text);
    const ptr = wasm.alloc(input.length);
    try {
      new Uint8Array(wasm.memory.buffer).set(input, ptr);
      const status = wasm.fmt_source(ptr, input.length);
      // fmt_source can grow memory; create a new view only after it returns.
      const memory = new Uint8Array(wasm.memory.buffer);
      const outPtr = wasm.fmt_out_ptr();
      const outLen = wasm.fmt_out_len();
      const output = memory.slice(outPtr, outPtr + outLen);
      return { status, text: new TextDecoder().decode(output) };
    } finally {
      wasm.free(ptr, input.length);
    }
  }
}

export function minimalChange(before: string, after: string): { from: number; to: number; insert: string } | null {
  if (before === after) return null;
  let prefix = 0;
  const limit = Math.min(before.length, after.length);
  while (prefix < limit && before[prefix] === after[prefix]) prefix += 1;
  let suffix = 0;
  while (
    suffix < before.length - prefix &&
    suffix < after.length - prefix &&
    before[before.length - suffix - 1] === after[after.length - suffix - 1]
  ) {
    suffix += 1;
  }
  return {
    from: prefix,
    to: before.length - suffix,
    insert: after.slice(prefix, after.length - suffix),
  };
}
