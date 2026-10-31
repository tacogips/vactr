import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium, webkit } from 'playwright';
import { distReady, ORIGIN, routeDist } from './route-server.mjs';

const args = process.argv.slice(2);
let browser = 'all', out = 'perf-results/e2e';
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--browser' && args[i + 1]) browser = args[++i];
  else if (args[i] === '--out' && args[i + 1]) out = args[++i];
  else { console.error(`Unknown or incomplete argument: ${args[i]}`); process.exit(2); }
}
if (!['all', 'chromium', 'webkit'].includes(browser)) {
  console.error(`Invalid browser: ${browser}`); process.exit(2);
}
if (!distReady('dist/diff')) {
  console.error('dist/diff/index.html is missing; run npm run build first'); process.exit(2);
}

const results = [];
let unavailable = false;
for (const name of browser === 'all' ? ['chromium', 'webkit'] : [browser]) {
  const checks = [];
  const check = (name, ok, detail = '') => checks.push({ name, ok, detail });
  let instance;
  try {
    instance = await (name === 'chromium' ? chromium : webkit).launch(name === 'chromium' ? { args: ['--mute-audio'] } : {});
  } catch (error) {
    unavailable = true;
    check('browser launch', false, String(error));
    results.push({ browser: name, checks, testsRun: 1, testsPassed: 0, failureCount: 1 });
    continue;
  }
  const errors = [];
  try {
    const context = await instance.newContext();
    await routeDist(context, 'dist');
    const page = await context.newPage();
    page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
    page.on('pageerror', error => errors.push(String(error)));
    const response = await page.goto(ORIGIN + '/diff/index.html?lines=400');
    if (!response?.ok()) throw new Error(`HTTP ${response?.status() ?? 'no response'}`);
    await page.waitForFunction(() => window.__emakiDiff?.stats().diffRevision > 0);
    const initial = await page.evaluate(() => window.__emakiDiff.stats());
    // The fixture contains 400 lines; the working copy deletes one and inserts two.
    const aligned = initial.fillerRowsLeft + 400 === initial.fillerRowsRight + 401;
    check('initial diff and alignment', initial.removed >= 1 && initial.added >= 2 && initial.modified >= 1 && aligned, JSON.stringify(initial));

    const rightCanvas = page.locator('[data-diff-pane="right"] canvas.emaki-canvas');
    await rightCanvas.click({ position: { x: 80, y: 4 * 18 + 9 } });
    await page.keyboard.press('End');
    await page.evaluate(() => {
      window.__diffLastPresented = -1;
      window.__emakiDiff.onApplied(event => { window.__diffLastPresented = event.rightRevision; });
    });
    await page.keyboard.type('zzz');
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    await page.waitForFunction(previous => {
      const current = window.__emakiDiff.stats();
      return current.diffRevision > previous.diffRevision && current.rightRevision > previous.rightRevision && window.__diffLastPresented >= current.rightRevision;
    }, initial);
    const typed = await page.evaluate(() => window.__emakiDiff.stats());
    check('typing updates word marks', typed.modified === initial.modified + 1 && typed.wordMarksRight >= 1, JSON.stringify(typed));

    await page.keyboard.press('Enter');
    await page.keyboard.type('new line');
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    await page.waitForFunction(previous => {
      const current = window.__emakiDiff.stats();
      return current.diffRevision > previous.diffRevision && current.rightRevision > previous.rightRevision && window.__diffLastPresented >= current.rightRevision;
    }, typed);
    const entered = await page.evaluate(() => window.__emakiDiff.stats());
    check('insertion adds an aligned row', entered.added === typed.added + 1 && entered.fillerRowsLeft === typed.fillerRowsLeft + 1, JSON.stringify(entered));

    await page.locator('[data-diff-pane="left"] canvas.emaki-canvas').hover();
    await page.mouse.wheel(0, 600);
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const tops = await page.evaluate(() => window.__emakiDiff.scrollTops());
    check('synchronized scrolling', tops[0] > 0 && tops[0] === tops[1], JSON.stringify(tops));
    await context.close();
  } catch (error) { check('browser execution', false, String(error)); }
  finally { await instance.close(); }
  check('no console or page errors', errors.length === 0, errors.join('\n'));
  const testsPassed = checks.filter(item => item.ok).length;
  results.push({ browser: name, checks, testsRun: checks.length, testsPassed, failureCount: checks.length - testsPassed });
}
const totals = results.reduce((sum, item) => ({ testsRun: sum.testsRun + item.testsRun, testsPassed: sum.testsPassed + item.testsPassed, failureCount: sum.failureCount + item.failureCount }), { testsRun: 0, testsPassed: 0, failureCount: 0 });
mkdirSync(resolve(out), { recursive: true });
writeFileSync(resolve(out, 'result.json'), JSON.stringify({ browsers: results, ...totals }, null, 2) + '\n');
console.log(JSON.stringify({ browsers: results, ...totals }, null, 2));
process.exit(unavailable ? 2 : totals.failureCount ? 1 : 0);
