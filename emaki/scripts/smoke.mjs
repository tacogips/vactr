import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium, webkit } from 'playwright';
import { distReady, ORIGIN, routeDist } from './route-server.mjs';

const args = process.argv.slice(2);
let browser = 'all', out = 'perf-results/smoke';
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--browser' && args[i + 1]) browser = args[++i];
  else if (args[i] === '--out' && args[i + 1]) out = args[++i];
  else { console.error(`Unknown or incomplete argument: ${args[i]}`); process.exit(2); }
}
if (!['all', 'chromium', 'webkit'].includes(browser)) {
  console.error(`Invalid browser: ${browser}`); process.exit(2);
}
if (!distReady('dist')) { console.error('dist/index.html is missing; run npm run build first'); process.exit(2); }

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
    results.push({ browser: name, checks, testsRun: checks.length, testsPassed: 0, failureCount: checks.length });
    continue;
  }
  try {
    const context = await instance.newContext();
    await routeDist(context, 'dist');
    const page = await context.newPage();
    const errors = [];
    page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
    page.on('pageerror', error => errors.push(String(error)));
    for (const [label, path, placeholder] of [
      ['index', '/index.html', false], ['showcase', '/plugin-showcase/', false],
      ['json', '/json-10k/?lines=1000', true], ['diff', '/diff/?lines=400', true],
    ]) {
      try {
        const response = await page.goto(ORIGIN + path);
        if (!response?.ok()) throw new Error(`HTTP ${response?.status() ?? 'no response'}`);
        if (label === 'index') {
          await page.waitForSelector('nav a', { timeout: 5000 });
        } else {
          try { await page.waitForSelector('canvas.emaki-canvas', { timeout: 5000 }); }
          catch (error) {
            if (!placeholder) throw error;
            check(`${label} loads`, true, 'skipped (placeholder)');
            continue;
          }
        }
        check(`${label} loads`, true);
        if (label === 'showcase') {
          await page.waitForSelector('.emaki-widget video, .emaki-widget [data-fallback]', { timeout: 5000 });
          check('showcase video or fallback', true);
          const changed = await page.evaluate(async () => {
            const canvases = [...document.querySelectorAll('.emaki-widget canvas')]
              .filter(canvas => !canvas.hasAttribute('data-fallback') && !canvas.closest('[data-fallback]'));
            const before = canvases.map(canvas => canvas.toDataURL());
            await new Promise(resolve => setTimeout(resolve, 250));
            return canvases.some((canvas, index) => canvas.isConnected && canvas.toDataURL() !== before[index]);
          });
          check('showcase waveform changes', changed, changed ? '' : 'No widget canvas changed between frames');
        }
      } catch (error) { check(`${label} loads and widgets`, false, String(error)); }
    }
    check('no console or page errors', errors.length === 0, errors.join('\n'));
    await context.close();
  } catch (error) { check('browser execution', false, String(error)); }
  finally { await instance.close(); }
  const testsPassed = checks.filter(item => item.ok).length;
  results.push({ browser: name, checks, testsRun: checks.length, testsPassed, failureCount: checks.length - testsPassed });
}
const totals = results.reduce((sum, item) => ({ testsRun: sum.testsRun + item.testsRun, testsPassed: sum.testsPassed + item.testsPassed, failureCount: sum.failureCount + item.failureCount }), { testsRun: 0, testsPassed: 0, failureCount: 0 });
mkdirSync(resolve(out), { recursive: true });
writeFileSync(resolve(out, 'result.json'), JSON.stringify({ browsers: results, ...totals }, null, 2) + '\n');
console.log(JSON.stringify({ browsers: results, ...totals }, null, 2));
process.exit(unavailable ? 2 : totals.failureCount ? 1 : 0);
