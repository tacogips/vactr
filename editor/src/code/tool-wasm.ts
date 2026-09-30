export interface ToolExports {
  memory: WebAssembly.Memory;
  alloc(len: number): number;
  free(ptr: number, len: number): void;
  [name: string]: unknown;
}

export class ToolWasm {
  private instance: Promise<ToolExports> | null = null;

  constructor(
    private readonly wasmUrl: string,
    private readonly fetchFn: (url: string) => Promise<Response> = (url) => fetch(url),
  ) {}

  exports(): Promise<ToolExports> {
    if (this.instance) return this.instance;
    const pending = this.fetchFn(this.wasmUrl).then(async (response) => {
      if (!response.ok) throw new Error(`Unable to load vactr wasm: ${response.status}`);
      const bytes = await response.arrayBuffer();
      const { instance } = await WebAssembly.instantiate(bytes, {});
      return instance.exports as unknown as ToolExports;
    });
    this.instance = pending;
    void pending.catch(() => {
      if (this.instance === pending) this.instance = null;
    });
    return pending;
  }
}
