// @vitest-environment node
import { describe, expect, it } from 'vitest';

interface StatsModule {
  THRESHOLDS: Readonly<Record<string, number>>;
  TARGETS: Readonly<{ textWorkP95Ms: number; animationWorkP50Ms: number }>;
  percentile(values: number[], p: number): number | null;
  beatResidualMs(row:{epoch:string|null;beatCycle:number;audibleTime:number},transport:{running:boolean;epoch:string|null;cycle:[number,number];sample_time:number;bpm:number;beats_per_cycle:number}):number|null;
  classifyStallSamples(samples:Array<{time:number;frameIndex:number;value:number;onset:{time:number;end:number;from:number;to:number;epoch:string|null};frame:{frameMs:number;targetMs:number;audibleTime:number;epoch:string|null;activeKey:string}}>,presented:Array<{frameMs:number;audibleTime:number;epoch:string|null;activeKey:string;targetMs:number;beatCycle?:number}>,windows:Array<{index:number;startMs:number;endMs:number;beatResidualMs?:number|null}>,onsets?:Array<{time:number;end:number;from:number;to:number;epoch:string|null}>):{stall:Array<{frameIndex:number;stallClass:string;stallConditions:{a:boolean;b:boolean};value:number}>;nonStall:Array<{frameIndex:number;stallClass:string;stallConditions:{a:boolean;b:boolean};value:number}>;windows:Array<{index:number;startMs:number;endMs:number;firstFrameMs:number|null;firstFrameLagMs:number|null;samples:number;activeSetMatch:boolean|null;replayed:boolean|null;early:number;beatResidualMs:number|null;audited:boolean}>;counts:{a:number;b:number;both:number}};
  pairInputLatency(frames: number[][], keys: number[][]): { paired: Array<{ keyTime: number; frameTime: number; latencyMs: number }>; pairedKeys: number; nonEditingKeys: number; expiredKeys: number; unpairedKeys: number };
  countChecks(checks: Array<{ status?: string; pass?: boolean }>): { total: number; passed: number; failed: number };
  evaluate(summary: Record<string, unknown>, thresholds?: Record<string, number>): { pass: boolean; failures: string[]; limitations: string[]; targets: { textWorkP95Met: boolean | null; animationWorkP50Met: boolean | null } };
  renderEvidence(summary: Record<string, unknown>): string;
  attributeWorkloadOnsets(rows: Array<{ receivedMs:number; [key:string]:unknown }>, baselineKeys: Set<string>, clickMs:number): { workload:Array<unknown>; count:number; excludedBaseline:number; excludedPreClick:number };
  splitSinkOnsets(times:number[], ctxTime:number): { control:number; workload:number };
  syncWindow(onsets:Array<{time:number;end:number}>,presented:Array<{audibleTime:number}>): {windowStart:number|null;windowEnd:number|null;empty:boolean};
  attributeSync(onsets:Array<{time:number;end:number;from:number;to:number;epoch:string|null;receivedMs?:number}>,presented:Array<{audibleTime:number;activeKey:string;epoch:string|null;frameMs:number;targetMs:number}>,options?:{earlyToleranceS?:number;nominalMs?:number}): {sync:number[];samples:Array<{time:number;epoch:string|null;frameIndex:number;value:number;droppedFrames:number;onset:{time:number;end:number;from:number;to:number;epoch:string|null;receivedMs?:number};frame:{frameMs:number;targetMs:number;audibleTime:number;epoch:string|null;activeKey:string};previousFrameMs:number|null;nextFrameMs:number}>;duplicateSamples:number;droppedFrameSamples:number;droppedFrames:number;earlyFlashCount:number;replayedFlashCount:number;framePairs:number;windowOnsets:number;excludedFrames:number;windowStart:number|null;windowEnd:number|null};
  lateActiveMismatches(onsets:Array<{time:number;end:number;from:number;to:number;epoch:string|null}>,presented:Array<{audibleTime:number;activeKey:string;epoch:string|null;frameMs:number}>,stalls:number[]): number;
  lateActiveMismatchDetails(onsets:Array<{time:number;end:number;from:number;to:number;epoch:string|null;receivedMs?:number}>,presented:Array<{audibleTime:number;activeKey:string;epoch:string|null;frameMs:number;executionMs?:number}>,stalls:number[]):Array<{stallFrameMs:number;frameMs:number;executionMs:number;audibleTime:number;epoch:string|null;activeKey:string;expectedRanges:string[];actualRanges:string[];missingRanges:string[];extraRanges:string[];eligibleOnsets:Array<{time:number;end:number;from:number;to:number;epoch:string|null;receivedMs?:number}>;overlappingOnsets:Array<{time:number;end:number;from:number;to:number;epoch:string|null;receivedMs?:number;eligible:boolean}>;mismatch:boolean}>;
  rankSelfTime(profile:Record<string,unknown>,top?:number):Array<{functionName:string;url:string;line:number;selfMs:number;share:number}>;
  classifyWithControl(metricKey:string,path:string,productValue:number,controlValue:number,threshold:number):'pass'|'fail'|'limitation';
  phaseSummary(rows:number[][]):Record<string,{spans:number;p50:number|null;p95:number|null;p99:number|null;totalMs:number}>;
  tickStarvationEvidence(rows:number[][], options?:{lookaheadMs?:number;longSpanMs?:number}):{tickStartCount:number;tickStartGapsOverLookahead:number;maxTickStartGapMs:number;mainThreadSpansOverThreshold:number;starvationOverlaps:number;overlaps:Array<{startMs:number;endMs:number;durationMs:number;span:{phase:number;startMs:number;durationMs:number}}>};
}
const spec: string = '../../test/e2e/stats.mjs';
const stats = (await import(/* @vite-ignore */ spec)) as StatsModule;

