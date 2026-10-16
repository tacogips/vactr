#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { chromium } from 'playwright';
import { startServer, repoRoot } from './serve.mjs';
import { installSilentSink } from './silent-sink.mjs';

const scenarios = (kalimba) => [
  { id: 'S1', text: 'use-bpm 108', type: true },
  { id: 'S2', text: '# hello\nabc def', type: true },
  { id: 'S3', text: '# c\nuse-bpm 108\ns :kalimbo > note :c5' }, // :kalimbo is deliberately unknown so P6 has a diagnostic to place
  { id: 'S4', text: kalimba },
];

async function settle(page) {
  await page.waitForFunction(() => !window.__vactrPerf.counters().textPending, { timeout: 10000 });
  await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  await page.waitForTimeout(300);
}

async function compareCaptures(analysisPage, canvasPng, refPng, blinkPngs, charsByLine, dpr, popupRects, state) {
  return analysisPage.evaluate(async ({ canvasPng, refPng, blinkPngs, charsByLine, dpr, popupRects, state }) => {
    const decode = async (base64) => {
      const image = new Image(); image.src = `data:image/png;base64,${base64}`; await image.decode();
      const canvas = document.createElement('canvas'); canvas.width = image.naturalWidth; canvas.height = image.naturalHeight;
      const context = canvas.getContext('2d', { willReadFrequently: true }); context.drawImage(image, 0, 0);
      return { data: new Uint8ClampedArray(context.getImageData(0, 0, canvas.width, canvas.height).data), width: canvas.width, height: canvas.height };
    };
    const api = window.__pixelCompare; const c = await decode(canvasPng); const r = await decode(refPng); const checks = [];
    for (let line = 0; line < charsByLine.length; line++) {
      const row = { left: 0, top: line * 18 * dpr, right: c.width, bottom: Math.min(c.height, (line + 1) * 18 * dpr - 2 * dpr) };
      const cb = api.modeColor(c, row); const rb = api.modeColor(r, row); const chars = charsByLine[line].filter((box) => box !== null);
      const cells = chars.map((box) => api.cellInk(c, { left: box.left, top: row.top, right: box.right, bottom: row.bottom }, cb));
      const refs = chars.map((box) => api.cellInk(r, { left: box.left, top: row.top, right: box.right, bottom: row.bottom }, rb));
      const result = api.compareLine(cells, refs, chars.map((box) => box.char), { minInk: 0.5, extentPx: 1, ratio: [0.8, 1.25] });
      checks.push({ id: `line-${line + 1}`, failures: result.failures, inkRatio: result.inkRatio, comparedCells: chars.length });
    }
    const prefix = state.text.slice(0, state.caret).split('\n'); const caretLine = prefix.length - 1; const caretColumn = Array.from(prefix.at(-1)).length;
    const caretX = caretColumn === 0 ? 48 * dpr : (charsByLine[caretLine]?.[caretColumn - 1]?.right ?? 48 * dpr);
    const caretColumns = [...new Set([caretX - 1, caretX, caretX + 1].map((x) => Math.max(0, Math.min(c.width - 1, x))))];
    const blink = await Promise.all(blinkPngs.map(decode));
    const cursorColor = state.cursorColor;
    const caretRuns = blink.map((img) => caretColumns.flatMap((x) => api.verticalRuns(img, x, cursorColor, 40)
      .filter(([a, b]) => b - a >= 0.8 * 18 * dpr).map(([top, bottom]) => ({ top, bottom, x }))));
    const p5 = caretRuns.some((runs) => runs.some(({ top, bottom }) => top < (caretLine + 1) * 18 * dpr && bottom > caretLine * 18 * dpr)) &&
      caretRuns.every((runs) => runs.every(({ top, bottom }) => top >= caretLine * 18 * dpr && bottom <= (caretLine + 1) * 18 * dpr));
    const danger = state.dangerColor;
    const p6 = state.annotations.filter((a) => a.kind === 'diagnostic').map((annotation) => {
      const row = state.text.slice(0, annotation.from).split('\n').length - 1;
      const start = annotation.from - state.text.lastIndexOf('\n', annotation.from - 1) - 1;
      const end = annotation.to - state.text.lastIndexOf('\n', annotation.to - 1) - 1;
      const selected = api.selectDiagnosticRect(charsByLine, row, start, end, dpr);
      if (selected.skipped) return { line: row, skipped: true, rows: [], pass: true };
      const rect = selected.rect;
      const rows = api.bandRows(c, rect, danger, 40);
      return { line: row, skipped: false, rows, pass: rows.length > 0 && rows.every((y) => y >= rect.bottom - 2 * dpr && y < rect.bottom) };
    });
    const p7 = [8, 40, 80].map((margin) => {
      const x = c.width - Math.round(margin * dpr); const covered = popupRects.some((rect) => x >= rect.left && x < rect.right);
      return covered ? null : api.columnUniform(c, Math.max(0, x), Math.min(c.height, charsByLine.length * 18 * dpr), c.height, 2);
    });
    return { checks, p5, caretLine, caretColumn, caretX, caretColumns, p6, p7 };
  }, { canvasPng: canvasPng.toString('base64'), refPng: refPng.toString('base64'), blinkPngs: blinkPngs.map((b) => b.toString('base64')), charsByLine, dpr, popupRects, state });
}

