// Percentiles use nearest rank: sorted[ceil(0.95 * n) - 1].
import { mkdirSync, writeFileSync, appendFileSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { cpus, loadavg, platform, release, arch } from 'node:os';
import { execFileSync } from 'node:child_process';
import { chromium, webkit } from 'playwright';
import { distReady, ORIGIN, routeDist } from './route-server.mjs';

const p95 = xs => xs.length ? [...xs].sort((a, b) => a - b)[Math.ceil(xs.length * .95) - 1] : null;
const scaleBudget = x => Math.max(2 * x, x + 2);
const gate = (browser, name, value, budget, pass = value !== null && value <= budget) => ({ browser, name, value, budget, pass });
function random(seed) { return () => { seed |= 0; seed = seed + 0x6d2b79f5 | 0; let t = Math.imul(seed ^ seed >>> 15, 1 | seed); t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; }; }
const args = process.argv.slice(2), options = { browser: 'all', sizes: '1000,10000,100000', loads: '5', keys: '200', edits: '100', out: `perf-results/${new Date().toISOString().replaceAll(':', '-')}` };
let selfTest = false;
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--self-test') { selfTest = true; continue; }
  const key = args[i].slice(2);
  if (!args[i].startsWith('--') || !(key in options) || !args[i + 1]) { console.error(`Invalid argument: ${args[i]}`); process.exit(2); }
  options[key] = args[++i];
}
if (selfTest) {
  const ok = p95(Array.from({ length: 100 }, (_, i) => i + 1)) === 95 && p95([]) === null && !gate('test', 'scale', 6, scaleBudget(2)).pass && gate('test', 'limit', 200, 200).pass;
  console.log(ok ? 'self-test passed' : 'self-test failed'); process.exit(ok ? 0 : 1);
}
const sizes = options.sizes.split(',').map(Number), loads = Number(options.loads), keys = Number(options.keys), edits = Number(options.edits);
if (!['all', 'chromium', 'webkit'].includes(options.browser) || !sizes.length || sizes.some(n => ![1000, 10000, 100000].includes(n)) || [loads, keys, edits].some(n => !Number.isInteger(n) || n < 1)) { console.error('Invalid options'); process.exit(2); }
if (!distReady('dist/json-10k') || !distReady('dist/diff')) { console.error('dist missing; run npm run build'); process.exit(2); }
const out = resolve(options.out); mkdirSync(out, { recursive: true });
const env = { os: { platform: platform(), release: release(), arch: arch() }, cpu: cpus().map(c => c.model), loadavgBefore: loadavg(), browsers: {}, scrollPaths: {}, distSha256: createHash('sha256').update(readFileSync('dist/json-10k/index.html')).digest('hex'), gitHead: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim() };
const gates = []; let unavailable = false;
const frame = page => page.evaluate(() => new Promise(r => requestAnimationFrame(() => r())));
const write = (file, sample) => appendFileSync(resolve(out, file), JSON.stringify(sample) + '\n');
for (const name of options.browser === 'all' ? ['chromium', 'webkit'] : [options.browser]) {
  let browser;
  try {
    browser = await (name === 'chromium' ? chromium : webkit).launch(name === 'chromium' ? { args: ['--mute-audio', '--use-angle=metal', '--enable-gpu', '--ignore-gpu-blocklist'] } : {});
    env.browsers[name] = browser.version();
    const context = await browser.newContext({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2 });
    await routeDist(context, 'dist');
    const keyP95 = {};
    for (const size of sizes) {
      const file = `raw-${name}-${size}.jsonl`; writeFileSync(resolve(out, file), '');
      let page, paints = [];
      for (let i = 0; i < loads; i++) {
        if (page) await page.close(); page = await context.newPage();
        await page.goto(`${ORIGIN}/json-10k/index.html?lines=${size}&autoload=0`);
        await page.waitForFunction(() => !!window.__emakiPerf);
        const sample = await page.evaluate(() => window.__emakiPerf.load()); paints.push(sample.firstPaintMs); write(file, { metric: 'firstPaintMs', value: sample.firstPaintMs, load: i });
      }
      if (size === 10000) gates.push(gate(name, 'firstPaintMs', Math.max(...paints), 200));
      await page.locator('#editor canvas').hover();
      await page.evaluate(() => window.__emakiPerf.startRecording());
      const initialTop = await page.evaluate(() => window.__emakiPerf.info().scrollTop); let path = 'wheel';
      for (let i = 0; i < 300; i++) {
        if (path === 'wheel') await page.mouse.wheel(0, 120); else await page.evaluate(() => window.__emakiPerf.scrollBy(120));
        await frame(page);
        if (i === 9 && await page.evaluate(() => window.__emakiPerf.info().scrollTop) <= initialTop) path = 'scrollBy';
      }
      env.scrollPaths[`${name}-${size}`] = path;
      const scroll = await page.evaluate(() => window.__emakiPerf.stopRecording());
      scroll.presents.forEach((p, i) => { write(file, { metric: 'scrollWorkMs', value: p.workMs, frame: i }); if (i) write(file, { metric: 'rafIntervalMs', value: p.frameStart - scroll.presents[i - 1].frameStart, frame: i }); });
      if (size === 10000) gates.push(gate(name, 'scrollWorkP95', p95(scroll.presents.map(p => p.workMs)), 16.7));
      await page.evaluate(() => window.__emakiPerf.startRecording()); const rng = random(7);
      for (let i = 0; i < keys; i++) {
        const target = await page.evaluate(() => { const e = document.querySelector('#editor canvas'); const info = window.__emakiPerf.info(); return { first: Math.floor(info.scrollTop / 18) + 1, count: Math.max(1, Math.floor(e.clientHeight / 18) - 1), lines: info.lines }; });
        const line = Math.min(target.lines, target.first + Math.floor(rng() * target.count));
        await page.evaluate(line => window.__emakiPerf.caretToLine(line, 2), line); await frame(page); await frame(page);
        await page.keyboard.press(String.fromCharCode(97 + Math.floor(rng() * 26))); await frame(page); await frame(page); await frame(page);
      }
      const metrics = await page.evaluate(() => { const r = window.__emakiPerf.stopRecording(); return window.__emakiPerf.keyMetrics(r.inputs, r.presents); });
      metrics.forEach((m, i) => write(file, { metric: 'key', value: m.b, rawLatencyMs: m.a, nextFrame: m.nextFrame, missing: m.missing, key: i }));
      const work = p95(metrics.filter(m => !m.missing).map(m => m.b)); keyP95[size] = metrics.length === keys && metrics.every(m => !m.missing) ? work : null;
      if (size === 10000) { gates.push(gate(name, 'keyWorkP95', work, 16.7, metrics.length === keys && metrics.every(m => !m.missing) && work !== null && work <= 16.7)); const ratio = metrics.filter(m => m.nextFrame).length / keys; gates.push(gate(name, 'nextFrameRatio', ratio, .95, metrics.length === keys && ratio >= .95)); }
      await page.close();
    }
    if (sizes.includes(1000) && sizes.includes(100000)) { const a = keyP95[1000], b = keyP95[100000]; gates.push(gate(name, 'sizeScaling', b, a === null ? null : scaleBudget(a), a !== null && b !== null && b <= scaleBudget(a))); }
    if (sizes.includes(10000)) {
      const file = `diff-${name}.jsonl`; writeFileSync(resolve(out, file), ''); const page = await context.newPage();
      await page.goto(`${ORIGIN}/diff/index.html?lines=10000`); await page.waitForFunction(() => window.__emakiDiff?.stats().diffRevision > 0);
      await page.evaluate(() => { window.__perfDiffEvents = []; document.querySelector('#right-editor').addEventListener('keydown', e => window.__perfDiffEvents.push({ time: e.timeStamp, revision: window.__emakiDiff.stats().rightRevision + 1 }), true); window.__perfDiffApplied = []; window.__emakiDiff.onApplied(s => window.__perfDiffApplied.push(s)); });
      const rng = random(7), canvas = page.locator('#right-editor canvas.emaki-canvas'); const box = await canvas.boundingBox();
      for (let i = 0; i < edits; i++) {
        const y = 9 + Math.floor(rng() * Math.max(1, Math.floor((box.height - 18) / 18))) * 18;
        const x = await canvas.evaluate((element, y) => { const rect = element.getBoundingClientRect(); for (let x = 80; x <= 88; x++) if (document.elementFromPoint(rect.left + x, rect.top + y) === element) return x; throw new Error('No unobstructed diff canvas click point'); }, y);
        await canvas.click({ position: { x, y } });
        const revision = await page.evaluate(() => window.__emakiDiff.stats().rightRevision + 1);
        await page.keyboard.press(String.fromCharCode(97 + Math.floor(rng() * 26)));
        await page.waitForFunction(rev => window.__perfDiffApplied.some(s => s.rightRevision >= rev), revision);
        const sample = await page.evaluate(rev => { const event = window.__perfDiffEvents.find(e => e.revision === rev); const present = window.__perfDiffApplied.find(s => s.rightRevision >= rev); return event && present ? present.presentEnd - event.time : null; }, revision);
        write(file, { metric: 'diffLatencyMs', value: sample, revision });
      }
      const values = readFileSync(resolve(out, file), 'utf8').trim().split('\n').map(line => JSON.parse(line).value); const val = p95(values.filter(v => v !== null)); gates.push(gate(name, 'diffLatencyP95', val, 100, values.length === edits && values.every(v => v !== null) && val !== null && val <= 100)); await page.close();
    }
    await context.close();
  } catch (error) { unavailable = true; console.error(`${name}: ${error.stack || error}`); }
  finally { if (browser) await browser.close(); }
}
env.loadavgAfter = loadavg(); writeFileSync(resolve(out, 'env.json'), JSON.stringify(env, null, 2) + '\n');
const summary = { gates, allPass: !unavailable && gates.length > 0 && gates.every(g => g.pass) }; writeFileSync(resolve(out, 'summary.json'), JSON.stringify(summary, null, 2) + '\n'); console.log(JSON.stringify(summary, null, 2)); process.exit(unavailable ? 2 : summary.allPass ? 0 : 1);
