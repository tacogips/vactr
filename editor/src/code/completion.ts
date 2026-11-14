import type { ToolExports } from './tool-wasm';
import { ToolWasm } from './tool-wasm';
import type {
  CompletionEngine,
  CompletionResult,
  CompletionSource,
  RawCompletion,
} from './completion-types';

interface CompletionExports extends ToolExports {
  complete_source?: unknown;
  complete_out_ptr?: unknown;
  complete_out_len?: unknown;
}

function codePointWidth(text: string, index: number): { bytes: number; units: number } {
  const point = text.codePointAt(index);
  if (point === undefined) return { bytes: 0, units: 0 };
  const units = point > 0xffff ? 2 : 1;
  if (point < 0x80) return { bytes: 1, units };
  if (point < 0x800) return { bytes: 2, units };
  if (point > 0xffff) return { bytes: 4, units };
  return { bytes: 3, units };
}

export function utf16ToUtf8(text: string, cursor16: number): number {
  let end = Number.isFinite(cursor16) ? Math.trunc(cursor16) : 0;
  end = Math.max(0, Math.min(text.length, end));
  if (
    end > 0 &&
    end < text.length &&
    text.charCodeAt(end - 1) >= 0xd800 &&
    text.charCodeAt(end - 1) <= 0xdbff &&
    text.charCodeAt(end) >= 0xdc00 &&
    text.charCodeAt(end) <= 0xdfff
  ) {
    end -= 1;
  }

  let bytes = 0;
  for (let index = 0; index < end;) {
    const width = codePointWidth(text, index);
    bytes += width.bytes;
    index += width.units;
  }
  return bytes;
}

export function utf8ToUtf16(text: string, byteOffset: number): number {
  const target = Number.isFinite(byteOffset) ? Math.max(0, Math.trunc(byteOffset)) : 0;
  if (target === 0) return 0;

  let bytes = 0;
  for (let index = 0; index < text.length;) {
    const width = codePointWidth(text, index);
    bytes += width.bytes;
    index += width.units;
    if (bytes >= target) return index;
  }
  return text.length;
}

export class WasmCompletionEngine implements CompletionEngine {
  constructor(private readonly tool: ToolWasm) {}

  async complete(text: string, byteCursor: number, limit: number): Promise<RawCompletion | null> {
    const wasm = (await this.tool.exports()) as CompletionExports;
    if (
      typeof wasm.complete_source !== 'function' ||
      typeof wasm.complete_out_ptr !== 'function' ||
      typeof wasm.complete_out_len !== 'function'
    ) {
      throw new Error('vactr wasm completion exports are unavailable');
    }

    const input = new TextEncoder().encode(text);
    const ptr = wasm.alloc(input.length);
    try {
      new Uint8Array(wasm.memory.buffer).set(input, ptr);
      const status = wasm.complete_source(ptr, input.length, byteCursor, limit);
      if (status !== 0) return null;

      // The completion call may grow wasm memory; always create the view afterward.
      const memory = new Uint8Array(wasm.memory.buffer);
      const outPtr = wasm.complete_out_ptr();
      const outLen = wasm.complete_out_len();
      const output = memory.slice(outPtr, outPtr + outLen);
      return JSON.parse(new TextDecoder().decode(output)) as RawCompletion;
    } finally {
      wasm.free(ptr, input.length);
    }
  }
}

export class CompletionService implements CompletionSource {
  private enabled = true;

  constructor(
    private readonly engine: CompletionEngine,
    private readonly limit = 100,
  ) {}

  get available(): boolean {
    return this.enabled;
  }

  async complete(text: string, cursor16: number): Promise<CompletionResult | null> {
    if (!this.enabled) return null;
    try {
      const raw = await this.engine.complete(text, utf16ToUtf8(text, cursor16), this.limit);
      if (raw === null || raw.context === 'none') return null;
      return {
        context: raw.context,
        from: utf8ToUtf16(text, raw.from),
        to: utf8ToUtf16(text, raw.to),
        incomplete: raw.incomplete,
        items: raw.items,
      };
    } catch {
      this.enabled = false;
      return null;
    }
  }
}
