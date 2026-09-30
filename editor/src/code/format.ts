import type { Extension } from '@codemirror/state';
import { keymap, type EditorView } from '@codemirror/view';

export const FORMAT_KEY = 'Shift-Alt-f';

export interface FormatResult {
  status: number;
  text: string;
}

export interface Formatter {
  format(text: string): Promise<FormatResult>;
}

interface FormatterExports {
  memory: WebAssembly.Memory;
  alloc(len: number): number;
  free(ptr: number, len: number): void;
  fmt_source(ptr: number, len: number): number;
  fmt_out_ptr(): number;
  fmt_out_len(): number;
}

export class WasmFormatter implements Formatter {
  private instance: Promise<FormatterExports> | null = null;

  constructor(
    private readonly wasmUrl: string,
    private readonly fetchFn: (url: string) => Promise<Response> = (url) => fetch(url),
  ) {}

  async format(text: string): Promise<FormatResult> {
    const wasm = await this.load();
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

  private load(): Promise<FormatterExports> {
    if (this.instance) return this.instance;
    const pending = this.fetchFn(this.wasmUrl).then(async (response) => {
      if (!response.ok) throw new Error(`Unable to load formatter wasm: ${response.status}`);
      const bytes = await response.arrayBuffer();
      const { instance } = await WebAssembly.instantiate(bytes, {});
      return instance.exports as unknown as FormatterExports;
    });
    this.instance = pending;
    void pending.catch(() => {
      if (this.instance === pending) this.instance = null;
    });
    return pending;
  }
}

export async function formatDocument(view: EditorView, formatter: Formatter): Promise<boolean> {
  const before = view.state.doc.toString();
  try {
    const result = await formatter.format(before);
    if (result.status !== 0 || result.text === before || view.state.doc.toString() !== before) return false;
    let prefix = 0;
    const limit = Math.min(before.length, result.text.length);
    while (prefix < limit && before[prefix] === result.text[prefix]) prefix += 1;
    let suffix = 0;
    while (
      suffix < before.length - prefix &&
      suffix < result.text.length - prefix &&
      before[before.length - suffix - 1] === result.text[result.text.length - suffix - 1]
    ) {
      suffix += 1;
    }
    view.dispatch({
      changes: {
        from: prefix,
        to: before.length - suffix,
        insert: result.text.slice(prefix, result.text.length - suffix),
      },
      userEvent: 'format',
    });
    return true;
  } catch {
    return false;
  }
}

export function formatKeymap(formatter: () => Formatter | undefined): Extension {
  return keymap.of([
    {
      key: FORMAT_KEY,
      run: (view) => {
        const current = formatter();
        if (!current) return false;
        void formatDocument(view, current);
        return true;
      },
    },
  ]);
}
