#!/usr/bin/env node
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium, webkit } from 'playwright';
import { startServer, editorRoot, repoRoot } from './serve.mjs';
import { runBehavior } from './behavior.mjs';
import { runMeasurement } from './measure.mjs';
import { countChecks, renderEvidence } from './stats.mjs';
import { installSilentSink } from './silent-sink.mjs';
import { gatingPreflight } from './wasm-profile.mjs';
const args=process.argv.slice(2); const value=(key,def)=>{const i=args.indexOf(key);return i>=0?args[i+1]:def;};
const list=(value('--browser','all')==='all'?['chromium','webkit']:[value('--browser','all')]);
const profile=value('--profile','all'); const runId=value('--run-id','run-001'); const out=path.resolve(value('--out',path.join(repoRoot,'design-docs/specs/evidence/canvas-cutover',runId)));
const writeEvidence=args.includes('--write-evidence'); const headedWebkit=args.includes('--headed-webkit'); const profileTrace=args.includes('--profile-trace');
const command=`cd editor && npm run e2e -- --browser ${value('--browser','all')} --profile ${profile} --run-id ${runId}${writeEvidence?' --write-evidence':''}${headedWebkit?' --headed-webkit':''}${profileTrace?' --profile-trace':''}`;
const evidencePath=(name)=>path.join(out,name); const browsers=[]; let blocked=false; let fatal=null;
const host={hostname:os.hostname(),platform:os.platform(),release:os.release(),arch:os.arch(),node:process.version,runId,commands:[command,'mise run build-wasm-release','cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build']};
try {
  if(!fs.existsSync(path.join(editorRoot,'dist','index.html'))) throw Object.assign(new Error(`built editor dist missing at ${path.join(editorRoot,'dist')}`),{blocked:true});
  const preflight=await gatingPreflight({distWasm:path.join(editorRoot,'dist','vactr.wasm'),releasePath:path.join(repoRoot,'target/wasm32-unknown-unknown/release/vactr.wasm'),debugPath:path.join(repoRoot,'target/wasm32-unknown-unknown/debug/vactr.wasm'),writeEvidence});
  const wasm=preflight.wasm; host.wasm=wasm;
  if(preflight.refusal){fatal=preflight.refusal;blocked=true;const environment={...host,osVersion:os.version(),webgl:{},simulator:{xcode:process.env.DEVELOPER_DIR??'xcode-select default; see ios-sim.json'}};const summary={runId,commands:host.commands,browsers:[],environment,wasm,pass:false,blocked:true,failures:[],fatal};console.log(JSON.stringify({runId,pass:false,blocked:true,wasm,fatal,testsRun:0,testsPassed:0,failureCount:0,summary}));process.exit(2);}
  fs.mkdirSync(out,{recursive:true});
  const server=await startServer();
  try {
    for(const name of list){
      const engine=name==='chromium'?chromium:webkit; let browser; let version=null; let actualHeadless=!(name==='webkit'&&headedWebkit); let behavior=null; let stage='browser-launch';
      try {
        browser=await engine.launch({headless:!(name==='webkit'&&headedWebkit),timeout:30000,args:name==='chromium'?['--mute-audio','--use-angle=metal','--enable-gpu','--ignore-gpu-blocklist']:[]});
        if(name==='webkit'&&actualHeadless){const probeContext=await browser.newContext();await installSilentSink(probeContext);const probe=await probeContext.newPage();await probe.setContent('<canvas></canvas>');const hasGl=await probe.evaluate(()=>Boolean(document.querySelector('canvas').getContext('webgl2')));await probeContext.close();if(!hasGl){await browser.close();browser=await engine.launch({headless:false,timeout:30000});actualHeadless=false;}}
        version=browser.version(); stage='behavior'; behavior=profile==='measure'?null:await runBehavior(browser,server.origin,name);
        const b={name,version,headless:actualHeadless,behavior:behavior?countChecks(behavior.checks):null,checks:behavior?.checks??[],limitations:behavior?.limitations??[],metrics:null,measurement:null};
        if(profile==='measure'||profile==='all'){
          stage='measurement';
          const context=await browser.newContext({viewport:{width:1280,height:900},deviceScaleFactor:1}); await installSilentSink(context); const page=await context.newPage();
          await page.goto(`${server.origin}/?perf=1`,{waitUntil:'domcontentloaded',timeout:60000});
          const measured=await runMeasurement(page,context,name,{profile:'all',runId,profileTrace:profileTrace&&name==='chromium',headless:actualHeadless});
          b.metrics=measured.metrics;b.phaseMs=measured.metrics.phaseMs??null;b.renderer=measured.renderer??null;b.control=measured.control??null;b.traceRanks=measured.traceRanks??[];b.measurement={pass:measured.pass,failures:measured.failures,blocked:measured.blocked,limitations:measured.limitations};b.samples=measured.samples;b.limitations.push(...measured.limitations);blocked ||= measured.blocked;
          await page.evaluate(()=>window.__vactrPerf?.disposeCode()); await context.close();
        }
        host[name]={version,headless:b.headless}; browsers.push(b); await browser.close();
      } catch(error){ await browser?.close().catch(()=>{}); const detail=String(error?.stack??error); if(/Executable doesn't exist|browserType\.launch/.test(detail))blocked=true; const checks=[...(behavior?.checks??[]),{id:stage==='measurement'?'workload-start':stage==='behavior'?'behavior-setup':'browser-launch',pass:false,detail}]; browsers.push({name,version,headless:actualHeadless,behavior:behavior?countChecks(behavior.checks):null,checks,limitations:behavior?.limitations??[],metrics:null,measurement:null}); }
    }
  } finally { await server.close(); }
} catch(error){fatal=String(error?.stack??error);blocked=Boolean(error?.blocked);}
const webgl=Object.fromEntries(browsers.map((browser)=>[browser.name,browser.renderer??null]));
for(const browser of browsers)if(String(browser.renderer??'').includes('SwiftShader'))browser.limitations.push('Chromium rendered with software GL (SwiftShader); frame metrics are recorded failures, never relabeled as passes');
const environment={...host,osVersion:os.version(),webgl,simulator:{xcode:process.env.DEVELOPER_DIR??'xcode-select default; see ios-sim.json'}};
for(const b of browsers)if(b.samples){b.sampleDownsampling={frame:'every 32nd sample',presented:'every 32nd sample',onset:'every 32nd sample',syncSample:'every 32nd sample',stallAudit:'every 32nd sample',beatResidualFrame:'every 32nd sample',phase:'serialized phase trace retains spans >= 50 ms; phase metrics use the complete in-memory trace',other:'all',analysis:'all pass/fail decisions and scalar metrics use the complete in-memory samples'};const metrics=b.metrics;if(metrics?.beatResidual?.frames)metrics.beatResidual={...metrics.beatResidual,frames:metrics.beatResidual.frames.filter((_,index)=>index%32===0)};for(const key of ['syncSamples','lateFrameAudits'])if(Array.isArray(metrics?.[key]))metrics[key]=metrics[key].filter((_,index)=>index%32===0);}
const anyFailed=browsers.some((b)=>countChecks(b.checks??[]).failed>0||(b.measurement&&(!b.measurement.pass||(b.measurement.failures??[]).length>0)));
const summary={runId,commands:host.commands,browsers:browsers.map(({samples,...b})=>b),environment,wasm:host.wasm,pass:!fatal&&!blocked&&!anyFailed,blocked,failures:browsers.flatMap((b)=>[...(b.checks??[]).filter((c)=>c.status!=='limitation'&&!c.pass).map((c)=>`${b.name}:${c.id}`),...(b.measurement?.failures??[]).map((x)=>`${b.name}:${x}`)]),fatal};
const checkTotals=countChecks(browsers.flatMap((b)=>b.checks??[]));const result={runId,pass:summary.pass,blocked,testsRun:checkTotals.total,testsPassed:checkTotals.passed,failureCount:checkTotals.failed,summary};
if(writeEvidence){
 fs.mkdirSync(out,{recursive:true}); fs.writeFileSync(evidencePath('environment.json'),JSON.stringify(environment,null,2)+'\n');
 for(const b of browsers){fs.writeFileSync(evidencePath(`${b.name}-behavior.json`),JSON.stringify({browser:b.name,version:b.version,checks:b.checks,limitations:b.limitations},null,2)+'\n');const rows=b.samples??[];const counters={};const sampledPhases=new Set(['frame','presented','onset','sync-sample','stall-audit']);const saved=rows.filter((row)=>{if(sampledPhases.has(row.phase)){const index=counters[row.phase]??0;counters[row.phase]=index+1;return index%32===0;}if(row.phase==='phase')return row.row?.childMs>=50;return true;});fs.writeFileSync(evidencePath(`${b.name}-measure.jsonl`),saved.map((x)=>JSON.stringify(x)).join('\n')+'\n');}
 fs.writeFileSync(evidencePath('summary.json'),JSON.stringify(summary,null,2)+'\n');
 const doc=path.join(repoRoot,'design-docs/specs/design-canvas-editor-evidence.md'); if(fs.existsSync(doc)){const source=fs.readFileSync(doc,'utf8');const section=renderEvidence(summary);const updated=source.replace(/<!-- EVIDENCE:BEGIN -->[\s\S]*?<!-- EVIDENCE:END -->/,`<!-- EVIDENCE:BEGIN -->\n${section}\n<!-- EVIDENCE:END -->`);if(updated===source&&!source.includes('<!-- EVIDENCE:BEGIN -->'))throw new Error('evidence markers missing');fs.writeFileSync(doc,updated);}
}
console.log(JSON.stringify(result)); if(fatal)console.error(fatal); process.exit(blocked?2:anyFailed||fatal?1:0);
