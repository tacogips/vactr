// @vitest-environment node
import { describe, expect, it } from 'vitest';

interface CompareStats {
  RENDERERS: readonly string[]; BROWSERS: readonly string[]; DEFAULT_LINES: readonly number[]; DEFAULT_RUNS: number;
  MAX_RUN_JSONL_BYTES: number; COMPARISON_BEGIN: string; COMPARISON_END: string; REPORT_SKELETON: string;
  METRIC_ROWS: Array<{ key: string; label: string; unit: string; lowerIsBetter: true }>;
  sliceWorkload(workload: Record<string, any>, lines: number): Record<string, any>;
  runOrder(runs: number, renderers: string[]): Array<{ run: number; renderer: string; position: number }>;
  runKey(value: { lines: number; browser: string; renderer: string; run: number }): string;
  inputP50(samples: Array<Record<string, any>>): number | null;
  extractRunMetrics(measured: Record<string, any>, extras?: Record<string, any>): Record<string, any>;
  aggregateCell(records: Array<Record<string, any>>, key: string): Record<string, any>;
  winner(canvas: Record<string, any>, dom: Record<string, any>): string;
  downsampleSamples(samples: Array<Record<string, any>>, stride?: number): { rows: Array<Record<string, any>>; stride: number };
  contended(load: number, cpus: number): boolean;
  checkLock(text: string): { ok: boolean; owner: string | null };
  renderComparison(value: Record<string, any>): string;
  replaceMarked(source: string, section: string): string;
  buildComparison(records: Array<Record<string, any>>, environment: Record<string, any>, attempts: unknown[]): Record<string, any>;
}
interface Fixture { createLargeDocument(): Record<string, any> }
const spec: string = '../../test/e2e/compare-stats.mjs';
const fixtureSpec: string = '../../test/e2e/fixtures/large-doc.mjs';
const stats = (await import(/* @vite-ignore */ spec)) as CompareStats;
const fixture = (await import(/* @vite-ignore */ fixtureSpec)) as Fixture;