const passingMetrics = { inputLatencyMs:{p95:40,p99:80}, animationWorkMs:{p50:2,p95:6,p99:12}, textWorkMs:{p95:6}, frameIntervalMs:{p95:18,p99:40}, editKeyCount:500, editPairedKeyCount:100, editUnpairedKeys:0, audioRunning:true, beatDriftMs:0, onsetCount:1 };
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
  it('folds 64 voices sharing an onset and first presented frame into one sync sample', () => {
    const voices=Array.from({length:64},(_,i)=>({time:0.2,end:0.3,from:i*2,to:i*2+1,epoch:'e'}));
    const active=voices.map((voice)=>`${voice.from}-${voice.to}`).join(',');
    const rows=[200,216.7].map((frameMs)=>({audibleTime:frameMs/1000,activeKey:active,epoch:'e',frameMs,targetMs:frameMs}));
    const result=stats.attributeSync([{time:0,end:0.1,from:1000,to:1001,epoch:'e'},...voices],rows,{nominalMs:16.7});
    expect(result.sync).toHaveLength(1);
    expect(result.samples).toHaveLength(1);
    expect(result.duplicateSamples).toBe(63);
    expect(result.samples[0]).toMatchObject({time:0.2,epoch:'e',frameIndex:0,droppedFrames:0,
      onset:{time:0.2,end:0.3,from:0,to:1,epoch:'e'},
      frame:{frameMs:200,targetMs:200,audibleTime:0.2,activeKey:active},nextFrameMs:216.7});
  });
  it('keeps distinct first presented frames and distinct onset times as separate samples', () => {
    const voices=Array.from({length:64},(_,i)=>({time:0.2,end:0.3,from:i*2,to:i*2+1,epoch:'e'}));
    const first=voices.slice(0,32).map((voice)=>`${voice.from}-${voice.to}`).join(',');
    const second=voices.slice(32).map((voice)=>`${voice.from}-${voice.to}`).join(',');
    const rows=[
      {audibleTime:0.2,activeKey:first,epoch:'e',frameMs:200,targetMs:200},
      {audibleTime:0.2167,activeKey:`${first},${second}`,epoch:'e',frameMs:216.7,targetMs:216.7},
      {audibleTime:0.2334,activeKey:`${first},${second}`,epoch:'e',frameMs:233.4,targetMs:233.4},
    ];
    const result=stats.attributeSync([{time:0,end:0.1,from:1000,to:1001,epoch:'e'},...voices],rows,{nominalMs:16.7});
    expect(result.sync).toHaveLength(2);
    expect(result.samples.map((sample)=>sample.frameIndex)).toEqual([0,1]);
    expect(result.duplicateSamples).toBe(62);
    const distinctTimes=stats.attributeSync([
      {time:0,end:0.1,from:1000,to:1001,epoch:'e'},
      {time:0.2,end:0.3,from:1,to:2,epoch:'e'},
      {time:0.21,end:0.3,from:3,to:4,epoch:'e'},
    ],[
      {audibleTime:0.2,activeKey:'1-2,3-4',epoch:'e',frameMs:200,targetMs:200},
      {audibleTime:0.22,activeKey:'1-2,3-4',epoch:'e',frameMs:220,targetMs:220},
      {audibleTime:0.24,activeKey:'1-2,3-4',epoch:'e',frameMs:240,targetMs:240},
    ],{nominalMs:20});
    expect(distinctTimes.sync).toHaveLength(2);
  });
  it('counts dropped frames per sample using the override or median nominal interval', () => {
    const onset=[{time:0,end:0.1,from:100,to:101,epoch:'e'},{time:0.2,end:0.3,from:1,to:2,epoch:'e'}];
    const rows=[
      {audibleTime:0.2,activeKey:'1-2',epoch:'e',frameMs:200,targetMs:200},
      {audibleTime:0.25,activeKey:'1-2',epoch:'e',frameMs:250,targetMs:250},
    ];
    const dropped=stats.attributeSync(onset,rows,{nominalMs:16.7});
    expect(dropped).toMatchObject({droppedFrames:2,droppedFrameSamples:1});
    expect(dropped.samples[0].droppedFrames).toBe(2);
    const normal=stats.attributeSync(onset,[rows[0],{...rows[1],frameMs:216.7}],{nominalMs:16.7});
    expect(normal).toMatchObject({droppedFrames:0,droppedFrameSamples:0});
    const medianRows=[
      {audibleTime:0.2,activeKey:'',epoch:'e',frameMs:200,targetMs:200},
      {audibleTime:0.216,activeKey:'',epoch:'e',frameMs:216,targetMs:216},
      {audibleTime:0.217,activeKey:'',epoch:'e',frameMs:233,targetMs:233},
      {audibleTime:0.218,activeKey:'1-2',epoch:'e',frameMs:249,targetMs:249},
      {audibleTime:0.299,activeKey:'1-2',epoch:'e',frameMs:299,targetMs:299},
    ];
    expect(stats.attributeSync(onset,medianRows).samples[0].droppedFrames).toBe(2);
    expect(stats.attributeSync(onset,[]).samples).toEqual([]);
  });
  it('classifies sync samples by the recorded page-time interval (condition a)', () => {
    const onset={time:0.2,end:0.4,from:1,to:2,epoch:'e'};
    const sample={time:0.2,frameIndex:0,value:300,onset,frame:{frameMs:1000,targetMs:1000,audibleTime:0.2,epoch:'e',activeKey:'1-2'}};
    const result=stats.classifyStallSamples([sample],[sample.frame],[{index:0,startMs:1000,endMs:1250}]);
    expect(result.stall[0]).toMatchObject({stallClass:'stall',stallConditions:{a:true,b:true}});
    expect(result.nonStall).toEqual([]);
  });
  it('classifies a last pre-stall frame with F as proxy by condition b', () => {
    const onset={time:0.19,end:0.4,from:1,to:2,epoch:'e'};
    const sample={time:0.19,frameIndex:0,value:20,onset,frame:{frameMs:990,targetMs:990,audibleTime:0.19,epoch:'e',activeKey:'1-2'}};
    const rows=[sample.frame,{frameMs:1000,targetMs:1000,audibleTime:0.2,epoch:'e',activeKey:'1-2'}];
    const result=stats.classifyStallSamples([sample],rows,[{index:0,startMs:1000,endMs:1250}]);
    expect(result.stall[0]).toMatchObject({stallClass:'stall',stallConditions:{a:false,b:true}});
  });
  it('keeps F for a frame timestamp inside the explicit window and leaves the second post-F frame non-stall', () => {
    const first={time:0.25,end:0.5,from:1,to:2,epoch:'e'};
    const rows=[0,1,2].map((i)=>({frameMs:990+i*10,targetMs:990+i*10,audibleTime:0.25+i*0.01,epoch:'e',activeKey:'1-2'}));
    const samples=[
      {time:0.26,frameIndex:1,value:1,onset:first,frame:rows[1]},
      {time:0.53,frameIndex:2,value:2,onset:{...first,time:0.53},frame:rows[2]},
    ];
    const result=stats.classifyStallSamples(samples,rows,[{index:0,startMs:1000,endMs:1250}]);
    expect(result.windows[0].firstFrameMs).toBe(1000);
    expect(result.stall.map((row)=>row.frameIndex)).toEqual([1]);
    expect(result.nonStall.map((row)=>row.frameIndex)).toEqual([2]);
  });
  it('classifies by timing rather than latency value and gates only non-stall sync', () => {
    const onset={time:0.2,end:0.4,from:1,to:2,epoch:'e'};
    const sample={time:0.2,frameIndex:0,value:300,onset,frame:{frameMs:1000,targetMs:1000,audibleTime:0.2,epoch:'e',activeKey:'1-2'}};
    const outside=stats.classifyStallSamples([sample],[sample.frame],[]);
    expect(outside.nonStall[0].stallClass).toBe('non-stall');
    expect(stats.evaluate({metrics:{...passingMetrics,syncProvenance:'measured',syncAbsMs:{p95:300,p99:300},stallWindowSync:{earlyCount:0},stallWindowsInjected:0,stallWindows:[]}}).failures).toContain('syncAbsMs.p99=300 exceeds 50');
    const inside=stats.classifyStallSamples([sample],[sample.frame],[{index:0,startMs:1000,endMs:1250}]);
    expect(inside.stall[0].stallClass).toBe('stall');
    const recovery={...inside.windows[0],audited:true,beatResidualMs:0};
    const gated=stats.evaluate({metrics:{...passingMetrics,syncProvenance:'measured',syncAbsMs:{p95:0,p99:0},stallWindowSync:{earlyCount:0},stallWindowsInjected:1,stallWindows:[recovery]}});
    expect(gated.pass).toBe(true);
  });
  it('gates stall-window early flashes, missing windows, short windows, F residual and unavailable beat drift', () => {
    const base={...passingMetrics,syncProvenance:'measured',syncAbsMs:{p95:0,p99:0},stallWindowSync:{earlyCount:1},stallWindowsInjected:1,stallWindows:[{startMs:0,endMs:249,audited:true,beatResidualMs:1.01}]};
    const failed=stats.evaluate({metrics:base});
    expect(failed.failures).toContain('stall-window early flashes=1');
    expect(failed.failures).toContain('stall window shorter than 250 ms');
    expect(failed.failures).toContain('stall-frame beat residual exceeded 1 ms');
    expect(stats.evaluate({metrics:{...base,stallWindowsInjected:2}}).failures).toContain('stall windows injected=2, recorded=1');
    expect(stats.evaluate({metrics:{...base,stallWindowSync:{earlyCount:0},stallWindows:[{startMs:0,endMs:250,audited:true,beatResidualMs:1}]}}).pass).toBe(true);
    expect(stats.evaluate({metrics:{...base,beatDriftMs:null}}).failures).toContain('beat drift unavailable');
  });
  it('rejects a -3 ms stall-window sync sample as early', () => {
    const onset={time:0.2,end:0.4,from:1,to:2,epoch:'e'};
    const sample={time:0.2,frameIndex:0,value:-3,onset,frame:{frameMs:1000,targetMs:1000,audibleTime:0.2,epoch:'e',activeKey:'1-2'}};
    const classified=stats.classifyStallSamples([sample],[sample.frame],[{index:0,startMs:1000,endMs:1250}]);
    const result=stats.evaluate({metrics:{...passingMetrics,syncProvenance:'measured',syncAbsMs:{p95:0,p99:0},stallWindowSync:{earlyCount:classified.stall.filter((row)=>row.value < -2).length},stallWindowsInjected:0,stallWindows:[]}});
    expect(result.failures).toContain('stall-window early flashes=1');
  });
  it('audits full workload onsets and gates an expired highlight still active on F', () => {
    const onset={time:1,end:1.8,from:1,to:2,epoch:'e'};
    const frame={frameMs:1000,targetMs:1000,audibleTime:1.8,epoch:'e',activeKey:'1-2'};
    const sample={time:1.7,frameIndex:0,value:0,onset,frame};
    const classified=stats.classifyStallSamples([sample],[frame],[{index:0,startMs:1000,endMs:1250,beatResidualMs:0}],[onset]);
    expect(classified.windows[0]).toMatchObject({audited:true,activeSetMatch:false,replayed:true});
    const result=stats.evaluate({metrics:{...passingMetrics,syncProvenance:'measured',syncAbsMs:{p95:0,p99:0},stallWindowSync:{earlyCount:0},stallWindowsInjected:1,stallWindows:classified.windows}});
    expect(result.failures).toContain('stall-window active-set mismatches=1');
    expect(result.failures).toContain('expired highlights replayed after stall');
  });
  it('computes beat residual only for the running matching epoch', () => {
    const transport={running:true,epoch:'e',cycle:[1,2] as [number,number],sample_time:2,bpm:120,beats_per_cycle:4};
    expect(stats.beatResidualMs({epoch:'other',beatCycle:1,audibleTime:2},transport)).toBeNull();
    expect(stats.beatResidualMs({epoch:'e',beatCycle:1,audibleTime:2},{...transport,running:false})).toBeNull();
    expect(stats.beatResidualMs({epoch:'e',beatCycle:0.501,audibleTime:2},transport)).toBeCloseTo(2,8);
  });
  it('attributes a stall gap before the first active frame to that sync sample', () => {
    const onsets=[{time:0,end:0.1,from:100,to:101,epoch:'e'},{time:0.2,end:0.3,from:1,to:2,epoch:'e'}];
    const rows=[
      {audibleTime:0.1,activeKey:'',epoch:'e',frameMs:100,targetMs:100},
      {audibleTime:0.2,activeKey:'1-2',epoch:'e',frameMs:200,targetMs:200},
      {audibleTime:0.2167,activeKey:'1-2',epoch:'e',frameMs:216.7,targetMs:216.7},
    ];
    const result=stats.attributeSync(onsets,rows,{nominalMs:16.7});
    expect(result.samples[0]).toMatchObject({previousFrameMs:100,droppedFrames:5});
    expect(result).toMatchObject({droppedFrames:5,droppedFrameSamples:1});
  });
  it('tightens textWork to 8 ms while recording non-gating targets', () => {
    const over=stats.evaluate({metrics:{...passingMetrics,textWorkMs:{p95:8.1}}});
    const equal=stats.evaluate({metrics:{...passingMetrics,textWorkMs:{p95:8.0}}});
    const targetMiss=stats.evaluate({metrics:{...passingMetrics,textWorkMs:{p95:5}}});
    expect(over.pass).toBe(false);
    expect(over.failures.some((failure)=>failure.includes('textWorkMs.p95'))).toBe(true);
    expect(equal.pass).toBe(true);
    expect(targetMiss).toMatchObject({pass:true,targets:{textWorkP95Met:false,animationWorkP50Met:false}});
    expect(stats.evaluate({metrics:{...passingMetrics,textWorkMs:{p95:Number.NaN},animationWorkMs:{p50:Number.NaN}}}).targets)
      .toEqual({textWorkP95Met:null,animationWorkP50Met:null});
    expect(stats.TARGETS).toEqual({textWorkP95Ms:4,animationWorkP50Ms:1});
  });
  it('keeps every non-text-work threshold unchanged', () => {
    expect(stats.THRESHOLDS).toEqual({inputP95Ms:50,inputP99Ms:100,animationWorkP50Ms:4,animationWorkP95Ms:8,animationWorkP99Ms:16.7,textWorkP95Ms:8,frameIntervalP95Ms:20,frameIntervalP99Ms:50,syncAbsP95Ms:33.4,syncAbsP99Ms:50,earlyFlashMs:2,ledgerMiB:96,heapGrowthMiB:8,lateBeatDriftMs:1});
  });
  it('counts covered early and expired frames while excluding unknown epochs', () => {
    const onsets=Array.from({length:9},(_,i)=>({time:i*0.25,end:i*0.25+0.2,from:1,to:2,epoch:'e'}));onsets.push({time:1,end:1.4,from:5,to:7,epoch:'e'});
    const rows=[
      {audibleTime:0.99,activeKey:'5-7',epoch:'e',frameMs:990,targetMs:990},
      {audibleTime:1.1,activeKey:'5-7',epoch:'other',frameMs:1100,targetMs:1100},
      {audibleTime:1.2,activeKey:'981-98,981-983',epoch:'e',frameMs:1200,targetMs:1200},
      {audibleTime:1.42,activeKey:'5-7',epoch:'e',frameMs:1420,targetMs:1420},
    ];
    const result=stats.attributeSync(onsets,rows);
    expect(result.earlyFlashCount).toBe(3);expect(result.replayedFlashCount).toBe(1);
    expect(result.framePairs).toBe(4);
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
    expect(stats.lateActiveMismatchDetails(onsets,rows,[3500])[0]).toMatchObject({
      stallFrameMs:3500,frameMs:3500,audibleTime:3.5,expectedRanges:['1-2'],
      actualRanges:['1-2','8-9'],missingRanges:[],extraRanges:['8-9'],mismatch:true,
    });
  });
  it('does not expect an onset before its receipt but checks it on later frames', () => {
    const onsets=[0,0.5,1,1.5,2].map((time)=>({time,end:time+1,from:5,to:7,epoch:'e',receivedMs:time===2?3000:time*1000}));
    const beforeReceipt={audibleTime:2.5,activeKey:'',epoch:'e',frameMs:2500};
    const afterReceipt={audibleTime:2.6,activeKey:'',epoch:'e',frameMs:3100};
    expect(stats.lateActiveMismatches(onsets,[beforeReceipt],[2500])).toBe(0);
    expect(stats.lateActiveMismatches(onsets,[afterReceipt],[3100])).toBe(1);
    expect(stats.lateActiveMismatchDetails(onsets,[afterReceipt],[3100])[0]).toMatchObject({
      expectedRanges:['5-7'],actualRanges:[],eligibleOnsets:[expect.objectContaining({time:2,receivedMs:3000})],
      overlappingOnsets:[expect.objectContaining({time:2,receivedMs:3000,eligible:true})],
    });
    const beforeAudit=stats.lateActiveMismatchDetails(onsets,[beforeReceipt,afterReceipt],[2500])[0];
    expect(beforeAudit).toMatchObject({expectedRanges:[],overlappingOnsets:[expect.objectContaining({time:2,receivedMs:3000,eligible:false})]});
  });
  it('uses callback execution time when a playing receipt follows the RAF timestamp', () => {
    const onsets=[0,0.5,1,1.5,2].map((time)=>({time,end:time+1,from:5,to:7,epoch:'e',receivedMs:time===2?2510:time*1000}));
    const row={audibleTime:2.5,activeKey:'5-7',epoch:'e',frameMs:2500,executionMs:2520};
    const audit=stats.lateActiveMismatchDetails(onsets,[row],[2500])[0];
    expect(audit).toMatchObject({frameMs:2500,executionMs:2520,expectedRanges:['5-7'],actualRanges:['5-7'],mismatch:false,
      overlappingOnsets:[expect.objectContaining({time:2,receivedMs:2510,eligible:true})]});
  });
  it('identifies only tick gaps overlapping a recorded long main-thread span', () => {
    const rows=[
      [6,0,0,0,0,0,0,0,1],
      [5,50,0,0,0,0,0,80,0],
      [6,200,0,0,0,0,0,0,1],
      [4,500,0,0,0,0,0,60,0],
      [6,310,0,0,0,0,0,0,1],
    ];
    expect(stats.tickStarvationEvidence(rows)).toMatchObject({
      tickStartCount:3,tickStartGapsOverLookahead:1,maxTickStartGapMs:200,
      mainThreadSpansOverThreshold:2,starvationOverlaps:1,
      overlaps:[{startMs:0,endMs:200,durationMs:200,span:{phase:5,startMs:50,durationMs:80}}],
    });
    expect(stats.tickStarvationEvidence([[6,0,0,0,0,0,0,0,1],[6,10,0,0,0,0,0,0,1]])
      .starvationOverlaps).toBe(0);
  });
  it('excludes telemetry retention holes while still checking covered frames and stalls', () => {
    const onsets=[0,1,2,3,4,10,11,12,13,14].map((time)=>({time,end:time+0.8,from:5,to:7,epoch:'e'}));
    const rows=[
      {audibleTime:2.2,activeKey:'5-7',epoch:'e',frameMs:2200,targetMs:2200},
      {audibleTime:7,activeKey:'5-7',epoch:'e',frameMs:7000,targetMs:7000},
      {audibleTime:10.2,activeKey:'5-7',epoch:'e',frameMs:10200,targetMs:10200},
    ];
    const result=stats.attributeSync(onsets,rows);
    expect(result).toMatchObject({framePairs:2,replayedFlashCount:0,coverageWindows:2,excludedFrames:1});
    expect(stats.lateActiveMismatches(onsets,rows,[7000,10200])).toBe(0);
    expect(stats.lateActiveMismatches(onsets,rows,[2200])).toBe(0);
    const coveredMismatch=[...rows.slice(0,1),{...rows[0],audibleTime:2.9,frameMs:2900}];
    expect(stats.lateActiveMismatches(onsets,coveredMismatch,[2900])).toBe(1);
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
    for (const row of ['Input latency p95', 'Editing keystrokes / paired', 'Audio control / run start', 'Silent sink post-sink peak', 'Direct destination connections', 'Pre-sink peak (dBFS)', 'Pre-sink RMS (dBFS)', 'Animation frame work p95', 'Frame interval p95', 'A/V model absolute error', 'A/V sync dropped-frame samples', 'A/V sync duplicates folded', 'Resource ledger peak', 'JS heap growth', 'Beat drift', 'Replayed flashes']) expect(text).toContain(row);
    expect(text).toContain('<= 8 (target 4, recorded)');
    expect(text).toContain('<= 4 (target 1, recorded)');
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
