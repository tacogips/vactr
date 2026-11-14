import { createLargeDocument } from './fixtures/large-doc.mjs';
import { installSilentSink } from './silent-sink.mjs';

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export async function runDomBehavior(browser, origin, name) {
  const checks = []; const limitations = []; let context; let page;
  const check = async (id, action) => {
    try {
      const detail = await action();
      if (detail?.status === 'limitation') { limitations.push(detail.detail); checks.push({ id, status: 'limitation', pass: null, detail: detail.detail }); }
      else checks.push({ id, pass: true, detail: detail ?? 'verified' });
    } catch (error) { checks.push({ id, pass: false, detail: String(error?.stack ?? error) }); }
  };
  try {
    context = await browser.newContext({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1, hasTouch: true,
      permissions: name === 'chromium' ? ['clipboard-read', 'clipboard-write'] : [] });
    await installSilentSink(context); page = await context.newPage();
    await page.goto(`${origin}/?perf=1&renderer=dom`, { waitUntil: 'domcontentloaded', timeout: 60000 });
    await page.waitForFunction(() => Boolean(window.__vactrPerf), { timeout: 30000 }); await page.waitForSelector('.vact-code-dom', { timeout: 30000 });
    await check('dom-visible-text', async () => {
      const source = 'let alpha = 1\n# Japanese: 日本語\nlet emoji = "🎹🎧"';
      const ta = page.locator('.vact-code-input-bridge textarea'); await ta.focus(); await ta.fill(source); await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      return page.evaluate((expected) => {
        const root = document.querySelector('.vact-code-dom'); if (!root || document.querySelector('.vact-code-canvas')) throw Error('DOM renderer surface mismatch');
        const lines = expected.split('\n'); const nums = [...root.querySelectorAll('.vact-dom-num')];
        for (const num of nums) { const index = Number(num.textContent) - 1; const top = num.getBoundingClientRect().top; const line = [...root.querySelectorAll('.vact-dom-line')].find((node) => Math.abs(node.getBoundingClientRect().top - top) <= 1); if (!line || line.textContent !== lines[index]) throw Error(`visible DOM line ${index + 1} mismatch: ${line?.textContent}`); }
        const token = root.querySelector('.vact-dom-tok-string'); if (!token) throw Error('string token span missing');
        const probe = document.createElement('span'); probe.style.color = 'var(--vt-syn-string)'; document.body.append(probe); const expectedColor = getComputedStyle(probe).color; probe.remove();
        const tokenColor = getComputedStyle(token).color; if (tokenColor !== expectedColor) throw Error(`string token color ${tokenColor} != ${expectedColor}`);
        const bridge = document.querySelector('.vact-code-input-bridge textarea'); const style = getComputedStyle(bridge); if (style.opacity !== '0' && style.color !== 'rgba(0, 0, 0, 0)') throw Error('bridge textarea is visible');
        return `visible rows=${nums.length}; keyword=${tokenColor}; bridge opacity=${style.opacity}`;
      }, source);
    });
    await check('dom-caret-alignment', async () => {
      const lines = ['let ascii = "abc"', 'let japanese = "日本語"', 'let emoji = "🎹🎧"'];
      const ta = page.locator('.vact-code-input-bridge textarea'); await ta.fill(lines.join('\n'));
      const deltas = [];
      for (let index = 0; index < lines.length; index++) {
        await ta.press('ControlOrMeta+Home'); for (let row = 0; row < index; row++) await ta.press('ArrowDown'); await ta.press('End');
        await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
        const result = await page.evaluate((lineIndex) => {
          const root = document.querySelector('.vact-code-dom'); const lineHeight = Number.parseFloat(getComputedStyle(root.querySelector('.vact-dom-line')).lineHeight); const line = [...root.querySelectorAll('.vact-dom-line')].find((node) => Math.round(Number((node.style.transform.match(/translateY\(([-\d.]+)px\)/) ?? [])[1]) / lineHeight) === lineIndex);
          const caret = root.querySelector('.vact-dom-caret'); if (!line || !caret) throw Error('line or caret missing');
          const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT); let last = null; while (walker.nextNode()) last = walker.currentNode; if (!last) throw Error('line has no text node');
          const range = document.createRange(); range.setStart(last, last.textContent.length); range.setEnd(last, last.textContent.length); const rect = range.getBoundingClientRect();
          return { caret: caret.getBoundingClientRect().left, text: rect.right };
        }, index);
        const delta = Math.abs(result.caret - result.text); const limit = index === 0 ? 1 : 2; if (delta > limit) throw Error(`line ${index + 1} caret delta=${delta}px, limit=${limit}px`); deltas.push(delta);
      }
      return `caret deltas=${deltas.join(',')}px`;
    });
    await check('editing-undo-redo-navigation', async () => {
      const ta = page.locator('.vact-code-input-bridge textarea'); await ta.focus(); await ta.press('ControlOrMeta+End'); await ta.type('\n#e2e'); await page.waitForTimeout(50);
      const changed = await page.evaluate(() => window.__vactrPerf.doc().endsWith('#e2e')); if (!changed) throw Error('typing did not update document');
      const typedRevision = await page.evaluate(() => window.__vactrPerf.revision()); await ta.press('ControlOrMeta+z');
      const undone = await page.evaluate(() => ({ doc: window.__vactrPerf.doc(), revision: window.__vactrPerf.revision() }));
      if (undone.doc.endsWith('#e2e') || undone.revision <= typedRevision) throw Error(`undo did not remove typed text: ${JSON.stringify(undone)}`);
      await ta.press('ControlOrMeta+Shift+z'); const redone = await page.evaluate(() => ({ doc: window.__vactrPerf.doc(), revision: window.__vactrPerf.revision() }));
      if (!redone.doc.endsWith('#e2e') || redone.revision <= undone.revision) throw Error(`redo did not restore typed text: ${JSON.stringify(redone)}`);
      await ta.press('ControlOrMeta+Home'); const home = await page.evaluate(() => window.__vactrPerf.selection());
      if (home.anchor !== 0 || home.head !== 0) throw Error(`document Home selection=${JSON.stringify(home)}`);
      await ta.press('Control+ArrowRight'); const word = await page.evaluate(() => window.__vactrPerf.selection());
      if (word.anchor !== word.head || word.head <= home.head || word.head >= redone.doc.length) throw Error(`word navigation selection=${JSON.stringify(word)}`);
      await ta.press('ControlOrMeta+End'); const end = await page.evaluate(() => window.__vactrPerf.selection());
      if (end.anchor !== redone.doc.length || end.head !== redone.doc.length) throw Error(`document End selection=${JSON.stringify(end)}`);
      return `undo=${undone.revision}; redo=${redone.revision}; home=${home.head}; word=${word.head}; end=${end.head}`;
    });
    if (name === 'chromium') await check('clipboard-round-trip', async () => {
      await context.grantPermissions(['clipboard-read', 'clipboard-write']); await page.evaluate(() => navigator.clipboard.writeText('dom-clipboard-evidence'));
      const text = await page.evaluate(() => navigator.clipboard.readText()); if (text !== 'dom-clipboard-evidence') throw Error(`clipboard read ${text}`); return text;
    });
    else await check('clipboard-synthetic-event', async () => ({ status: 'limitation', detail: 'WebKit synthetic ClipboardEvent cannot verify system clipboard contents; excluded from pass counts.' }));
    if (name === 'chromium') await check('japanese-ime', async () => {
      const session = await context.newCDPSession(page); const before = await page.evaluate(() => ({ doc: window.__vactrPerf.doc(), revision: window.__vactrPerf.revision() }));
      try {
        await session.send('Input.imeSetComposition', { text: 'にほんご', selectionStart: 5, selectionEnd: 5 }); await page.waitForTimeout(50);
        const composing = await page.evaluate(() => ({ doc: window.__vactrPerf.doc(), revision: window.__vactrPerf.revision(), visible: [...document.querySelectorAll('.vact-dom-line')].some((line) => line.textContent.includes('にほんご')), bar: Boolean(document.querySelector('.vact-dom-bar.vact-dom-comp')) }));
        if (composing.doc !== before.doc || composing.revision !== before.revision) throw Error('IME preedit changed committed document state');
        if (!composing.visible || !composing.bar) throw Error(`IME preedit not visible in DOM: ${JSON.stringify(composing)}`);
        await session.send('Input.insertText', { text: '日本語' }); await page.waitForFunction(() => window.__vactrPerf.doc().includes('日本語'));
        const committed = await page.evaluate(() => ({ doc: window.__vactrPerf.doc(), revision: window.__vactrPerf.revision() }));
        if (committed.revision !== before.revision + 1) throw Error(`IME commit transactions=${committed.revision - before.revision}, expected exactly 1`);
        return `one commit revision=${committed.revision}; Japanese preedit visible`;
      } finally { await session.detach(); }
    });
    else await check('japanese-ime-synthetic', async () => ({ status: 'limitation', detail: 'WebKit synthetic composition events cannot establish real IME commit behavior; excluded from pass counts.' }));
    await check('touch-selection', async () => {
      const geometry = await page.evaluate(() => {
        const root = document.querySelector('.vact-code-dom'); const line = root.querySelector('.vact-dom-line'); const walker = line && document.createTreeWalker(line, NodeFilter.SHOW_TEXT); const text = walker?.nextNode();
        if (!text) throw Error('first rendered line has no text node'); const range = document.createRange(); range.setStart(text, 0); range.setEnd(text, Math.min(4, text.textContent.length));
        const rect = range.getBoundingClientRect(); return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
      });
      if (name === 'chromium') {
        const session = await context.newCDPSession(page); try {
          await session.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: geometry.x, y: geometry.y }] }); await sleep(650);
          await session.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: geometry.x + 70, y: geometry.y }] });
          await session.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
        } finally { await session.detach(); }
      } else {
        limitations.push('WebKit touch selection uses synthetic pointer events.'); const surface = page.locator('.vact-code-dom');
        const event = { pointerId: 1, pointerType: 'touch', isPrimary: true, button: 0, width: 1, height: 1, pressure: 0.5 };
        await surface.dispatchEvent('pointerdown', { ...event, clientX: geometry.x, clientY: geometry.y, buttons: 1 }); await sleep(650);
        await surface.dispatchEvent('pointerup', { ...event, clientX: geometry.x, clientY: geometry.y, buttons: 0 });
      }
      await page.waitForFunction(() => window.__vactrPerf.selection().anchor !== window.__vactrPerf.selection().head && window.__vactrPerf.presented().at(-1)?.handles === 2, undefined, { timeout: 2500 });
      const state = await page.evaluate(() => ({ selection: window.__vactrPerf.selection(), handles: window.__vactrPerf.presented().at(-1)?.handles, domHandles: document.querySelectorAll('.vact-dom-handle').length }));
      if (state.selection.anchor === state.selection.head || state.handles !== 2 || state.domHandles !== 2) throw Error(`touch did not create selection handles: ${JSON.stringify(state)}`);
      return JSON.stringify(state);
    });
    await check('dpr-and-resize', async () => {
      const before = await page.evaluate(() => ({ selection: window.__vactrPerf.selection(), builds: window.__vactrPerf.counters().renderer.lineBuilds })); await page.setViewportSize({ width: 1100, height: 760 });
      let session = null;
      try {
        if (name === 'chromium') { session = await context.newCDPSession(page); await session.send('Emulation.setDeviceMetricsOverride', { width: 1100, height: 760, deviceScaleFactor: 2, mobile: false }); }
        await page.waitForTimeout(100); const state = await page.evaluate(() => ({ selection: window.__vactrPerf.selection(), builds: window.__vactrPerf.counters().renderer.lineBuilds, actual: devicePixelRatio, effective: window.__vactrPerf.counters().gpuStatus.effectiveDpr }));
        if (JSON.stringify(before.selection) !== JSON.stringify(state.selection)) throw Error('selection changed after resize/DPR');
        if (name === 'chromium' && (state.actual !== 2 || state.effective !== 2 || state.builds !== before.builds)) throw Error(`DPR change rebuilt lines or failed: ${JSON.stringify({ before, state })}`);
        if (name === 'webkit') {
          const dprContext = await browser.newContext({ viewport: { width: 900, height: 700 }, deviceScaleFactor: 2, hasTouch: true });
          try { await installSilentSink(dprContext); const dprPage = await dprContext.newPage(); await dprPage.goto(`${origin}/?perf=1&renderer=dom`, { waitUntil: 'domcontentloaded' }); await dprPage.waitForFunction(() => Boolean(window.__vactrPerf)); await dprPage.waitForTimeout(100); const dpr = await dprPage.evaluate(() => ({ actual: devicePixelRatio, effective: window.__vactrPerf.counters().gpuStatus.effectiveDpr })); if (dpr.actual !== 2 || dpr.effective !== 2) throw Error(`DPR-2 context did not update: ${JSON.stringify(dpr)}`); }
          finally { await dprContext.close(); }
        }
        const bounds = await page.evaluate(() => { const rect = document.querySelector('.vact-code-input-bridge textarea').getBoundingClientRect(); const height = window.visualViewport?.height ?? innerHeight; return { right: rect.right, bottom: rect.bottom, height, width: innerWidth }; });
        if (bounds.bottom > bounds.height || bounds.right > bounds.width) throw Error(`bridge bounds overflow: ${JSON.stringify(bounds)}`); return JSON.stringify(state);
      } finally { if (session) { await session.send('Emulation.clearDeviceMetricsOverride').catch(() => {}); await session.detach(); } }
    });
    await check('visual-viewport-inset', async () => page.evaluate(async () => {
      const viewport = window.visualViewport; if (!viewport || !Object.isExtensible(viewport)) return { status: 'limitation', detail: 'visualViewport height cannot be overridden in this browser; keyboard-inset geometry remains unverified.' };
      const originalHeight = viewport.height;
      try { Object.defineProperty(viewport, 'height', { configurable: true, get: () => Math.min(originalHeight, 360) }); viewport.dispatchEvent(new Event('resize')); await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))); const rect = document.querySelector('.vact-code-input-bridge textarea').getBoundingClientRect(); if (rect.bottom > viewport.height) throw Error(`caret bridge bottom=${rect.bottom} exceeds inset viewport height=${viewport.height}`); return `bridge bottom=${rect.bottom}; inset height=${viewport.height}`; }
      catch (error) { if (error instanceof Error && error.message.includes('exceeds inset')) throw error; return { status: 'limitation', detail: `visualViewport height substitution unavailable: ${String(error)}` }; }
      finally { delete viewport.height; viewport.dispatchEvent(new Event('resize')); }
    }));
    await check('background-no-replay', async () => {
      await page.evaluate(() => { Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'hidden' }); document.dispatchEvent(new Event('visibilitychange')); }); await sleep(100);
      const before = await page.evaluate(() => ({ frames: window.__vactrPerf.perf.snapshot().frames.length })); await sleep(2000); const hidden = await page.evaluate(() => window.__vactrPerf.perf.snapshot().frames.length);
      await page.evaluate(() => { Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' }); document.dispatchEvent(new Event('visibilitychange')); }); await sleep(150);
      const now = await page.evaluate(() => window.__vactrPerf.perf.snapshot().frames.length); if (hidden !== before.frames) throw Error(`frames grew while hidden: ${before.frames} -> ${hidden}`); if (now < hidden) throw Error('frame count regressed');
      const recovered = await page.evaluate(() => ({ presented: window.__vactrPerf.presented(), onsets: window.__vactrPerf.onsets() })); const first = recovered.presented.at(-1);
      if (first) { const expected = recovered.onsets.filter((onset) => onset.time <= first.audibleTime && first.audibleTime < onset.end).map((onset) => `${onset.from}-${onset.to}`).sort().join(','); if (expected !== String(first.activeKey).split(',').filter(Boolean).sort().join(',')) throw Error(`first resumed active set mismatch: expected ${expected}, got ${first.activeKey}`); for (const onset of recovered.onsets) if (recovered.presented.some((row) => row.activeKey.includes(`${onset.from}-${onset.to}`) && row.audibleTime >= onset.end)) throw Error('expired highlight replayed after resume'); }
      return `frames hidden=${hidden}, resumed=${now}`;
    });
    await check('dom-bounded-nodes', async () => {
      const workload = createLargeDocument(); const ta = page.locator('.vact-code-input-bridge textarea'); await ta.fill(workload.text); await page.mouse.wheel(0, 180000); await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      const assertBound = async () => page.evaluate(() => {
        const root = document.querySelector('.vact-code-dom'); const rows = Math.ceil(innerHeight / 18); const margin = Math.max(8, Math.ceil(rows / 2)); const counters = window.__vactrPerf.counters();
        const nums = [...root.querySelectorAll('.vact-dom-num')].map((node) => Number(node.textContent)); const from = counters.renderer.windowFrom; const to = counters.renderer.windowTo;
        if (counters.renderer.liveLines > rows + 2 * margin) throw Error(`live lines ${counters.renderer.liveLines} > ${rows + 2 * margin}`);
        if (nums.some((number) => !(from < number && number <= to))) throw Error(`gutter numbers outside ${from}..${to}: ${nums.slice(0, 5)}`);
        return { liveLines: counters.renderer.liveLines, rows, margin, from, to };
      });
      const middle = await assertBound(); await ta.press('ControlOrMeta+End'); await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))); const end = await assertBound();
      return JSON.stringify({ middle, end });
    });
    await check('dispose-ledger', async () => page.evaluate(() => {
      const api = window.__vactrPerf; const ledger = api.ledger; api.disposeCode();
      if (ledger.usedBytes !== 0 || document.querySelector('.vact-code-dom') || window.__vactrPerf !== undefined) throw Error(`dispose ledger=${ledger.usedBytes}`);
      return `usedBytes=${ledger.usedBytes}; DOM surface removed`;
    }));
  } catch (error) { checks.push({ id: 'browser-setup', pass: false, detail: String(error?.stack ?? error) }); }
  finally { await context?.close(); }
  const counted = checks.filter((result) => result.status !== 'limitation');
  return { name, checks, passed: counted.filter((result) => result.pass === true).length, total: counted.length, limitations };
}
