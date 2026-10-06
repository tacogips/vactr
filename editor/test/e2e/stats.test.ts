// @vitest-environment node
import { describe, expect, it } from 'vitest';

interface StatsModule {
  percentile(values: number[], p: number): number | null;
  pairInputLatency(frames: number[][], keys: number[][]): { paired: Array<{ keyTime: number; frameTime: number; latencyMs: number }>; pairedKeys: number; nonEditingKeys: number; expiredKeys: number; unpairedKeys: number };
  countChecks(checks: Array<{ status?: string; pass?: boolean }>): { total: number; passed: number; failed: number };
  evaluate(summary: Record<string, unknown>, thresholds?: Record<string, number>): { pass: boolean; failures: string[]; limitations: string[] };
  renderEvidence(summary: Record<string, unknown>): string;
  attributeWorkloadOnsets(rows: Array<{ receivedMs:number; [key:string]:unknown }>, baselineKeys: Set<string>, clickMs:number): { workload:Array<unknown>; count:number; excludedBaseline:number; excludedPreClick:number };
  splitSinkOnsets(times:number[], ctxTime:number): { control:number; workload:number };
  syncWindow(onsets:Array<{time:number;end:number}>,presented:Array<{audibleTime:number}>): {windowStart:number|null;windowEnd:number|null;empty:boolean};
  attributeSync(onsets:Array<{time:number;end:number;from:number;to:number;epoch:string|null}>,presented:Array<{audibleTime:number;activeKey:string;epoch:string|null;frameMs:number;targetMs:number}>,options?:{earlyToleranceS?:number}): {sync:number[];earlyFlashCount:number;replayedFlashCount:number;framePairs:number;windowOnsets:number;excludedFrames:number;windowStart:number|null;windowEnd:number|null};
  lateActiveMismatches(onsets:Array<{time:number;end:number;from:number;to:number;epoch:string|null}>,presented:Array<{audibleTime:number;activeKey:string;epoch:string|null;frameMs:number}>,stalls:number[]): number;
  rankSelfTime(profile:Record<string,unknown>,top?:number):Array<{functionName:string;url:string;line:number;selfMs:number;share:number}>;
  classifyWithControl(metricKey:string,path:string,productValue:number,controlValue:number,threshold:number):'pass'|'fail'|'limitation';
  phaseSummary(rows:number[][]):Record<string,{spans:number;p50:number|null;p95:number|null;p99:number|null;totalMs:number}>;
}
const spec: string = '../../test/e2e/stats.mjs';
const stats = (await import(/* @vite-ignore */ spec)) as StatsModule;

