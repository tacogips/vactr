#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { chromium } from 'playwright';
import { installSilentSink, readSinkReport } from './silent-sink.mjs';
import { editorRoot, repoRoot, startServer } from './serve.mjs';

const outputDir = path.join(repoRoot, 'tmp/examples-e2e');
const resultPath = path.join(outputDir, 'result.json');
const checks = [];
const text = (relativePath) => fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');
const showcase = text('examples/showcase.vact');
const industrial = text('examples/industrial-techno.vact');
const fm6 = text('examples/fm6-algorithms.vact');
const technoPattern = text('examples/techno-pattern.vact');
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function document(page) {
  return page.evaluate(() => window.__vactrPerf?.doc() ?? null);
}

async function waitForDocument(page, expected) {
  await page.waitForFunction((value) => window.__vactrPerf?.doc() === value, expected, { timeout: 15000 });
}

async function openPicker(page) {
  const toggle = page.locator('.vact-doc-picker-toggle');
  const panel = page.locator('.vact-doc-picker[role="dialog"]');
  if (!(await panel.isVisible().catch(() => false))) await toggle.click();
  await page.waitForSelector('.vact-doc-picker[role="dialog"]');
  return panel;
}

async function openExampleByKeyboard(page, id, query) {
  const panel = await openPicker(page);
  const filter = panel.getByRole('textbox', { name: 'Filter documents' });
  await filter.fill(query);
  await filter.press('ArrowDown');
  for (let count = 0; count < 10; count += 1) {
    if (await page.evaluate((exampleId) => document.activeElement?.getAttribute('data-example-id') === exampleId, id)) break;
    await page.keyboard.press('ArrowDown');
  }
  if (!(await page.evaluate((exampleId) => document.activeElement?.getAttribute('data-example-id') === exampleId, id))) {
    throw new Error(`keyboard navigation did not reach ${id}`);
  }
  await page.keyboard.press('Enter');
}

async function check(id, action) {
  try {
    const detail = await action();
    checks.push({ id, pass: true, detail });
  } catch (error) {
    checks.push({ id, pass: false, detail: String(error?.stack ?? error) });
  }
}