export async function runCodePixels(browser, origin, { dpr, expectBackend = 'metal', scenarios: selected, outDir } = {}) {
  const kalimba = fs.readFileSync(path.join(repoRoot, 'examples/kalimba.vact'), 'utf8');
  const inputs = selected ?? scenarios(kalimba); const report = { dpr, expectBackend, scenarios: [], tolerances: { p3ExtentPx: 1, p4Ratio: [0.8, 1.25] } };
  const failures = []; let checkedDiagnostics = 0;
  const analysis = await browser.newPage(); await analysis.goto('about:blank');
  const helperSource = fs.readFileSync(new URL('./pixel-compare.mjs', import.meta.url), 'utf8').replaceAll('export function ', 'function ');
  await analysis.addScriptTag({ content: `${helperSource}\nwindow.__pixelCompare={modeColor,cellInk,compareLine,verticalRuns,bandRows,columnUniform,selectDiagnosticRect};` });
  try {
    for (const scenario of inputs) {
      const context = await browser.newContext({ viewport: { width: 1100, height: 700 }, deviceScaleFactor: dpr });
      await installSilentSink(context); const page = await context.newPage(); const consoleErrors = [];
      page.on('console', (message) => { if (/GL_INVALID|WebGL: INVALID|GL ERROR/.test(message.text())) consoleErrors.push(message.text()); });
      page.on('pageerror', (error) => consoleErrors.push(String(error)));
      try {
        await page.goto(`${origin}/?perf=1`, { waitUntil: 'domcontentloaded', timeout: 60000 });
        await page.waitForFunction(() => Boolean(window.__vactrPerf), { timeout: 30000 });
        await page.waitForSelector('.vact-code-canvas', { timeout: 30000 });
        let syntaxTimeout = false;
        try { await page.waitForFunction(() => document.querySelector('[data-pane="code"]')?.dataset.syntax === 'tree-sitter-worker', { timeout: 10000 }); } catch { syntaxTimeout = true; }
        const canvas = page.locator('.vact-code-canvas'); const bounds = await canvas.boundingBox();
        if (!bounds) throw new Error('code canvas has no bounds');
        await canvas.click({ position: { x: 200, y: 10 } });
        // Startup opens the default example; scenarios start from an empty document.
        try { await page.waitForFunction(() => window.__vactrPerf.doc().length > 0, { timeout: 10000 }); } catch { /* empty startup */ }
        await page.keyboard.press('ControlOrMeta+a'); await page.keyboard.press('Backspace');
        await page.waitForFunction(() => window.__vactrPerf.doc() === '', { timeout: 10000 });
        await settle(page);
        if (scenario.type) {
          const keyboard = page.keyboard; await keyboard.type(scenario.id === 'S1' ? scenario.text : scenario.text.split('\n')[0], { delay: 20 });
          if (scenario.id === 'S2') { await keyboard.press('Enter'); await keyboard.type('abc def', { delay: 20 }); }
        } else await page.keyboard.insertText(scenario.text);
        await settle(page);
        let diagnosticWaitMs = 0; let diagnosticWaitTimedOut = false;
        if (scenario.id === 'S2' || scenario.id === 'S3') {
          const diagnosticWaitStarted = Date.now();
          try { await page.waitForFunction(() => window.__vactrPerf.annotations().some((a) => a.kind === 'diagnostic'), { timeout: 5000 }); }
          catch { diagnosticWaitTimedOut = true; }
          diagnosticWaitMs = Date.now() - diagnosticWaitStarted;
          await settle(page);
        }
        const state = await page.evaluate(() => {
          const sample = (value) => { const probe = document.createElement('span'); probe.style.color = `var(${value})`; document.body.append(probe); const color = getComputedStyle(probe).color.match(/[\d.]+/g)?.slice(0, 3).map(Number) ?? [255, 255, 255]; probe.remove(); return color; };
          return { text: window.__vactrPerf.doc(), caret: window.__vactrPerf.selection().head, annotations: window.__vactrPerf.annotations(),
          backend: (() => { const gl = document.querySelector('.vact-code-canvas').getContext('webgl2'); const ext = gl.getExtension('WEBGL_debug_renderer_info'); return ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : 'unknown'; })(),
          completionVisible: (() => { const popup = document.querySelector('.vact-completion'); if (!popup) return false; const rect = popup.getBoundingClientRect(); const style = getComputedStyle(popup); return rect.width > 0 && rect.height > 0 && style.display !== 'none' && style.visibility !== 'hidden'; })(),
          cursorColor: sample('--vt-text'), dangerColor: sample('--vt-danger') };
        });
        const documentMatch = state.text === scenario.text;
        if (!documentMatch) failures.push(`${scenario.id} document mismatch: expected ${JSON.stringify(scenario.text)}, got ${JSON.stringify(state.text)}`);
        if (scenario.id === 'S2' && !state.completionVisible) failures.push('S2 completion popup is not visible at capture time');
        if (expectBackend && !state.backend.toLowerCase().includes(expectBackend === 'metal' ? 'metal' : 'swiftshader')) throw Object.assign(new Error(`backend mismatch: expected ${expectBackend}, got ${state.backend}`), { blocked: true });
        const clip = { x: Math.max(0, bounds.x), y: Math.max(0, bounds.y), width: Math.min(bounds.width, 1100 - bounds.x), height: Math.min(bounds.height, 700 - bounds.y) };
        const canvasPng = await page.screenshot({ clip });
        const shape = await page.evaluate(({ text, clip, dpr }) => {
          const overlay = document.createElement('div'); overlay.setAttribute('aria-hidden', 'true');
          Object.assign(overlay.style, { position: 'fixed', left: `${clip.x + 48}px`, top: `${clip.y}px`, width: `${clip.width - 48}px`, height: `${clip.height}px`, zIndex: '2147483647', pointerEvents: 'none', color: '#fff', background: '#000', font: '13px ui-monospace, SFMono-Regular, Menlo, monospace' });
          const rows = text.split('\n'); const textNodes = [];
          for (const row of rows) { const div = document.createElement('div'); Object.assign(div.style, { height: '18px', lineHeight: '18px', whiteSpace: 'pre' }); const node = document.createTextNode(row); div.append(node); textNodes.push(node); overlay.append(div); }
          document.body.append(overlay);
          const obscurers = [...document.querySelectorAll('.vact-completion,.vact-code-diag-tooltip')].map((el) => el.getBoundingClientRect()).filter((r) => r.width && r.height);
          const chars = rows.map((row) => Array.from(row).map(() => null)); let skipCount = 0;
          for (let line = 0; line < rows.length; line++) {
            let offset = 0;
            for (let col = 0; col < Array.from(rows[line]).length; col++) {
            const char = Array.from(rows[line])[col]; const range = document.createRange(); range.setStart(textNodes[line], offset); range.setEnd(textNodes[line], offset + char.length); offset += char.length;
            const r = range.getBoundingClientRect(); const clipped = r.right > clip.x + clip.width - 8;
            const obscured = obscurers.some((o) => r.left < o.right && r.right > o.left && r.top < o.bottom && r.bottom > o.top);
            if (clipped || obscured) { skipCount++; continue; }
            chars[line][col] = { left: Math.round((r.left - clip.x) * dpr), right: Math.round((r.right - clip.x) * dpr), char };
            }
          }
          const popupRects = obscurers.map((r) => ({ left: Math.round((r.left - clip.x) * dpr), right: Math.round((r.right - clip.x) * dpr) }));
          return { chars, skipCount, popupRects };
        }, { text: state.text, clip, dpr });
        const referencePng = await page.screenshot({ clip });
        await page.evaluate(() => document.querySelector('[aria-hidden="true"][style*="2147483647"]')?.remove());
        const blinkPngs = [];
        for (let i = 0; i < 6; i++) { await page.waitForTimeout(150); blinkPngs.push(await page.screenshot({ clip })); }
        const compare = await compareCaptures(analysis, canvasPng, referencePng, blinkPngs, shape.chars, dpr, shape.popupRects, state);
        const lineChecks = compare.checks;
        for (const check of lineChecks) for (const failure of check.failures) failures.push(`${scenario.id} ${check.id} ${failure}`);
        const diag = state.annotations.filter((a) => a.kind === 'diagnostic');
        const diagRows = diag.map((a) => Math.max(0, state.text.slice(0, a.from).split('\n').length - 1));
        if (!compare.p5) failures.push(`${scenario.id} P5 caret not isolated on expected row`);
        const diagnosticSkipped = compare.p6.filter((check) => check.skipped).length;
        const visibleDiagnosticChecks = compare.p6.filter((check) => !check.skipped).length;
        checkedDiagnostics += visibleDiagnosticChecks;
        if (scenario.id === 'S2' || scenario.id === 'S3') {
          if (!diag.length || !visibleDiagnosticChecks) failures.push(`${scenario.id} P6 no diagnostic to check`);
        }
        for (const check of compare.p6) if (!check.pass) failures.push(`${scenario.id} P6 diagnostic underline rows ${check.rows} outside line ${check.line + 1}`);
        if (compare.p7.some((uniform) => uniform === false)) failures.push(`${scenario.id} P7 backdrop is not uniform below the document`);
        if (consoleErrors.length) failures.push(`${scenario.id} P8 ${consoleErrors.join('; ')}`);
        if (outDir) {
          fs.mkdirSync(outDir, { recursive: true }); fs.writeFileSync(path.join(outDir, `${scenario.id}-dpr${dpr}-canvas.png`), canvasPng); fs.writeFileSync(path.join(outDir, `${scenario.id}-dpr${dpr}-reference.png`), referencePng);
        }
        report.scenarios.push({ id: scenario.id, documentLength: state.text.length, expectedDocumentLength: scenario.text.length, documentMatch, completionVisible: state.completionVisible, backend: state.backend, syntaxTimeout, caret: state.caret,
          caretCheck: { pass: compare.p5, line: compare.caretLine, column: compare.caretColumn, caretX: compare.caretX, columns: compare.caretColumns }, skippedCells: shape.skipCount,
          diagnostics: diag.length, diagnosticWaitMs, diagnosticWaitTimedOut, diagnosticSkipped, diagnosticLines: diagRows, diagnosticChecks: compare.p6, backdropUniform: compare.p7, checks: lineChecks, consoleErrors });
      } finally { await context.close(); }
    }
  } finally { await analysis.close(); }
  if (checkedDiagnostics === 0) failures.push('P6 no diagnostic checked across S2/S3');
  report.pass = failures.length === 0; report.failures = failures;
  if (outDir) fs.writeFileSync(path.join(outDir, 'report.json'), JSON.stringify(report, null, 2) + '\n');
  if (failures.length) throw new Error(failures.join('\n'));
  return `DPR ${dpr}; ${report.scenarios.length} scenarios; backend ${expectBackend}`;
}

