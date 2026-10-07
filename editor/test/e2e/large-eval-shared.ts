import { expect } from 'vitest';

interface Wasm {
  call(name: string, ...args: number[]): number;
  callStr(name: string, text: string): number;
  drainRecords(): Array<{ tag: number; bytes: Uint8Array }>;
}

interface Helpers {
  loadVactrWasm(): Promise<Wasm>;
}

interface FixtureModule {
  createLargeDocument(): { text: string };
}

interface EvalEnvelope {
  kind: string;
  re?: number;
  body: { diagnostics?: Array<{ severity: string }> };
}

export const LARGE_EVAL_RATIO_LIMIT = 6;

export interface LargeEvalMeasurement {
  smallMedian: number;
  largeMedian: number;
  ratio: number;
}

const fixtureSpec: string = './fixtures/large-doc.mjs';
const helperSpec: string = '../support/wasm.ts';
const fixture = (await import(/* @vite-ignore */ fixtureSpec)) as FixtureModule;
const helpers = (await import(/* @vite-ignore */ helperSpec)) as Helpers;
const decode = new TextDecoder();

async function evaluate(code: string): Promise<{ elapsedMs: number; results: EvalEnvelope[] }> {
  const wasm = await helpers.loadVactrWasm();
  expect(wasm.call('session_init', 48_000, 0)).toBe(1);
  wasm.drainRecords();
  const request = JSON.stringify({
    v: 1,
    seq: 1,
    kind: 'eval',
    body: { file: 'main.vact', code, doc_revision: 1, edit_epoch: 1 },
  });
  const started = performance.now();
  wasm.callStr('session_apply', request);
  const elapsedMs = performance.now() - started;
  const records = wasm.drainRecords().filter((record) => record.tag === 0x71);
  const results = records
    .map((record) => JSON.parse(decode.decode(record.bytes)) as EvalEnvelope)
    .filter((record) => record.kind === 'eval-result' && record.re === 1);
  return { elapsedMs, results };
}

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)] ?? 0;
}

export async function measureLargeEval(label: string): Promise<LargeEvalMeasurement> {
  const text = fixture.createLargeDocument().text;
  const lines = text.split('\n');
  const small = lines.slice(0, 5_000).join('\n');
  const samples: Record<string, number[]> = { '5000': [], '20000': [] };

  for (let run = 0; run < 3; run += 1) {
    const smallResult = await evaluate(small);
    expect(smallResult.results).toHaveLength(1);
    samples['5000']?.push(smallResult.elapsedMs);

    const largeResult = await evaluate(text);
    expect(largeResult.results).toHaveLength(1);
    expect(
      largeResult.results[0]?.body.diagnostics?.filter(
        (diagnostic) => diagnostic.severity === 'error',
      ),
    ).toEqual([]);
    samples['20000']?.push(largeResult.elapsedMs);
  }

  const smallMedian = median(samples['5000'] ?? []);
  const largeMedian = median(samples['20000'] ?? []);
  const ratio = largeMedian / Math.max(smallMedian, 0.01);
  const stdout = (globalThis as unknown as { process: { stdout: { write(value: string): void } } })
    .process.stdout;
  stdout.write(
    `${label} medians: 5000=${smallMedian.toFixed(1)}ms 20000=${largeMedian.toFixed(1)}ms ratio=${ratio.toFixed(2)}\n`,
  );
  return { smallMedian, largeMedian, ratio };
}