async function main() {
  if (!fs.existsSync(path.join(editorRoot, 'dist', 'index.html'))) {
    return { pass: false, blocked: true, fatal: `editor dist missing at ${path.join(editorRoot, 'dist')}` };
  }
  let server;
  let browser;
  let context;
  try {
    server = await startServer({ dist: path.join(editorRoot, 'dist') });
    browser = await chromium.launch({ headless: true, args: ['--mute-audio'] });
    context = await browser.newContext();
    await installSilentSink(context);
    const page = await context.newPage();
    await page.goto(`${server.origin}/?perf=1`, { waitUntil: 'domcontentloaded', timeout: 60000 });
    await page.waitForFunction(() => Boolean(window.__vactrPerf), undefined, { timeout: 30000 });
    await page.waitForFunction(() => window.__vactrPerf.doc().length > 0, undefined, { timeout: 30000 });

    await check('first-open-showcase', async () => {
      if (await document(page) !== showcase) throw new Error('first open did not load examples/showcase.vact');
      await sleep(1500);
      const sink = await readSinkReport(page);
      if (!sink || sink.pre.onsetCount !== 0) throw new Error(`startup produced audio onset: ${JSON.stringify(sink?.pre)}`);
      return { docMatchesShowcase: true, onsetCount: sink.pre.onsetCount };
    });

    await check('run-plays-moderately', async () => {
      await page.locator('.vact-audio').click();
      await page.waitForFunction(() => document.querySelector('.vact-audio-state')?.getAttribute('data-state') === 'running', undefined, { timeout: 10000 });
      await page.locator('.vact-run').click();
      const diagnosticState = () => page.evaluate(() => ({
        sink: window.__vactrSink?.report(),
        audio: document.querySelector('.vact-audio-state')?.getAttribute('data-state'),
        evaluation: {
          status: document.querySelector('.vact-eval-status')?.getAttribute('data-status'),
          label: document.querySelector('.vact-eval-status')?.getAttribute('aria-label'),
        },
      }));
      try {
        await page.waitForFunction(() => {
          const status = document.querySelector('.vact-eval-status')?.getAttribute('data-status');
          return status === 'ok' || status === 'diagnostics' || status === 'not-delivered';
        }, undefined, { timeout: 10000 });
      } catch (error) {
        const state = await diagnosticState();
        await page.locator('.vact-hush').click();
        throw new Error(`evaluation readiness was not reported: ${JSON.stringify(state)}; ${String(error)}`);
      }
      const evaluationState = await diagnosticState();
      if (evaluationState.evaluation.status !== 'ok') {
        await page.locator('.vact-hush').click();
        throw new Error(`evaluation did not become ready: ${JSON.stringify(evaluationState)}`);
      }
      try {
        await page.waitForFunction(() => window.__vactrSink?.report()?.pre.onsetCount >= 1, undefined, { timeout: 10000 });
      } catch (error) {
        const state = await diagnosticState();
        await page.locator('.vact-hush').click();
        throw new Error(`no playback onset within 10 seconds: ${JSON.stringify(state)}; ${String(error)}`);
      }
      await sleep(250);
      const sink = await readSinkReport(page);
      await page.locator('.vact-hush').click();
      if (sink.pre.peakDbfs > -1) throw new Error(`pre-sink peak ${sink.pre.peakDbfs} dBFS exceeds -1 dBFS`);
      if (sink.post.peak !== 0) throw new Error(`post-sink peak was ${sink.post.peak}`);
      if (sink.directDestinationConnections !== 0) throw new Error(`direct destination connections=${sink.directDestinationConnections}`);
      return { peakDbfs: sink.pre.peakDbfs, onsetCount: sink.pre.onsetCount, postPeak: sink.post.peak, directDestinationConnections: sink.directDestinationConnections };
    });

    await check('picker-keyboard-open-undo', async () => {
      await page.locator('.vact-doc-picker-toggle').focus();
      await page.keyboard.press('Enter');
      await page.waitForSelector('.vact-doc-picker-filter');
      await page.waitForFunction(() => document.activeElement?.matches('.vact-doc-picker-filter'));
      await openExampleByKeyboard(page, 'industrial-techno', 'Industrial techno');
      await waitForDocument(page, industrial);
      const input = page.locator('.vact-code-input-bridge textarea');
      await input.focus();
      await input.press('ControlOrMeta+z');
      await waitForDocument(page, showcase);
      await input.press('ControlOrMeta+Shift+z');
      await waitForDocument(page, industrial);
      return { opened: 'industrial-techno', undoRestoredShowcase: true, redoRestoredExample: true };
    });

    let markerText = '';
    await check('draft-restore', async () => {
      const input = page.locator('.vact-code-input-bridge textarea');
      await input.focus();
      await input.press('ControlOrMeta+End');
      await input.type('\n# e2e-draft-marker');
      markerText = `${industrial}\n# e2e-draft-marker`;
      await waitForDocument(page, markerText);
      await sleep(1500);
      await page.reload({ waitUntil: 'domcontentloaded' });
      await page.waitForFunction(() => Boolean(window.__vactrPerf), undefined, { timeout: 30000 });
      await waitForDocument(page, markerText);
      return { restored: true, marker: 'e2e-draft-marker' };
    });

    let savedText = '';
    await check('library', async () => {
      const panel = await openPicker(page);
      await panel.getByRole('button', { name: 'Save as...' }).click();
      await panel.getByRole('textbox', { name: 'Document name' }).fill('e2e-one');
      await panel.locator('form.vact-doc-picker-form').getByRole('button', { name: 'Save' }).click();
      await panel.getByRole('button', { name: /e2e-one/ }).waitFor();
      savedText = await document(page);
      await panel.locator('[data-example-id="industrial-techno"]').click();
      await waitForDocument(page, industrial);
      const input = page.locator('.vact-code-input-bridge textarea');
      await input.focus();
      await input.press('ControlOrMeta+End');
      await input.type('\n# dirty');
      const replacementPanel = await openPicker(page);
      await replacementPanel.getByRole('button', { name: /e2e-one/ }).click();
      await replacementPanel.locator('.vact-doc-picker-confirm').waitFor();
      await replacementPanel.locator('.vact-doc-picker-confirm').getByRole('button', { name: 'Replace' }).click();
      await waitForDocument(page, savedText);
      const libraryPanel = await openPicker(page);
      const row = libraryPanel.locator('.vact-doc-picker-library-row').filter({ hasText: 'e2e-one' });
      await row.getByRole('button', { name: 'Rename' }).click();
      await libraryPanel.getByRole('textbox', { name: 'New document name' }).fill('e2e-two');
      await row.locator('form').getByRole('button', { name: 'Save' }).click();
      await libraryPanel.getByRole('button', { name: /e2e-two/ }).waitFor();
      const renamedRow = libraryPanel.locator('.vact-doc-picker-library-row').filter({ hasText: 'e2e-two' });
      await renamedRow.getByRole('button', { name: 'Delete' }).click();
      await renamedRow.getByRole('button', { name: 'Confirm delete' }).click();
      await libraryPanel.getByRole('button', { name: /e2e-two/ }).waitFor({ state: 'detached' });
      return { saved: true, openedWithDirtyConfirmation: true, renamed: true, deleted: true };
    });

    await check('default-template', async () => {
      const panel = await openPicker(page);
      await panel.locator('[data-example-id="fm6-algorithms"]').click();
      await waitForDocument(page, fm6);
      const templatePanel = await openPicker(page);
      await templatePanel.getByRole('button', { name: 'Use as default template' }).click();
      await page.waitForFunction(() => document.querySelector('.vact-doc-picker-status')?.textContent.includes('Default template set'));
      await templatePanel.locator('[data-example-id="industrial-techno"]').click();
      await waitForDocument(page, industrial);
      const newPanel = await openPicker(page);
      await newPanel.getByRole('button', { name: 'New', exact: true }).click();
      await waitForDocument(page, fm6);
      await page.reload({ waitUntil: 'domcontentloaded' });
      await page.waitForFunction(() => Boolean(window.__vactrPerf), undefined, { timeout: 30000 });
      await waitForDocument(page, fm6);
      const reopened = await openPicker(page);
      await reopened.getByRole('button', { name: 'Reset default template' }).click();
      await page.waitForFunction(() => document.querySelector('.vact-doc-picker-status')?.textContent.includes('Default template reset'));
      await reopened.getByRole('button', { name: 'New', exact: true }).click();
      await waitForDocument(page, showcase);
      return { persistedAcrossReload: true, resetReturnsToShowcase: true };
    });

    await check('explicit-example-url-draft-recovery', async () => {
      const input = page.locator('.vact-code-input-bridge textarea');
      await input.focus();
      await input.press('ControlOrMeta+End');
      await input.type('\n# explicit-url-draft-marker');
      markerText = `${showcase}\n# explicit-url-draft-marker`;
      await waitForDocument(page, markerText);
      await sleep(1500);
      await page.goto(`${server.origin}/?perf=1&example=techno-pattern`, { waitUntil: 'domcontentloaded' });
      await page.waitForFunction(() => Boolean(window.__vactrPerf), undefined, { timeout: 30000 });
      await waitForDocument(page, technoPattern);
      const status = await page.locator('.vact-doc-picker-status').textContent();
      if (!status?.includes('restores your unsaved draft')) throw new Error(`draft recovery status missing: ${status}`);
      await page.locator('.vact-code-input-bridge textarea').focus();
      await page.keyboard.press('ControlOrMeta+z');
      await waitForDocument(page, markerText);
      await sleep(1500);
      await page.goto(`${server.origin}/?perf=1`, { waitUntil: 'domcontentloaded' });
      await page.waitForFunction(() => Boolean(window.__vactrPerf), undefined, { timeout: 30000 });
      await waitForDocument(page, markerText);
      return { urlExample: 'techno-pattern', undoRestoredDraft: true, autosaveRewroteDraft: true };
    });

    await context.close();
    context = await browser.newContext();
    await installSilentSink(context);
    await context.addInitScript(() => {
      Object.defineProperty(window, 'localStorage', { configurable: false, get() { throw new DOMException('blocked', 'SecurityError'); } });
    });
    const unavailablePage = await context.newPage();
    await unavailablePage.goto(`${server.origin}/?perf=1`, { waitUntil: 'domcontentloaded' });
    await unavailablePage.waitForFunction(() => Boolean(window.__vactrPerf), undefined, { timeout: 30000 });
    await unavailablePage.waitForFunction(() => window.__vactrPerf.doc().length > 0, undefined, { timeout: 30000 });
    await check('storage-unavailable', async () => {
      if (await document(unavailablePage) !== showcase) throw new Error('unavailable storage prevented showcase startup');
      const panel = await openPicker(unavailablePage);
      const notice = await panel.locator('.vact-doc-picker-notice').textContent();
      if (!notice?.includes('Browser storage is unavailable')) throw new Error(`storage warning missing: ${notice}`);
      const sink = await readSinkReport(unavailablePage);
      if (sink.post.peak !== 0 || sink.directDestinationConnections !== 0) throw new Error(`silent sink invariant failed: ${JSON.stringify(sink)}`);
      return { openedShowcase: true, notice, postPeak: sink.post.peak, directDestinationConnections: sink.directDestinationConnections };
    });
  } catch (error) {
    return { pass: false, blocked: /Executable doesn't exist|browserType\.launch/.test(String(error)) || !fs.existsSync(path.join(editorRoot, 'dist', 'index.html')), fatal: String(error?.stack ?? error) };
  } finally {
    await context?.close().catch(() => {});
    await browser?.close().catch(() => {});
    await server?.close().catch(() => {});
  }
  return { pass: checks.length === 8 && checks.every((item) => item.pass), blocked: false, checks };
}

fs.mkdirSync(outputDir, { recursive: true });
const result = await main();
result.testsRun = checks.length;
result.testsPassed = checks.filter((item) => item.pass).length;
result.failureCount = checks.filter((item) => !item.pass).length;
result.checks = checks;
fs.writeFileSync(resultPath, `${JSON.stringify(result, null, 2)}\n`);
console.log(JSON.stringify({ pass: result.pass, blocked: result.blocked, testsRun: result.testsRun, testsPassed: result.testsPassed, failureCount: result.failureCount, resultPath, checks }));
if (result.blocked) process.exitCode = 2;
else if (!result.pass) process.exitCode = 1;
