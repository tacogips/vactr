import { defineConfig, type Plugin } from 'vite';
import solid from 'vite-plugin-solid';

// Build of the editor page (design 15.1.3 "Wasm artifact").
//
// The `vactr-assets` plugin reads the host-wasm artifact
// (`$VACTR_WASM`, else `../target/wasm32-unknown-unknown/debug/vactr.wasm`)
// and emits it as `vactr.wasm`, plus `worklet/processor.js`, into the
// RESOLVED output directory, so `vite build --outDir <dir>` keeps parallel
// plans apart (15.1.12). The build FAILS when the artifact is missing or
// lacks the wasm magic bytes, and, when `VACTR_REQUIRE_SESSION_ABI=1`,
// when the module does not export `session_init` (the export lands with the
// wasm session half; earlier waves build without the flag).

// No @types/node in the pinned dependency set: the few node APIs used here
// are typed locally and loaded through a non-literal dynamic import.
interface NodeFs {
  readFileSync(path: string): Uint8Array<ArrayBuffer>;
}

interface DevResponse {
  setHeader(name: string, value: string): void;
  end(body: Uint8Array): void;
}

const env = (globalThis as unknown as { process: { env: Record<string, string | undefined> } })
  .process.env;

const WASM_MAGIC = [0x00, 0x61, 0x73, 0x6d];
const DEFAULT_WASM = '../target/wasm32-unknown-unknown/debug/vactr.wasm';
const PROCESSOR = 'worklet/processor.js';

async function nodeFs(): Promise<NodeFs> {
  const spec: string = 'node:fs';
  return (await import(/* @vite-ignore */ spec)) as NodeFs;
}

function joinPath(root: string, p: string): string {
  if (p.startsWith('/') || /^[A-Za-z]:[\\/]/.test(p)) return p;
  return `${root.replace(/[\\/]+$/, '')}/${p}`;
}

interface Assets {
  wasm: Uint8Array<ArrayBuffer>;
  processor: Uint8Array<ArrayBuffer>;
}

/** Reads and validates both artifacts; throws a clear message on failure. */
async function loadAssets(root: string): Promise<Assets> {
  const fs = await nodeFs();
  const wasmPath = joinPath(root, env.VACTR_WASM || DEFAULT_WASM);
  let wasm: Uint8Array<ArrayBuffer>;
  try {
    wasm = fs.readFileSync(wasmPath);
  } catch {
    throw new Error(
      `vactr-assets: the wasm artifact ${wasmPath} is missing. Build it with ` +
        '`cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm` ' +
        'or set VACTR_WASM.',
    );
  }
  if (wasm.length < 8 || !WASM_MAGIC.every((b, i) => wasm[i] === b)) {
    throw new Error(`vactr-assets: ${wasmPath} is not a wasm module (bad magic bytes).`);
  }
  if (env.VACTR_REQUIRE_SESSION_ABI === '1') {
    const mod = await WebAssembly.compile(wasm);
    const names = WebAssembly.Module.exports(mod).map((e) => e.name);
    if (!names.includes('session_init')) {
      throw new Error(
        `vactr-assets: ${wasmPath} does not export session_init (VACTR_REQUIRE_SESSION_ABI=1).`,
      );
    }
  }
  const processor = fs.readFileSync(joinPath(root, PROCESSOR));
  return { wasm, processor };
}

export function vactrAssets(): Plugin {
  let root = '';
  let assets: Assets | null = null;
  return {
    name: 'vactr-assets',
    configResolved(config) {
      root = config.root;
    },
    async buildStart() {
      assets = await loadAssets(root);
    },
    generateBundle() {
      if (!assets) throw new Error('vactr-assets: buildStart did not load the artifacts');
      this.emitFile({ type: 'asset', fileName: 'vactr.wasm', source: assets.wasm });
      this.emitFile({ type: 'asset', fileName: PROCESSOR, source: assets.processor });
    },
    configureServer(server) {
      // `vite` (dev): serve the same two files from their sources.
      server.middlewares.use((req, res, next) => {
        const url = (req as { url?: string }).url ?? '';
        const path = url.split('?')[0];
        if (path !== '/vactr.wasm' && path !== `/${PROCESSOR}`) {
          next();
          return;
        }
        loadAssets(root).then(
          (a) => {
            const r = res as unknown as DevResponse;
            const wasm = path === '/vactr.wasm';
            r.setHeader('Content-Type', wasm ? 'application/wasm' : 'text/javascript');
            r.end(wasm ? a.wasm : a.processor);
          },
          (e: unknown) => next(e),
        );
      });
    },
  };
}

export default defineConfig({
  // Relative asset URLs: the same dist serves the browser and the Tauri shell.
  base: './',
  plugins: [solid(), vactrAssets()],
  build: {
    target: 'es2022',
  },
});
