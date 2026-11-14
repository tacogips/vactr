// @vitest-environment node
import { describe, expect, it } from 'vitest';
interface SinkModule {
  dbfs(peak:number):number;
  blockRms(samples:Float32Array,sampleRate:number,blockMs?:number):number[];
  detectOnsets(values:number[],blockMs:number,options?:Record<string,number>):number[];
  silentSinkInit():void;
}
interface VmModule { runInNewContext(code:string,context:Record<string,unknown>):void; }
const sinkSpec:string='./silent-sink.mjs';
const vmSpec:string='node:vm';
const sink=(await import(/* @vite-ignore */ sinkSpec)) as SinkModule;
const vm=(await import(/* @vite-ignore */ vmSpec)) as unknown as VmModule;

describe('silent virtual sink',()=>{
  it('converts peaks to dBFS and detects thresholded onsets with a quiet gap',()=>{
    expect(sink.dbfs(0)).toBe(Number.NEGATIVE_INFINITY);expect(sink.dbfs(1)).toBe(0);expect(sink.dbfs(0.5)).toBeCloseTo(-6.02,2);
    const samples=new Float32Array(48_000);for(let i=4_800;i<14_400;i+=1)samples[i]=Math.SQRT1_2*0.1;
    const rms=sink.blockRms(samples,48_000);const db=rms.map(sink.dbfs);
    expect(sink.detectOnsets(db,10)).toHaveLength(1);
    const close=[...Array(10).fill(-Infinity),...Array(20).fill(-20),...Array(3).fill(-Infinity),...Array(20).fill(-20)];
    const separate=[...Array(10).fill(-Infinity),...Array(20).fill(-20),...Array(8).fill(-Infinity),...Array(20).fill(-20)];
    expect(sink.detectOnsets(close,10)).toHaveLength(1);expect(sink.detectOnsets(separate,10)).toHaveLength(2);
    expect(sink.detectOnsets([...Array(10).fill(-Infinity),...Array(20).fill(-45)],10)).toHaveLength(0);
  });
  it('wraps realtime destination connections through one zero-gain measurement chain',async()=>{
    const edges:Array<[FakeNode,FakeNode]>=[];
    class FakeNode { context:FakeContext; constructor(context:FakeContext){this.context=context;} connect(target:FakeNode){edges.push([this,target]);return target;} disconnect(_target?:FakeNode){} }
    class FakeDestination extends FakeNode {}
    class FakeAnalyser extends FakeNode { fftSize=0; getFloatTimeDomainData(data:Float32Array){data.fill(0);} }
    class FakeGain extends FakeNode { gain:{value:number};constructor(context:FakeContext,options:{gain:number}){super(context);this.gain={value:options.gain};} }
    class FakeContext { destination:FakeDestination;currentTime=1;sampleRate=48_000;state='running';constructor(){this.destination=new FakeDestination(this);}createAnalyser(){return new FakeAnalyser(this);}suspend(){this.state='suspended';return Promise.resolve();} }
    class FakeOffline extends FakeContext {}
    class FakeMedia { muted=false;play(){return Promise.resolve();} }
    const win:Record<string,unknown>={};
    vm.runInNewContext(`(${sink.silentSinkInit.toString()})()`,{window:win,AudioNode:FakeNode,AudioDestinationNode:FakeDestination,OfflineAudioContext:FakeOffline,HTMLMediaElement:FakeMedia,GainNode:FakeGain,setInterval:()=>1,clearInterval:()=>{},performance:{now:()=>1},Map,Math,Number});
    const ctx=new FakeContext();const source=new FakeNode(ctx);source.connect(ctx.destination);source.connect(ctx.destination);const other=new FakeNode(ctx);source.connect(other);
    const api=win.__vactrSink as {report():{contexts:number;wrappedDestinationConnections:number;directDestinationConnections:number;installedBeforeFirstConnect:boolean;mediaElementsForcedMuted:number;post:{peak:number}}};const report=api.report();
    expect(report).toMatchObject({contexts:1,wrappedDestinationConnections:2,directDestinationConnections:0,installedBeforeFirstConnect:true,mediaElementsForcedMuted:0,post:{peak:0}});
    expect(edges).toHaveLength(6);const [pre,gain]=edges[0]!;const [,post]=edges[1]!;expect(edges[2]?.[0]).toBe(post);expect(edges[2]?.[1]).toBe(ctx.destination);expect(edges[3]?.[0]).toBe(source);expect(edges[3]?.[1]).toBe(pre);expect(edges[4]?.[0]).toBe(source);expect(edges[4]?.[1]).toBe(pre);expect(edges[5]?.[0]).toBe(source);expect(edges[5]?.[1]).toBe(other);expect(pre).toBeInstanceOf(FakeAnalyser);expect(gain).toBeInstanceOf(FakeGain);expect((gain as FakeGain).gain.value).toBe(0);expect(post).toBeInstanceOf(FakeAnalyser);
    const media=new FakeMedia();await media.play();expect(media.muted).toBe(true);expect(api.report().mediaElementsForcedMuted).toBe(1);
    const offline=new FakeOffline();new FakeNode(offline).connect(offline.destination);expect((win.__vactrSink as {report():{wrappedDestinationConnections:number}}).report().wrappedDestinationConnections).toBe(2);
  });
});
