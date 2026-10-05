import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createLargeDocument } from './fixtures/large-doc.mjs';
import { percentile, pairInputLatency, evaluate, THRESHOLDS } from './stats.mjs';
const sleep=(ms)=>new Promise((r)=>setTimeout(r,ms));
export async function runMeasurement(page, context, browserName, { profile='all', runId='run-001' }={}) {
  const api = () => page.evaluate(() => window.__vactrPerf?.perf?.snapshot?.() ?? null);
  const limitations=[]; const samples=[]; const startupFailures=[]; let blocked=false; let controlOnsetCount=0; let controlOnsets=[]; let controlOnsetKeys=new Set(); let controlStartMethod='toolbar-click'; let largeDocumentStartSucceeded=false;
  await page.waitForFunction(() => Boolean(window.__vactrPerf), { timeout:30000 });
  const workload=profile==='behavior'?null:createLargeDocument();
  let audioState='unavailable';
  if(workload){const audioButton=page.locator('.vact-audio');if(await audioButton.count()){await audioButton.click();audioState=await audioButton.getAttribute('aria-label');}const ta=page.locator('.vact-code-input-bridge textarea');const runButton=page.locator('.vact-run');await ta.focus();await ta.fill(workload.controlText);await page.waitForTimeout(300);try{await runButton.click({timeout:10000});}catch(error){startupFailures.push(`Head-only control toolbar click failed: ${String(error)}`);}try{await page.waitForFunction(()=>window.__vactrPerf.onsets().length>0,undefined,{timeout:10000});}catch{/* Control result distinguishes workload/harness setup from large-document behavior. */}controlOnsets=await page.evaluate(()=>window.__vactrPerf.onsets());controlOnsetCount=controlOnsets.length;controlOnsetKeys=new Set(controlOnsets.map((row)=>JSON.stringify(row)));await ta.fill(workload.text);await page.waitForTimeout(500);const loaded=await page.evaluate(()=>window.__vactrPerf.doc());if(loaded.length!==workload.text.length)throw new Error(`large document load mismatch: ${loaded.length}`);try{await runButton.click({timeout:10000});largeDocumentStartSucceeded=true;}catch(error){startupFailures.push(`Large-document toolbar click failed: ${String(error)}`);}if(largeDocumentStartSucceeded){await page.waitForTimeout(2000);try{await page.waitForFunction((baseline)=>window.__vactrPerf.onsets().some((row)=>!baseline.includes(JSON.stringify(row))),[...controlOnsetKeys],{timeout:10000});}catch{/* Missing onset telemetry remains a gated measurement failure. */}}}
  const audioRunning=audioState==='audio running';if(!audioRunning)limitations.push(`Accessible audio state is ${audioState??'unavailable'} after the gesture.`);
  const warmupStart=Date.now(); await sleep(profile==='behavior'?50:10_000); const warm=await api();
  if (!warm) throw new Error('pinned perf.snapshot API unavailable');
  let heapStart=null;
  if (browserName==='chromium') { try { const cdp=await context.newCDPSession(page); await cdp.send('HeapProfiler.collectGarbage'); heapStart=(await cdp.send('Runtime.getHeapUsage')).usedSize; await cdp.detach(); } catch { limitations.push('Chromium heap API unavailable.'); } }
  if (!audioRunning) { limitations.push('AudioContext did not expose a running audio workload; A/V sync and audio counters are unavailable.'); if(browserName==='chromium') blocked=true; }
  let ledgerPeakBytes=0; let editingSnapshot=null;
  if(profile!=='behavior') {
    const ta=page.locator('.vact-code-input-bridge textarea'); await ta.focus();
    await context.newPage().then(async(p)=>{
      let tempFile=null;
      try { await p.setContent('<canvas width="1280" height="720"></canvas>'); const bytes=await p.evaluate(async()=>{ const c=document.querySelector('canvas'); const ctx=c.getContext('2d'); ctx.fillStyle='#132d46';ctx.fillRect(0,0,1280,720); const stream=c.captureStream(30); const rec=new MediaRecorder(stream); const chunks=[];rec.ondataavailable=e=>chunks.push(e.data);rec.start();await new Promise(r=>setTimeout(r,3000));rec.stop();await new Promise(r=>rec.onstop=r);stream.getTracks().forEach(t=>t.stop());return Array.from(new Uint8Array(await new Blob(chunks,{type:rec.mimeType}).arrayBuffer())); }); tempFile=path.join(os.tmpdir(),`vactr-e2e-${process.pid}.webm`);fs.writeFileSync(tempFile,Buffer.from(bytes)); const input=page.locator('[data-pane="visual"] input[type="file"]'); if(await input.count()) await input.setInputFiles(tempFile); else limitations.push('Video file input selector unavailable; generated 720p fixture recorded but not loaded.'); }
      catch { limitations.push('MediaRecorder 720p video fixture unsupported in this browser.'); } finally { if(tempFile)fs.rmSync(tempFile,{force:true}); await p.close(); }
    });
    const editStart=Date.now();
    while(Date.now()-editStart<60_000) { await ta.press('End'); await ta.type('x'); await ta.press('Backspace'); await ta.press('Shift+ArrowLeft');await ta.press('ArrowRight');await ta.press('Home');await ta.press('End');await ta.press('ArrowLeft');await ta.press('ArrowRight');await ta.press('ArrowDown');await page.mouse.wheel(0, 220); ledgerPeakBytes=Math.max(ledgerPeakBytes,await page.evaluate(()=>window.__vactrPerf.counters().usedBytes));samples.push({phase:'edit',at:Date.now()}); }
    editingSnapshot=await api();
    const cycleStart=Date.now(); let cycle=0; let cdp=null;
    if(browserName==='chromium')cdp=await context.newCDPSession(page);
    try {
      while(Date.now()-cycleStart<120_000) {
        const at=Date.now(); await ta.press('End'); await ta.type('z'); await ta.press('Backspace');
        if(cdp)await cdp.send('Emulation.setDeviceMetricsOverride',{width:1280,height:900,deviceScaleFactor:cycle%2?2:1,mobile:false});
        const observedDpr=await page.evaluate(()=>devicePixelRatio);
        await page.evaluate(()=>{ const c=document.querySelector('.vact-code-canvas'); c.style.fontSize=`${12+(Math.floor(performance.now()/5000)%3)}px`; window.dispatchEvent(new Event('resize')); });
        if(cycle%2===1) await page.evaluate(()=>{ const until=performance.now()+250; while(performance.now()<until){} });
        ledgerPeakBytes=Math.max(ledgerPeakBytes,await page.evaluate(()=>window.__vactrPerf.counters().usedBytes));samples.push({phase:'cycle',at,stall:cycle%2===1,devicePixelRatio:observedDpr}); cycle++; await sleep(5000);
      }
    } finally { if(cdp)await cdp.detach(); }
  }
  const finalSnapshot=await api();
  const mergeRows=(...groups)=>{const unique=new Map();for(const group of groups)for(const row of group??[])unique.set(JSON.stringify(row),row);return [...unique.values()].sort((a,b)=>Number(a[0])-Number(b[0]));};
  const frames=mergeRows(editingSnapshot?.frames,finalSnapshot?.frames); const keys=mergeRows(editingSnapshot?.keys,finalSnapshot?.keys);
  const intervals=[]; for(let i=1;i<frames.length;i++){ const dt=frames[i][0]-frames[i-1][0]; if(dt>=0&&dt<=200) intervals.push(dt); }
  const work=frames.map((r)=>Number(r[1])).filter(Number.isFinite); const textWork=frames.filter((r)=>Boolean(r[2])).map((r)=>Number(r[1])).filter(Number.isFinite); const animation=frames.filter((r)=>!r[2]).map((r)=>Number(r[1])).filter(Number.isFinite);
  const inputPairing=pairInputLatency(frames,keys);
  const editingPairing=editingSnapshot?pairInputLatency(editingSnapshot.frames??[],editingSnapshot.keys??[]):{paired:[],pairedKeys:0,nonEditingKeys:0,expiredKeys:0,unpairedKeys:0};
  const input=inputPairing.paired.map((pair)=>pair.latencyMs);
  const presented=await page.evaluate(()=>window.__vactrPerf.presented()); const onsets=largeDocumentStartSucceeded?(await page.evaluate(()=>window.__vactrPerf.onsets())).filter((row)=>!controlOnsetKeys.has(JSON.stringify(row))):[];const transport=await page.evaluate(()=>window.__vactrPerf.transportSample());
  const provenance=presented.find((p)=>p.valid)?.provenance??'unavailable'; const sync=[]; let early=0; let replayed=0;
  for(const o of onsets){const p=presented.find((v)=>v.activeKey?.includes(`${o.from}-${o.to}`)&&v.audibleTime>=o.time); if(p){const next=presented.find((v)=>v.frameMs>p.frameMs);if(next)sync.push(next.frameMs-(p.targetMs+(o.time-p.audibleTime)*1000));} for(const v of presented)if(v.activeKey?.includes(`${o.from}-${o.to}`)){if(v.audibleTime<o.time-.002)early++;if(v.audibleTime>=o.end)replayed++;}}
  let heapEnd=null; if(browserName==='chromium')try{const cdp=await context.newCDPSession(page);await cdp.send('HeapProfiler.collectGarbage');heapEnd=(await cdp.send('Runtime.getHeapUsage')).usedSize;await cdp.detach();}catch{}
  const counters=await page.evaluate(()=>window.__vactrPerf.counters()); const max=(a,p)=>percentile(a,p);
  const peakPlayingRanges=onsets.length?Math.max(0,...presented.map((p)=>String(p.activeKey??'').split(',').filter(Boolean).length)):0;let beatDriftMs=null;if(transport?.running&&onsets.length&&presented.length&&Number.isFinite(presented.at(-1).beatCycle)){const cyc=transport.cycle[0]/transport.cycle[1];const expected=cyc+(presented.at(-1).audibleTime-transport.sample_time)*transport.bpm/60/transport.beats_per_cycle;beatDriftMs=(presented.at(-1).beatCycle-expected)*60*transport.beats_per_cycle/transport.bpm*1000;}
  const onsetAttribution=onsets.length>0?'active-workload-onsets-observed':controlOnsetCount>0?(largeDocumentStartSucceeded?'product/full-document failure: control sounded but large document produced no onsets':'product/full-document startup failure: proven toolbar click timed out on the large document'):'workload/harness failure: head-only control produced no onsets';
  const metrics={ inputLatencyMs:{p95:max(input,95),p99:max(input,99)},animationWorkMs:{p50:max(animation,50),p95:max(animation,95),p99:max(animation,99)},textWorkMs:{p95:max(textWork,95)},frameIntervalMs:{p95:max(intervals,95),p99:max(intervals,99)},syncProvenance:provenance,syncAbsMs:{p95:max(sync.map(Math.abs),95),p99:max(sync.map(Math.abs),99)},earlyFlashCount:early,replayedFlashCount:replayed,ledgerMaxBytes:Math.max(ledgerPeakBytes,counters.usedBytes),ledgerCounters:counters.ledger,heapGrowthBytes:heapStart!==null&&heapEnd!==null?heapEnd-heapStart:null,beatDriftMs,peakPlayingRanges,audioRunning,onsetCount:onsets.length,controlOnsetCount,controlStartMethod,runStartMethod:controlStartMethod,largeDocumentStartStatus:largeDocumentStartSucceeded?'toolbar-click-returned':'toolbar-click-timeout',onsetAttribution,workletCounters:'not exposed',probe:counters.probe,highlight:counters.highlight,client:counters.client,frameCount:frames.length,keyCount:keys.length,editKeyCount:Math.max(0,(editingSnapshot?.keys?.length??0)-editingPairing.expiredKeys),editPairedKeyCount:editingPairing.pairedKeys,inputSamplesUsed:input.length,inputKeysNonEditing:inputPairing.nonEditingKeys,inputKeysExpired:inputPairing.expiredKeys,inputKeysUnpaired:inputPairing.unpairedKeys,editPairedSamples:editingPairing.pairedKeys,editNonEditingKeys:editingPairing.nonEditingKeys,editExpiredKeys:editingPairing.expiredKeys,editUnpairedKeys:editingPairing.unpairedKeys,workSamples:work.length,startedAt:new Date(warmupStart).toISOString()};
  samples.push(...controlOnsets.map((r)=>({phase:'control-onset',...r})));samples.push(...frames.map((r)=>({phase:'frame',frameMs:r[0],workMs:r[1],text:Boolean(r[2]),revision:r[3]})));samples.push(...keys.map((r)=>({phase:'key',timeStamp:r[0],revision:r[1]})));samples.push(...inputPairing.paired.map((r)=>({phase:'input-pair',...r})));samples.push(...presented.map((r)=>({phase:'presented',...r})));samples.push(...onsets.map((r)=>({phase:'onset',...r})));
  const stalls=[];for(let i=1;i<frames.length;i++)if(frames[i][0]-frames[i-1][0]>200)stalls.push(frames[i][0]);let activeMismatch=0;for(const stall of stalls){const p=presented.find((item)=>item.frameMs>=stall);if(!p)continue;const expected=onsets.filter((o)=>o.time<=p.audibleTime&&p.audibleTime<o.end).map((o)=>`${o.from}-${o.to}`).sort().join(',');if(expected!==String(p.activeKey).split(',').filter(Boolean).sort().join(','))activeMismatch++;}
  const metricsLate={lateFrameCount:stalls.length,lateActiveMismatchCount:activeMismatch};if(activeMismatch)limitations.push(`${activeMismatch} post-stall active highlight set mismatches.`);metrics.lateFrameCount=metricsLate.lateFrameCount;metrics.lateActiveMismatchCount=metricsLate.lateActiveMismatchCount;
  if(audioRunning&&beatDriftMs===null)limitations.push('Beat phase drift unavailable because transport sample/presentation did not provide a running correlated position.');metrics.imeLatencyMs={p95:null,p99:null};metrics.imeLatencyNote='separate from key latency; CDP IME is covered by behavior checks but the pinned perf hook does not tag composition keys';
  metrics.ledgerAfterDisposeBytes=await page.evaluate(()=>{const a=window.__vactrPerf;const l=a.ledger;a.disposeCode();return l.usedBytes;});
  const gate=evaluate({metrics},THRESHOLDS);gate.failures.push(...startupFailures);if(activeMismatch)gate.failures.push(`post-stall active-set mismatches=${activeMismatch}`); return {runId,browser:browserName,metrics,pass:gate.pass&&gate.failures.length===0,failures:gate.failures,blocked,limitations,samples};
}
