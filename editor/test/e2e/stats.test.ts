// @vitest-environment node
import { describe, expect, it } from 'vitest';

interface StatsModule {
  percentile(values: number[], p: number): number | null;
  pairInputLatency(frames: number[][], keys: number[][]): { paired: Array<{ keyTime: number; frameTime: number; latencyMs: number }>; pairedKeys: number; nonEditingKeys: number; expiredKeys: number; unpairedKeys: number };
  countChecks(checks: Array<{ status?: string; pass?: boolean }>): { total: number; passed: number; failed: number };
  evaluate(summary: Record<string, unknown>, thresholds?: Record<string, number>): { pass: boolean; failures: string[] };
  renderEvidence(summary: Record<string, unknown>): string;
  attributeWorkloadOnsets(rows: Array<{ receivedMs:number; [key:string]:unknown }>, baselineKeys: Set<string>, clickMs:number): { workload:Array<unknown>; count:number; excludedBaseline:number; excludedPreClick:number };
  splitSinkOnsets(times:number[], ctxTime:number): { control:number; workload:number };
}
const spec: string = '../../test/e2e/stats.mjs';
const stats = (await import(/* @vite-ignore */ spec)) as StatsModule;

const passingMetrics = { inputLatencyMs:{p95:40,p99:80}, animationWorkMs:{p50:2,p95:6,p99:12}, textWorkMs:{p95:12}, frameIntervalMs:{p95:18,p99:40}, editKeyCount:500, editPairedKeyCount:100, editUnpairedKeys:0, audioRunning:true, onsetCount:1 };
describe('canvas evidence statistics', () => {
  it('uses nearest rank percentiles', () => expect(stats.percentile(Array.from({ length: 100 }, (_, i) => i + 1), 95)).toBe(95));
  it('fails an input p95 of 51 ms', () => expect(stats.evaluate({ metrics: { ...passingMetrics, inputLatencyMs: { p95: 51, p99:80 } } }).pass).toBe(false));
  it('pairs a post-dispatch revision with its next presentation frame', () => {
    const result = stats.pairInputLatency([[100, 1, 0, 3], [111, 1, 0, 4]], [[105, 4]]);
    expect(result.paired).toEqual([{ keyTime:105, keyRevision:4, frameTime:111, frameRevision:4, latencyMs:6 }]);
  });
  it('does not pair a non-editing key across a later input burst', () => {
    const result = stats.pairInputLatency([[100, 1, 0, 3], [5100, 1, 0, 4]], [[110, 3], [5001, 4]]);
    expect(result.paired.map((sample) => sample.keyTime)).toEqual([5001]);
    expect(result.nonEditingKeys).toBe(1);
  });
  it('counts keys older than the retained frame ring as expired', () => {
    const result = stats.pairInputLatency([[100, 1, 0, 3], [110, 1, 0, 4]], [[90, 3]]);
    expect(result).toMatchObject({ pairedKeys:0, expiredKeys:1 });
  });
  it('fails 499 editing keystrokes', () => {
    const result = stats.evaluate({ metrics:{ ...passingMetrics, editKeyCount:499 } });
    expect(result.failures).toContain('editing keystrokes=499, expected at least 500');
  });
  it('accepts 500 editing keystrokes with 100 paired samples', () => {
    expect(stats.evaluate({ metrics:{ ...passingMetrics, editKeyCount:500, editPairedKeyCount:100 } }).pass).toBe(true);
  });
  it('gates silent sink output and reports pre-sink levels', () => {
    const clean={installed:true,installedBeforeFirstConnect:true,directDestinationConnections:0,violations:[],post:{peak:0},pre:{peakDbfs:-10,rmsDbfs:-20,onsetCount:1}};
    expect(stats.evaluate({metrics:passingMetrics,controlSink:clean,workloadSink:clean,hushQuiet:true,workloadOnsetCount:1}).pass).toBe(true);
    expect(stats.evaluate({metrics:passingMetrics,controlSink:clean,workloadSink:{...clean,post:{peak:0.001}}}).failures).toContain('silent-sink: post-sink peak=0.001');
    expect(stats.evaluate({metrics:passingMetrics,controlSink:{...clean,directDestinationConnections:1}}).failures).toContain('silent-sink: direct destination connections=1');
  });
  it('attributes control-only rows before the click as no workload onset', () => {
    const rows=[{receivedMs:10},{receivedMs:20}];
    const result=stats.attributeWorkloadOnsets(rows,new Set(),30);
    expect(result).toMatchObject({count:0,excludedPreClick:2});
    expect(stats.evaluate({metrics:passingMetrics,controlSink:{pre:{onsetCount:1,peakDbfs:-10},installed:true,installedBeforeFirstConnect:true,directDestinationConnections:0,post:{peak:0},violations:[]},workloadOnsetCount:result.count}).failures).toContain('no playing onset telemetry; active audio/visual workload was not observed');
  });
  it('counts new onset rows received after the large-run click', () => {
    const result=stats.attributeWorkloadOnsets([{receivedMs:101,from:3,to:4}],new Set(),100);
    expect(result).toMatchObject({count:1,excludedBaseline:0,excludedPreClick:0});
  });
  it('excludes a post-hush baseline row even when received after the large-run click', () => {
    const baselineRow={receivedMs:101,from:1,to:2};const baseline=JSON.stringify(baselineRow);
    const result=stats.attributeWorkloadOnsets([baselineRow],new Set([baseline]),100);
    expect(result).toMatchObject({count:0,excludedBaseline:1,excludedPreClick:0});
  });
  it('splits sink onset timestamps at the large-run context time', () => {
    expect(stats.splitSinkOnsets([0.5,1.0,3.2,4.0],2.0)).toEqual({control:2,workload:2});
  });
  it('requires a paired editing sample and caps unpaired keys at 10 percent', () => {
    expect(stats.evaluate({ metrics:{ ...passingMetrics, editPairedKeyCount:0 } }).failures).toContain('paired editing samples=0, expected at least 1');
    expect(stats.evaluate({ metrics:{ ...passingMetrics, editUnpairedKeys:51 } }).failures).toContain('unpaired editing keys=51/500, expected at most 10%');
  });
  it('fails when audio is running but the active workload has no onsets', () => {
    const result = stats.evaluate({ metrics:{ ...passingMetrics, onsetCount:0 } });
    expect(result.pass).toBe(false);
    expect(result.failures).toContain('no playing onset telemetry; active audio/visual workload was not observed');
  });
  it('excludes limitations from behavioral pass and failure counts', () => {
    expect(stats.countChecks([{ pass:true }, { status:'limitation' }, { pass:false }])).toEqual({ total:2, passed:1, failed:1 });
  });
  it('reports estimated sync without gating it', () => expect(stats.evaluate({ metrics: { ...passingMetrics, syncProvenance: 'estimate', syncAbsMs: { p95: 999 } } }).pass).toBe(true));
  it('does not gate unavailable sync', () => expect(stats.evaluate({ metrics: { ...passingMetrics, syncProvenance: 'unavailable' } }).pass).toBe(true));
  it('renders all metric rows and an ASCII run id', () => {
    const text = stats.renderEvidence({ runId: 'run-001', browsers: [{ name: 'Chromium', metrics: { editKeyCount:499, editPairedKeyCount:100, controlOnsetCount:3, controlStartMethod:'toolbar-click', onsetAttribution:'control sounded' }, checks:[{ id:'real-failure', pass:false }, { id:'synthetic-ime', status:'limitation', pass:null }], measurement:{ failures:['no playing onset telemetry', 'Run shortcut timeout\nstack trace'] }, behavior: { passed: 4, total: 4 } }] });
    expect(text).toContain('run-001');
    for (const row of ['Input latency p95', 'Editing keystrokes / paired', 'Audio control / run start', 'Silent sink post-sink peak', 'Direct destination connections', 'Pre-sink peak (dBFS)', 'Pre-sink RMS (dBFS)', 'Animation frame work p95', 'Frame interval p95', 'A/V model absolute error', 'Resource ledger peak', 'JS heap growth', 'Beat drift', 'Replayed flashes']) expect(text).toContain(row);
    expect(text).toContain('Chromium:real-failure');
    expect(text).not.toContain('Chromium:synthetic-ime');
    expect(text).toContain('Measurement failures: Chromium:no playing onset telemetry; Chromium:Run shortcut timeout.');
    expect(text).not.toContain('stack trace');
    expect(/[^\x00-\x7F]/.test(text)).toBe(false);
  });
});
