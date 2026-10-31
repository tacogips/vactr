import type { EditorHandle, InputSample, PresentSample } from '../../src/editor';

export interface KeyMetric { a: number | null; b: number | null; nextFrame: boolean; missing: boolean }
/**
 * Returns one metric per editing key: raw latency `a`, input plus frame work `b`,
 * whether it reached the next frame, and whether a presenting frame is missing.
 * Missing metrics have null timings. Keydowns without a matching beforeinput are omitted.
 */
export function keyMetrics(inputs: readonly InputSample[], presents: readonly PresentSample[]): KeyMetric[] {
  const metrics: KeyMetric[] = [];
  const used = new Set<number>();
  for (let i = 0; i < inputs.length; i++) {
    const key = inputs[i]!;
    if (key.type !== 'keydown') continue;
    const nextKey = inputs.findIndex((sample, index) => index > i && sample.type === 'keydown');
    const beforeIndex = inputs.findIndex((sample, index) => index > i && (nextKey < 0 || index < nextKey) &&
      !used.has(index) && sample.type === 'beforeinput' && sample.revisionBefore === key.revisionBefore);
    if (beforeIndex < 0) continue;
    used.add(beforeIndex);
    const before = inputs[beforeIndex]!;
    const presentIndex = presents.findIndex(sample => sample.revision >= before.revisionAfter && sample.frameStart >= key.end);
    if (presentIndex < 0) { metrics.push({ a: null, b: null, nextFrame: false, missing: true }); continue; }
    const present = presents[presentIndex]!;
    const nextFrame = !presents.some((sample, index) => index !== presentIndex && sample.frameStart >= key.end && sample.frameStart < present.frameStart);
    metrics.push({ a: present.frameEnd - key.eventTimeStamp, b: key.workMs + before.workMs + present.workMs, nextFrame, missing: false });
  }
  return metrics;
}

export interface PerfHook {
  load(): Promise<{ firstPaintMs: number }>;
  startRecording(): void;
  stopRecording(): { presents: PresentSample[]; inputs: InputSample[] };
  scrollBy(px: number): void;
  caretToLine(line: number, col: number): void;
  info(): { lines: number; bytes: number; revision: number; scrollTop: number; totalHeight: number };
  keyMetrics: typeof keyMetrics;
}
declare global { interface Window { __emakiPerf: PerfHook } }

export function installPerfHook(handle: EditorHandle, text: string): PerfHook {
  let presents: PresentSample[] = [], inputs: InputSample[] = [];
  let unsubscribe: (() => void)[] = [];
  const stop = () => { for (const off of unsubscribe) off(); unsubscribe = []; };
  const hook: PerfHook = {
    load: () => new Promise(resolve => {
      const start = performance.now();
      let revision: number | undefined;
      const off = handle.perf.onPresent(sample => {
        if (sample.revision === revision && sample.visibleComplete) { off(); resolve({ firstPaintMs: sample.frameEnd - start }); }
      });
      handle.setDoc(text);
      revision = handle.revision;
    }),
    startRecording: () => { stop(); presents = []; inputs = [];
      unsubscribe = [handle.perf.onPresent(sample => presents.push(sample)), handle.perf.onInput(sample => inputs.push(sample))]; },
    stopRecording: () => { stop(); return { presents, inputs }; },
    scrollBy: px => handle.scrollTo({ top: handle.scrollTop + px }),
    caretToLine: (line, col) => {
      const row = handle.state.doc.line(Math.max(1, Math.min(handle.state.doc.lines, Math.floor(line))));
      handle.dispatch({ selection: { anchor: row.from + Math.max(0, Math.min(row.length, Math.floor(col))) } });
      handle.focus();
    },
    info: () => ({ lines: handle.state.doc.lines, bytes: new TextEncoder().encode(handle.state.doc.toString()).length,
      revision: handle.revision, scrollTop: handle.scrollTop, totalHeight: handle.lineTop(handle.state.doc.lines) }),
    keyMetrics,
  };
  window.__emakiPerf = hook;
  return hook;
}