describe('renderer comparison helpers', () => {
  it('exports the pinned constants and slices a 1,000-line workload', () => {
    expect(stats.RENDERERS).toEqual(['canvas', 'dom']); expect(Object.isFrozen(stats.RENDERERS)).toBe(true);
    expect(stats.BROWSERS).toEqual(['chromium', 'webkit']); expect(Object.isFrozen(stats.BROWSERS)).toBe(true);
    expect(stats.DEFAULT_LINES).toEqual([1000, 20000]); expect(stats.DEFAULT_RUNS).toBe(3); expect(stats.MAX_RUN_JSONL_BYTES).toBe(870000);
    const source = fixture.createLargeDocument(); const sliced = stats.sliceWorkload(source, 1000);
    expect(sliced.lines).toBe(1000); expect(sliced.text.split('\n')).toHaveLength(1000); expect(sliced.text.startsWith(source.head)).toBe(true);
    expect(sliced.controlText).toBe(source.controlText); expect(sliced.bytes).toBe(new TextEncoder().encode(sliced.text).length); expect(sliced.sourceLines).toBe(20000);
  });
  it('rejects an invalid workload slice size', () => {
    const source = fixture.createLargeDocument();
    expect(() => stats.sliceWorkload(source, 7)).toThrow(RangeError); expect(() => stats.sliceWorkload(source, 20001)).toThrow(RangeError); expect(() => stats.sliceWorkload(source, 1.5)).toThrow(RangeError);
  });
  it('alternates the renderer order and creates stable run keys', () => {
    expect(stats.runOrder(3, ['canvas', 'dom'])).toEqual([
      { run: 1, renderer: 'canvas', position: 0 }, { run: 1, renderer: 'dom', position: 1 },
      { run: 2, renderer: 'dom', position: 0 }, { run: 2, renderer: 'canvas', position: 1 },
      { run: 3, renderer: 'canvas', position: 0 }, { run: 3, renderer: 'dom', position: 1 },
    ]);
    expect(stats.runKey({ lines: 1000, browser: 'webkit', renderer: 'dom', run: 2 })).toBe('1000-webkit-dom-r2');
  });
  it('computes input p50 from input-pair rows and returns null when absent', () => {
    expect(stats.inputP50([{ phase: 'input-pair', latencyMs: 10 }, { phase: 'frame', latencyMs: 500 }, { phase: 'input-pair', latencyMs: 20 }, { phase: 'input-pair', latencyMs: 30 }])).toBe(20);
    expect(stats.inputP50([])).toBeNull();
  });
  it('aggregates finite values and applies the variance-aware winner rule', () => {
    const aggregate = stats.aggregateCell([3, 1, null, 2].map((value) => ({ metrics: { x: value } })), 'x');
    expect(aggregate).toEqual({ values: [3, 1, 2], median: 2, min: 1, max: 3, missing: 1 });
    expect(stats.winner({ median: 10, min: 9, max: 11 }, { median: 20, min: 18, max: 21 })).toBe('canvas');
    expect(stats.winner({ median: 10, min: 8.5, max: 11.5 }, { median: 11, min: 10.5, max: 11.5 })).toBe('no clear difference');
    expect(stats.winner({ median: null, min: null, max: null }, { median: 2, min: 2, max: 2 })).toBe('unavailable');
  });
  it('downsamples only bulky phases and retains long phase spans and all other samples', () => {
    const samples = [...Array.from({ length: 100 }, (_, index) => ({ phase: 'frame', index })),
      { phase: 'phase', row: [0, 0, 10] }, { phase: 'phase', row: [0, 0, 50] }, { phase: 'phase', row: [0, 0, 70] },
      ...Array.from({ length: 5 }, (_, index) => ({ phase: 'input-pair', index }))];
    const result = stats.downsampleSamples(samples);
    expect(result.rows.filter((row) => row.phase === 'frame').map((row) => row.index)).toEqual([0, 32, 64, 96]);
    expect(result.rows.filter((row) => row.phase === 'phase')).toHaveLength(2); expect(result.rows.filter((row) => row.phase === 'input-pair')).toHaveLength(5); expect(result.stride).toBe(32);
  });
  it('preserves unavailable values, reports a comparison cell and renders its tables', () => {
    const extracted = stats.extractRunMetrics({ metrics: { heapGrowthBytes: null }, samples: [] });
    expect(extracted.heapGrowthBytes).toBeNull(); expect(extracted.domNodesPeak).toBeNull(); expect(extracted.domNodesFinal).toBeNull();
    const comparison = stats.buildComparison([{ lines: 1000, browser: 'chromium', renderer: 'canvas', run: 1, position: 0, metrics: extracted, gate: { pass: false, failures: [] } }], { runId: 'test' }, []);
    const report = stats.renderComparison(comparison);
    expect(report).toContain('unavailable'); expect(report).toContain('Canvas median'); expect(report).toContain('DOM median');
    const emptyReport = stats.renderComparison({ cells: [] }); expect(emptyReport).toContain('1000 lines · chromium'); expect(emptyReport).toContain('20000 lines · webkit'); expect(emptyReport).toContain('unavailable');
    expect(stats.METRIC_ROWS.every((row) => row.lowerIsBetter)).toBe(true);
  });
  it('replaces only the marked region and rejects missing markers', () => {
    expect(() => stats.replaceMarked('outside only', 'replacement')).toThrow();
    const source = `before\n${stats.COMPARISON_BEGIN}\nold\n${stats.COMPARISON_END}\nafter`;
    expect(stats.replaceMarked(source, 'new section')).toBe(`before\n${stats.COMPARISON_BEGIN}\nnew section\n${stats.COMPARISON_END}\nafter`);
  });
  it('checks contention, lock ownership and the report skeleton', () => {
    expect(stats.contended(9, 8)).toBe(true); expect(stats.contended(8, 8)).toBe(false);
    expect(stats.checkLock('')).toEqual({ ok: false, owner: null }); expect(stats.checkLock('owner-x\n')).toEqual({ ok: true, owner: 'owner-x' });
    expect(stats.REPORT_SKELETON).toContain(stats.COMPARISON_BEGIN); expect(stats.REPORT_SKELETON).toContain(stats.COMPARISON_END);
    expect(stats.REPORT_SKELETON).toContain('## Analysis'); expect(stats.REPORT_SKELETON).toContain('## Limitations'); expect(stats.REPORT_SKELETON).toContain('## Observations for the canvas tuning proposal');
  });
});