const passingMetrics = { inputLatencyMs:{p95:40,p99:80}, animationWorkMs:{p50:2,p95:6,p99:12}, textWorkMs:{p95:12}, frameIntervalMs:{p95:18,p99:40}, editKeyCount:500, editPairedKeyCount:100, editUnpairedKeys:0, audioRunning:true, onsetCount:1 };
describe('canvas evidence statistics', () => {
  it('uses nearest rank percentiles', () => expect(stats.percentile(Array.from({ length: 100 }, (_, i) => i + 1), 95)).toBe(95));
  it('summarizes exclusive phase rows and leaves empty phases unmeasured', () => {
    const rows=[[0,0,1,2,0,0,0,0,0],[5,10,3,0,0,0,4,0,0],[5,20,5,0,0,0,0,0,0]];
    const result=stats.phaseSummary(rows);
    expect(result.input).toMatchObject({spans:3,p50:3,p95:5,p99:5,totalMs:9});
    expect(result.caret).toMatchObject({spans:1,p50:2,p95:2,p99:2,totalMs:2});
    expect(result.syntax).toMatchObject({spans:0,p50:null,p95:null,p99:null,totalMs:0});
    expect(result.upload).toMatchObject({spans:1,p50:4,p95:4,p99:4,totalMs:4});
    expect(Object.keys(result)).toEqual(['input','caret','shaping','syntax','upload','frame','tick']);
  });
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
  it('attributes repeated ranges by onset occurrence and exact window', () => {
    const onsets=[0,0.25,0.5,0.75,1].map((time)=>({time,end:time+0.2,from:1,to:2,epoch:'e'}));
    const presented=Array.from({length:61},(_,i)=>{const audibleTime=i/60;const active=onsets.filter((o)=>o.time<=audibleTime&&audibleTime<o.end).map((o)=>`${o.from}-${o.to}`);return {audibleTime,activeKey:active.join(','),epoch:'e',frameMs:audibleTime*1000,targetMs:audibleTime*1000};});
    const result=stats.attributeSync(onsets,presented);
    expect(result.earlyFlashCount).toBe(0);expect(result.replayedFlashCount).toBe(0);
    expect(result.earlyFlashCount+result.replayedFlashCount).toBeLessThanOrEqual(result.framePairs);
    expect(result.sync.length).toBeGreaterThan(0);expect(result.sync.every((sample)=>Math.abs(sample)<=16.8)).toBe(true);
  });
  it('counts an early frame, an expired frame, and an epoch mismatch once each', () => {
    const onsets=Array.from({length:9},(_,i)=>({time:i*0.25,end:i*0.25+0.2,from:1,to:2,epoch:'e'}));onsets.push({time:1,end:1.4,from:5,to:7,epoch:'e'});
    const rows=[
      {audibleTime:0.99,activeKey:'5-7',epoch:'e',frameMs:990,targetMs:990},
      {audibleTime:1.1,activeKey:'5-7',epoch:'other',frameMs:1100,targetMs:1100},
      {audibleTime:1.2,activeKey:'981-98,981-983',epoch:'e',frameMs:1200,targetMs:1200},
      {audibleTime:1.42,activeKey:'5-7',epoch:'e',frameMs:1420,targetMs:1420},
    ];
    const result=stats.attributeSync(onsets,rows);
    expect(result.earlyFlashCount).toBe(4);expect(result.replayedFlashCount).toBe(1);
    expect(result.framePairs).toBe(5);
  });
  it('reports disjoint and evicted windows without false flash counts', () => {
    const onsets=[{time:0,end:1,from:5,to:7,epoch:'e'}];
    const presented=Array.from({length:3},(_,i)=>({audibleTime:5+i/2,activeKey:'5-7',epoch:'e',frameMs:5000+i*500,targetMs:5000+i*500}));
    expect(stats.attributeSync(onsets,presented)).toMatchObject({sync:[],earlyFlashCount:0,replayedFlashCount:0,framePairs:0,excludedFrames:3});
    const retained=Array.from({length:11},(_,i)=>({time:30+i*0.5,end:30.4+i*0.5,from:5,to:7,epoch:'e'}));
    const oldRows=Array.from({length:11},(_,i)=>({audibleTime:25+i*0.5,activeKey:'5-7',epoch:'e',frameMs:25000+i*500,targetMs:25000+i*500}));
    const evicted=stats.attributeSync(retained,oldRows);
    expect(evicted.windowStart).toBe(30.4);expect(evicted.excludedFrames).toBeGreaterThan(0);expect(evicted.earlyFlashCount).toBe(0);
  });
  it('counts in-window ranges with no matching onset as early and excludes range substring matches', () => {
    const onsets=Array.from({length:9},(_,i)=>({time:i*0.5,end:i*0.5+0.2,from:1,to:2,epoch:'e'}));
    const presented=[{audibleTime:1,activeKey:'981-98,9-9',epoch:'e',frameMs:1000,targetMs:1000}];
    expect(stats.attributeSync(onsets,presented).earlyFlashCount).toBe(2);
  });
  it('uses the in-window presented set for late active mismatch checks', () => {
    const onsets=Array.from({length:9},(_,i)=>({time:i*0.5,end:i*0.5+0.2,from:1,to:2,epoch:'e'}));onsets.push({time:3.25,end:3.8,from:1,to:2,epoch:'e'});
    const rows=[{audibleTime:3.4,activeKey:'1-2',epoch:'e',frameMs:3400},{audibleTime:3.5,activeKey:'1-2,8-9',epoch:'e',frameMs:3500}];
    expect(stats.lateActiveMismatches(onsets,rows,[3300])).toBe(0);
    expect(stats.lateActiveMismatches(onsets,rows,[3500])).toBe(1);
  });
  it('ranks CPU self time and classifies WebKit headless limits conservatively', () => {
    const profile={nodes:[{id:1,callFrame:{functionName:'slow',url:'app.js',lineNumber:4}},{id:2,callFrame:{functionName:'fast',url:'app.js',lineNumber:8}}],samples:[1,2,1],timeDeltas:[6000,2000,2000]};
    expect(stats.rankSelfTime(profile)).toEqual([{functionName:'slow',url:'app.js',line:4,selfMs:8,share:0.8},{functionName:'fast',url:'app.js',line:8,selfMs:2,share:0.2}]);
    expect(stats.classifyWithControl('frameIntervalMs','p95',30,10,20)).toBe('fail');
    expect(stats.classifyWithControl('frameIntervalMs','p95',30,30,20)).toBe('limitation');
    expect(stats.classifyWithControl('frameIntervalMs','p95',10,30,20)).toBe('pass');
    expect(stats.classifyWithControl('inputLatencyMs','p95',30,Number.NaN,20)).toBe('fail');
  });
  it('uses control limitations only for headless WebKit input, frame interval and key count', () => {
    const base={...passingMetrics,editKeyCount:400,inputLatencyMs:{p95:60,p99:120},frameIntervalMs:{p95:30,p99:60}};
    const control={mode:'headless',editKeyCount:300,inputLatencyMs:{p95:60,p99:120},frameIntervalMs:{p95:30,p99:60}};
    const webkit=stats.evaluate({browser:'webkit',metrics:base,control});
    expect(webkit.pass).toBe(true);expect(webkit.failures).toEqual([]);expect(webkit.limitations).toHaveLength(5);
    const chromium=stats.evaluate({browser:'chromium',metrics:base,control});
    expect(chromium.pass).toBe(false);expect(chromium.failures.some((failure)=>failure.includes('inputLatencyMs.p95'))).toBe(true);
    const missing=stats.evaluate({browser:'webkit',metrics:base,control:{...control,inputLatencyMs:{p95:Number.NaN,p99:Number.NaN}}});
    expect(missing.pass).toBe(false);expect(missing.failures.some((failure)=>failure.includes('inputLatencyMs.p95'))).toBe(true);
    const nonLimited=stats.evaluate({browser:'webkit',metrics:{...base,animationWorkMs:{p50:30,p95:60,p99:70},syncProvenance:'measured',syncAbsMs:{p95:70,p99:80}},control});
    expect(nonLimited.failures.some((failure)=>failure.includes('animationWorkMs.p50'))).toBe(true);
    expect(nonLimited.failures.some((failure)=>failure.includes('syncAbsMs.p95'))).toBe(true);
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
  it('renders wasm provenance and reports when it is absent', () => {
    const wasm = { profile:'release', bytes:6451635, sha256:'abc123' };
    expect(stats.renderEvidence({ runId:'run-001', wasm, browsers:[] }))
      .toContain('WASM: release; 6451635 bytes; SHA-256 abc123.');
    expect(stats.renderEvidence({ runId:'run-001', environment:{ wasm }, browsers:[] }))
      .toContain('WASM: release; 6451635 bytes; SHA-256 abc123.');
    expect(stats.renderEvidence({ runId:'run-001', browsers:[] })).toContain('WASM: not recorded.');
  });
});
