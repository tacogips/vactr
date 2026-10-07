import { loadVactSyntax } from './syntax-core';
import { SyntaxWorkerCore, type SyntaxWorkerRequest } from './syntax-worker-core';

type WorkerScope = { onmessage: ((event: MessageEvent<SyntaxWorkerRequest>) => void) | null; postMessage(message: unknown, transfer?: Transferable[]): void };
const workerGlobal = (globalThis as typeof globalThis & { WorkerGlobalScope?: new (...args: never[]) => object }).WorkerGlobalScope;
if (workerGlobal && self instanceof workerGlobal) {
  const worker = self as unknown as WorkerScope;
  const core = new SyntaxWorkerCore({
    post(reply, transfer) { worker.postMessage(reply, transfer ?? []); },
    load: (base) => loadVactSyntax(base),
    defer: (callback) => { setTimeout(callback, 0); },
  });
  worker.onmessage = (event: MessageEvent<SyntaxWorkerRequest>) => core.handle(event.data);
}