async function cli() {
  const args = process.argv.slice(2); const value = (key, fallback) => { const i = args.indexOf(key); return i >= 0 ? args[i + 1] : fallback; };
  const out = path.resolve(value('--out', '../tmp/code-render/pixels')); const backendArg = value('--backend', 'all'); const dprArg = value('--dpr', 'all');
  let server; let testsRun = 0; let testsPassed = 0; let failureCount = 0; let blocked = false; const rows = [];
  try {
    const dist = path.resolve(value('--dist', path.join(repoRoot, 'editor/dist'))); server = await startServer({ dist });
    const backends = backendArg === 'all' ? ['metal', 'swiftshader'] : [backendArg]; const dprs = dprArg === 'all' ? [1, 2] : [Number(dprArg)];
    for (const backend of backends) for (const dpr of dprs) {
      testsRun++; let browser;
      try {
        browser = await chromium.launch({ headless: true, timeout: 30000, args: backend === 'metal' ? ['--mute-audio', '--use-angle=metal', '--enable-gpu', '--ignore-gpu-blocklist'] : ['--mute-audio', '--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
        const detail = await runCodePixels(browser, server.origin, { dpr, expectBackend: backend, outDir: path.join(out, `${backend}-dpr${dpr}`) });
        testsPassed++; rows.push({ backend, dpr, pass: true, detail });
      } catch (error) { failureCount++; blocked ||= Boolean(error.blocked) || /Executable doesn't exist|browserType\.launch|dist missing/.test(String(error)); rows.push({ backend, dpr, pass: false, detail: String(error?.stack ?? error) }); }
      finally { await browser?.close().catch(() => {}); }
    }
  } catch (error) { blocked = true; failureCount++; rows.push({ pass: false, detail: String(error?.stack ?? error) }); }
  finally { await server?.close(); }
  fs.mkdirSync(out, { recursive: true }); const report = { pass: failureCount === 0, blocked, testsRun, testsPassed, failureCount, rows };
  fs.writeFileSync(path.join(out, 'report.json'), JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify({ ...report, report: path.join(out, 'report.json') }));
  process.exitCode = blocked ? 2 : failureCount ? 1 : 0;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) await cli();
