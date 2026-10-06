const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
import fs from 'node:fs';
import path from 'node:path';
import { installSilentSink } from './silent-sink.mjs';
export async function runBehavior(browser, origin, name) {
  const results = []; const limitations = [];
  let context; let page; let api;
  const check = async (id, action) => {
    try {
      const detail = await action();
      if (detail?.status === 'limitation') { limitations.push(detail.detail); results.push({ id, status:'limitation', pass:null, detail:detail.detail }); }
      else results.push({ id, pass: true, detail: detail ?? 'verified' });
    }
    catch (error) { results.push({ id, pass: false, detail: String(error?.stack ?? error) }); }
  };
  try {
    context = await browser.newContext({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1, hasTouch: true,
      permissions: name === 'chromium' ? ['clipboard-read', 'clipboard-write'] : [] });
    await installSilentSink(context);
    page = await context.newPage();
    await page.goto(`${origin}/?perf=1`, { waitUntil: 'domcontentloaded', timeout: 60000 });
    await page.waitForFunction(() => Boolean(window.__vactrPerf), { timeout: 30000 });
    await page.waitForSelector('.vact-code-canvas', { timeout: 30000 });
    api = await page.evaluate(() => Boolean(window.__vactrPerf));
    await check('canvas-only-text', async () => {
      const ta=page.locator('.vact-code-input-bridge textarea');await ta.focus();await ta.fill('# canvas-visible-evidence');await ta.type('x');await ta.press('Backspace');
      return page.evaluate(async () => {
        await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
        const bridge = document.querySelector('.vact-code-input-bridge textarea');
        if (!bridge) throw Error('bridge missing');
        const canvas = document.querySelector('.vact-code-canvas');
        if (!canvas) throw Error('canvas missing');
        const canvasRect = canvas.getBoundingClientRect();
        const scaleX = canvas.width / canvasRect.width;
        const lineStart = Math.floor(48 * scaleX);
        let bridgeFrames = 0;
        while (Math.floor((bridge.getBoundingClientRect().left - canvasRect.left) * scaleX) <= lineStart + 3) {
          if (++bridgeFrames >= 30) throw Error('bridge did not receive positioned bounds within 30 animation frames');
          await new Promise((resolve) => requestAnimationFrame(resolve));
        }
        return new Promise((resolve, reject) => {
        const pane=document.querySelector('.vact-code');const canvas=document.querySelector('.vact-code-canvas');const bridge=document.querySelector('.vact-code-input-bridge textarea');
        if(!pane||!canvas||!bridge){reject(Error('code pane, canvas, or bridge missing'));return;}
        const source=window.__vactrPerf.doc();
        if([...pane.querySelectorAll('*')].some((el)=>el!==bridge&&el.textContent?.includes(source)&&source.length>0)){reject(Error('document text appears in DOM'));return;}
        const style=getComputedStyle(bridge);if(style.opacity!=='0'&&style.color!=='rgba(0, 0, 0, 0)'){reject(Error('bridge textarea is visible'));return;}
        if(!canvas.getContext('webgl2')){reject(Error('WebGL2 unavailable'));return;}
        if(canvas.width<=1||canvas.height<=1){reject(Error(`canvas backing size is ${canvas.width}x${canvas.height}`));return;}
        const caretRect=bridge.getBoundingClientRect();const scaleY=canvas.height/canvasRect.height;
        const left=Math.max(0,Math.floor(48*scaleX));const right=Math.min(canvas.width,Math.max(left+1,Math.ceil((caretRect.left-canvasRect.left)*scaleX)));
        const top=Math.max(0,Math.floor((caretRect.top-canvasRect.top)*scaleY));const bottom=Math.min(canvas.height,Math.max(top+1,Math.ceil(top+caretRect.height*scaleY)));
        if(right-left<4){reject(Error(`line-1 glyph rectangle too narrow: ${right-left}px`));return;}
        const originalRaf=window.requestAnimationFrame;let attempts=0;let done=false;
        const finish=(error,detail)=>{if(done)return;done=true;window.requestAnimationFrame=originalRaf;if(error)reject(error);else resolve(detail);};
        window.requestAnimationFrame=(callback)=>originalRaf((time)=>{const before=window.__vactrPerf.presented().at(-1)?.frameMs;callback(time);const after=window.__vactrPerf.presented().at(-1)?.frameMs;
          if(after!==undefined&&after!==before){try{const probe=document.createElement('canvas');probe.width=canvas.width;probe.height=canvas.height;const ctx=probe.getContext('2d');ctx.drawImage(canvas,0,0);const image=ctx.getImageData(left,top,right-left,bottom-top);const colors=new Set();for(let i=0;i<image.data.length;i+=4)colors.add(`${image.data[i]},${image.data[i+1]},${image.data[i+2]}`);if(colors.size<2)throw Error(`canvas pixel readback has no glyph variation in line-1 rect ${left},${top},${right-left},${bottom-top}`);canvas.hidden=true;const stillVisible=[...pane.querySelectorAll('*')].some((el)=>el.textContent?.includes(source)&&source.length>0);canvas.hidden=false;if(stillVisible)throw Error('hiding canvas leaves source visible');finish(null,`source chars=${source.length}; bridge opacity=${style.opacity}; canvas=${canvas.width}x${canvas.height}; line1=${left},${top},${right-left},${bottom-top}; colors=${colors.size}`);}catch(error){canvas.hidden=false;finish(error);}}
          else if(++attempts>=30)finish(Error('no frame recorded a new presented row within 30 animation frames'));});
        originalRaf(()=>{});
        });
      });
    });
    await check('editing-undo-redo-navigation', async () => {
      const ta = page.locator('.vact-code-input-bridge textarea'); await ta.focus();
      await ta.press('ControlOrMeta+End'); await ta.type('\n#e2e'); await page.waitForTimeout(50);
      const changed = await page.evaluate(() => window.__vactrPerf.doc().endsWith('#e2e'));
      if (!changed) throw Error('typing did not update document');
      const typedRevision = await page.evaluate(() => window.__vactrPerf.revision());
      await ta.press('ControlOrMeta+z');
      const undone = await page.evaluate(() => ({ doc:window.__vactrPerf.doc(), revision:window.__vactrPerf.revision() }));
      if (undone.doc.endsWith('#e2e') || undone.revision <= typedRevision) throw Error(`undo did not remove typed text: ${JSON.stringify(undone)}`);
      await ta.press('ControlOrMeta+Shift+z');
      const redone = await page.evaluate(() => ({ doc:window.__vactrPerf.doc(), revision:window.__vactrPerf.revision() }));
      if (!redone.doc.endsWith('#e2e') || redone.revision <= undone.revision) throw Error(`redo did not restore typed text: ${JSON.stringify(redone)}`);
      await ta.press('ControlOrMeta+Home');
      const home = await page.evaluate(() => window.__vactrPerf.selection());
      if (home.anchor !== 0 || home.head !== 0) throw Error(`document Home selection=${JSON.stringify(home)}`);
      await ta.press('Control+ArrowRight');
      const word = await page.evaluate(() => window.__vactrPerf.selection());
      if (word.anchor !== word.head || word.head <= home.head || word.head >= redone.doc.length) throw Error(`word navigation selection=${JSON.stringify(word)}`);
      await ta.press('ControlOrMeta+End');
      const end = await page.evaluate(() => window.__vactrPerf.selection());
      if (end.anchor !== redone.doc.length || end.head !== redone.doc.length) throw Error(`document End selection=${JSON.stringify(end)}`);
      return `undo=${undone.revision}; redo=${redone.revision}; home=${home.head}; word=${word.head}; end=${end.head}`;
    });
    if (name === 'chromium') {
      await check('clipboard-round-trip', async () => {
        await context.grantPermissions(['clipboard-read', 'clipboard-write']);
        await page.evaluate(() => navigator.clipboard.writeText('canvas-clipboard-evidence'));
        const text = await page.evaluate(() => navigator.clipboard.readText());
        if (text !== 'canvas-clipboard-evidence') throw Error(`clipboard read ${text}`); return text;
      });
    } else {
      await check('clipboard-synthetic-event', async () => ({ status:'limitation', detail:'WebKit synthetic ClipboardEvent cannot verify system clipboard contents; excluded from pass counts.' }));
    }
    if (name === 'chromium') {
      await check('japanese-ime', async () => {
        const session = await context.newCDPSession(page); const before = await page.evaluate(() => ({ doc:window.__vactrPerf.doc(), revision:window.__vactrPerf.revision() }));
        try {
          await session.send('Input.imeSetComposition', { text: '\u306b\u307b\u3093\u3054', selectionStart: 5, selectionEnd: 5 });
          const composing = await page.evaluate(() => ({ doc:window.__vactrPerf.doc(), revision:window.__vactrPerf.revision() }));
          if (composing.doc !== before.doc || composing.revision !== before.revision) throw Error('IME preedit changed committed document state');
          await session.send('Input.insertText', { text: '\u65e5\u672c\u8a9e' });
          await page.waitForFunction(() => window.__vactrPerf.doc().includes('\u65e5\u672c\u8a9e'));
          const committed = await page.evaluate(() => ({ doc:window.__vactrPerf.doc(), revision:window.__vactrPerf.revision() }));
          if (committed.revision !== before.revision + 1) throw Error(`IME commit transactions=${committed.revision-before.revision}, expected exactly 1`);
          return `one commit revision=${committed.revision}; Japanese text present`;
        } finally { await session.detach(); }
      });
    } else {
      await check('japanese-ime-synthetic', async () => ({ status:'limitation', detail:'WebKit synthetic composition events cannot establish real IME commit behavior; excluded from pass counts.' }));
    }
    await check('touch-selection', async () => {
      const rect = await page.locator('.vact-code-canvas').boundingBox(); if (!rect) throw Error('canvas has no box');
      if (name === 'chromium') { const session = await context.newCDPSession(page); await session.send('Input.dispatchTouchEvent', { type:'touchStart', touchPoints:[{x:rect.x+80,y:rect.y+20}] }); await sleep(650); await session.send('Input.dispatchTouchEvent', { type:'touchMove', touchPoints:[{x:rect.x+155,y:rect.y+20}] }); await session.send('Input.dispatchTouchEvent', { type:'touchEnd', touchPoints:[] }); await session.detach(); }
      else { limitations.push('WebKit touch selection uses synthetic pointer events.'); const c=page.locator('.vact-code-canvas');const event={pointerId:1,pointerType:'touch',isPrimary:true,button:0,width:1,height:1,pressure:0.5};const geometry=await page.evaluate(()=>{const canvas=document.querySelector('.vact-code-canvas');const pane=document.querySelector('.vact-code');window.__vactrTouchEvents=[];for(const type of ['pointerdown','pointermove','pointerup','pointercancel'])canvas.addEventListener(type,e=>window.__vactrTouchEvents.push({type:e.type,constructor:e.constructor.name,pointerType:e.pointerType,isPrimary:e.isPrimary,pointerId:e.pointerId,button:e.button,buttons:e.buttons,width:e.width,height:e.height,timeStamp:e.timeStamp}),true);const doc=window.__vactrPerf.doc();const match=/[A-Za-z]+/.exec(doc);const start=match?.index??0;const word=match?.[0]??'';const line=doc.slice(0,start).split('\n').length-1;const column=start-doc.lastIndexOf('\n',start-1)-1;return{doc,canvas:canvas.getBoundingClientRect().toJSON(),pane:pane.getBoundingClientRect().toJSON(),touchX:pane.getBoundingClientRect().left+48+(column+Math.floor(word.length/2))*8,touchY:pane.getBoundingClientRect().top+line*20+10,word,wordStart:start};});const touchX=geometry.touchX,touchY=geometry.touchY;await c.dispatchEvent('pointerdown',{...event,clientX:touchX,clientY:touchY,buttons:1});await c.dispatchEvent('pointerup',{...event,clientX:touchX,clientY:touchY,buttons:0});await sleep(50);const tapSelection=await page.evaluate(()=>window.__vactrPerf.selection());await c.dispatchEvent('pointerdown',{...event,clientX:touchX,clientY:touchY,buttons:1});await sleep(520);const atLongPress=await page.evaluate(()=>({selection:window.__vactrPerf.selection(),handles:window.__vactrPerf.presented().at(-1)?.handles,events:window.__vactrTouchEvents}));await c.dispatchEvent('pointerup',{...event,clientX:touchX,clientY:touchY,buttons:0});let handleWaitError=null;try{await page.waitForFunction(()=>window.__vactrPerf.presented().at(-1)?.handles===2,undefined,{timeout:2000});}catch(error){handleWaitError=String(error);}const atEnd=await page.evaluate(()=>({selection:window.__vactrPerf.selection(),handles:window.__vactrPerf.presented().at(-1)?.handles,events:window.__vactrTouchEvents}));const detail=JSON.stringify({geometry,tapSelection,atLongPress,atEnd,handleWaitError});const diagnosticPath=path.resolve(process.cwd(),'../tmp/canvas-cutover/evidence/s274-webkit-touch-events.json');fs.mkdirSync(path.dirname(diagnosticPath),{recursive:true});fs.writeFileSync(diagnosticPath,detail+'\n');if(atLongPress.selection.anchor===atLongPress.selection.head||atEnd.selection.anchor===atEnd.selection.head||atEnd.handles!==2)throw Error(`WebKit touch diagnostic=${detail}`);return `WebKit pointer event diagnostic=${diagnosticPath}; ${detail}`; }
      await page.waitForFunction(()=>window.__vactrPerf.selection().anchor!==window.__vactrPerf.selection().head&&window.__vactrPerf.presented().at(-1)?.handles===2,undefined,{timeout:2000});const state=await page.evaluate(() => ({selection:window.__vactrPerf.selection(),handles:window.__vactrPerf.presented().at(-1)?.handles}));if(state.selection.anchor===state.selection.head||state.handles!==2)throw Error(`long press did not create word selection handles: ${JSON.stringify(state)}`);return JSON.stringify(state);
    });
    await check('context-loss-restore', async () => {
      return page.evaluate(async () => {
        const canvas = document.querySelector('.vact-code-canvas');
        const gl = canvas?.getContext('webgl2');
        const ext = gl?.getExtension('WEBGL_lose_context');
        if (!ext) throw Error('WEBGL_lose_context unavailable');
        const before = window.__vactrPerf.doc();
        const waitFor = async (predicate, timeoutMs) => {
          const deadline = performance.now() + timeoutMs;
          while (!predicate()) {
            if (performance.now() >= deadline) throw Error('timed out waiting for gpuStatus transition');
            await new Promise((resolve) => setTimeout(resolve, 25));
          }
        };
        ext.loseContext();
        await waitFor(() => window.__vactrPerf.counters().gpuStatus.kind === 'context-lost', 2000);
        ext.restoreContext();
        await waitFor(() => ['ready','degraded'].includes(window.__vactrPerf.counters().gpuStatus.kind), 5000);
        const kind = window.__vactrPerf.counters().gpuStatus.kind;
        if (window.__vactrPerf.doc() !== before) throw Error('document changed across WebGL context loss and restore');
        return `context-lost -> ${kind}`;
      });
    });
    await check('dpr-and-resize', async () => {
      const before = await page.evaluate(() => window.__vactrPerf.selection()); await page.setViewportSize({ width: 1100, height: 760 });
      let session=null;
      try {
        if (name === 'chromium') { session = await context.newCDPSession(page); await session.send('Emulation.setDeviceMetricsOverride', { width:1100,height:760,deviceScaleFactor:2,mobile:false }); }
        await page.waitForTimeout(100); const after = await page.evaluate(() => window.__vactrPerf.selection());
        if (JSON.stringify(before) !== JSON.stringify(after)) throw Error('selection changed after resize/DPR');const dpr=await page.evaluate(()=>({actual:devicePixelRatio,effective:window.__vactrPerf.counters().gpuStatus.effectiveDpr}));if(name==='chromium'&&(dpr.actual!==2||dpr.effective!==2))throw Error(`DPR did not update: ${JSON.stringify(dpr)}`);if(name==='webkit'){const c2=await browser.newContext({viewport:{width:900,height:700},deviceScaleFactor:2,hasTouch:true});try{await installSilentSink(c2);const p2=await c2.newPage();await p2.goto(`${origin}/?perf=1`,{waitUntil:'domcontentloaded'});await p2.waitForFunction(()=>Boolean(window.__vactrPerf));await p2.waitForTimeout(100);const value=await p2.evaluate(()=>({actual:devicePixelRatio,effective:window.__vactrPerf.counters().gpuStatus.effectiveDpr}));if(value.actual!==2||value.effective!==2)throw Error(`DPR-2 context did not update: ${JSON.stringify(value)}`);}finally{await c2.close();}}const bounds=await page.evaluate(()=>{const r=document.querySelector('.vact-code-input-bridge textarea').getBoundingClientRect();const h=window.visualViewport?.height??innerHeight;return{right:r.right,bottom:r.bottom,height:h,width:innerWidth}});if(bounds.bottom>bounds.height||bounds.right>bounds.width)throw Error(`bridge bounds overflow: ${JSON.stringify(bounds)}`);return JSON.stringify(dpr);
      } finally { if(session){await session.send('Emulation.clearDeviceMetricsOverride').catch(()=>{});await session.detach();} }
    });
    await check('visual-viewport-inset', async () => page.evaluate(async () => {
      const viewport = window.visualViewport;
      if (!viewport || !Object.isExtensible(viewport)) return { status:'limitation', detail:'visualViewport height cannot be overridden in this browser; keyboard-inset geometry remains unverified.' };
      const originalHeight = viewport.height;
      try {
        Object.defineProperty(viewport, 'height', { configurable:true, get:() => Math.min(originalHeight, 360) });
        viewport.dispatchEvent(new Event('resize'));
        await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
        const rect = document.querySelector('.vact-code-input-bridge textarea').getBoundingClientRect();
        if (rect.bottom > viewport.height) throw Error(`caret bridge bottom=${rect.bottom} exceeds inset viewport height=${viewport.height}`);
        return { status:'pass', detail:`bridge bottom=${rect.bottom}; inset height=${viewport.height}` };
      } catch (error) {
        if (error instanceof Error && error.message.includes('exceeds inset')) throw error;
        return { status:'limitation', detail:`visualViewport height substitution unavailable: ${String(error)}` };
      } finally {
        delete viewport.height;
        viewport.dispatchEvent(new Event('resize'));
      }
    }));
    await check('background-no-replay', async () => {
      await page.evaluate(() => { Object.defineProperty(document, 'visibilityState', { configurable:true, value:'hidden' }); document.dispatchEvent(new Event('visibilitychange')); });
      await sleep(100);
      const before = await page.evaluate(() => ({frames:window.__vactrPerf.perf.snapshot().frames.length,onsets:window.__vactrPerf.onsets()}));
      await sleep(2000);
      const hidden = await page.evaluate(() => window.__vactrPerf.perf.snapshot().frames.length);
      await page.evaluate(() => { Object.defineProperty(document, 'visibilityState', { configurable:true, value:'visible' }); document.dispatchEvent(new Event('visibilitychange')); });
      await sleep(150); const now = await page.evaluate(() => window.__vactrPerf.perf.snapshot().frames.length);
      if (hidden !== before.frames) throw Error(`frames grew while hidden: ${before.frames} -> ${hidden}`);
      if (now < hidden) throw Error('frame count regressed');const recovered=await page.evaluate(()=>({presented:window.__vactrPerf.presented(),onsets:window.__vactrPerf.onsets()}));const first=recovered.presented.at(-1);if(first){const expected=recovered.onsets.filter(o=>o.time<=first.audibleTime&&first.audibleTime<o.end).map(o=>`${o.from}-${o.to}`).sort().join(',');if(expected!==String(first.activeKey).split(',').filter(Boolean).sort().join(','))throw Error(`first resumed active set mismatch: expected ${expected}, got ${first.activeKey}`);for(const o of recovered.onsets)if(recovered.presented.some(p=>p.activeKey.includes(`${o.from}-${o.to}`)&&p.audibleTime>=o.end))throw Error('expired highlight replayed after resume');}return `frames hidden=${hidden}, resumed=${now}`;
    });
    await check('dispose-ledger', async () => page.evaluate(() => {
      const api = window.__vactrPerf; const ledger = api.ledger; api.disposeCode();
      if (ledger.usedBytes !== 0 || window.__vactrPerf !== undefined) throw Error(`dispose ledger=${ledger.usedBytes}`); return `usedBytes=${ledger.usedBytes}`;
    }));
    if (!api) throw new Error('perf API was not installed');
  } catch (error) { results.push({ id:'browser-setup', pass:false, detail:String(error?.stack ?? error) }); }
  finally { await context?.close(); }
  const counted = results.filter((result) => result.status !== 'limitation');
  return { name, checks:results, passed:counted.filter((result)=>result.pass===true).length, total:counted.length, limitations };
}
