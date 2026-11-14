#!/usr/bin/env node
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium, webkit } from 'playwright';
import { startServer, editorRoot, repoRoot } from './serve.mjs';
import { runBehavior } from './behavior.mjs';
import { runDomBehavior } from './behavior-dom.mjs';
import { runMeasurement } from './measure.mjs';
import { installSilentSink } from './silent-sink.mjs';
import { gatingPreflight } from './wasm-profile.mjs';
import { BROWSERS, DEFAULT_LINES, DEFAULT_RUNS, RENDERERS, buildComparison, checkLock, contended, downsampleSamples, extractRunMetrics, renderComparison, replaceMarked, REPORT_SKELETON, runKey, runOrder, sliceWorkload, MAX_RUN_JSONL_BYTES } from './compare-stats.mjs';
import { createLargeDocument } from './fixtures/large-doc.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
function parseArgs(args) {
  const values = new Map(); const flags = new Set(['--behavior', '--resume', '--write-report']);
  for (let i = 0; i < args.length; i++) {
    const arg = args[i]; if (!arg.startsWith('--')) throw new Error(`unexpected argument: ${arg}`);
    if (flags.has(arg)) { if (values.has(arg)) throw new Error(`duplicate flag: ${arg}`); values.set(arg, true); continue; }
    if (!['--run-id', '--renderers', '--browsers', '--lines', '--runs', '--out', '--report'].includes(arg)) throw new Error(`unknown flag: ${arg}`);
    if (values.has(arg) || !args[i + 1] || args[i + 1].startsWith('--')) throw new Error(`missing or duplicate value for ${arg}`);
    values.set(arg, args[++i]);
  }
  const get = (name, fallback) => values.get(name) ?? fallback;
  const runId = get('--run-id', 'rc-001'); if (!/^[a-z0-9-]+$/.test(runId)) throw new Error('--run-id must match ^[a-z0-9-]+$');
  const parseList = (name, defaults, allowed) => {
    const result = get(name, defaults.join(',')).split(','); if (!result.length || result.some((item) => !allowed.includes(item)) || new Set(result).size !== result.length) throw new Error(`invalid ${name} value`); return result;
  };
  const renderers = parseList('--renderers', [...RENDERERS], [...RENDERERS]); const browsers = parseList('--browsers', [...BROWSERS], [...BROWSERS]);
  const lines = get('--lines', DEFAULT_LINES.join(',')).split(',').map(Number); if (!lines.length || lines.some((line) => !Number.isSafeInteger(line) || line < 8 || line > 20000) || new Set(lines).size !== lines.length) throw new Error('invalid --lines value');
  const runs = Number(get('--runs', String(DEFAULT_RUNS))); if (!Number.isSafeInteger(runs) || runs < 1 || runs > 100) throw new Error('--runs must be an integer from 1 to 100');
  const out = path.resolve(get('--out', path.join(repoRoot, 'design-docs/specs/evidence/renderer-comparison', runId)));
  const report = path.resolve(get('--report', path.join(repoRoot, 'design-docs/specs/design-renderer-comparison.md')));
  return { runId, renderers, browsers, lines, runs, out, report, behavior: values.has('--behavior'), resume: values.has('--resume'), writeReport: values.has('--write-report') };
}

const atomicJson = (file, value) => { fs.mkdirSync(path.dirname(file), { recursive: true }); const temp = `${file}.${process.pid}.tmp`; fs.writeFileSync(temp, `${JSON.stringify(value, null, 2)}\n`); fs.renameSync(temp, file); };
const jsonFiles = (directory) => fs.existsSync(directory) ? fs.readdirSync(directory).filter((file) => /^\d+-(chromium|webkit)-(canvas|dom)-r\d+\.json$/.test(file)).sort() : [];
function readRecords(out) {
  const records = [];
  for (const file of jsonFiles(path.join(out, 'runs'))) { try { const record = JSON.parse(fs.readFileSync(path.join(out, 'runs', file), 'utf8')); if (record.complete === true) records.push(record); } catch { /* Partial or malformed records are not complete evidence. */ } }
  return records;
}
function writeReport(out, report, environment, attempts) {
  const comparison = buildComparison(readRecords(out), environment, attempts); atomicJson(path.join(out, 'comparison.json'), comparison);
  const source = fs.existsSync(report) ? fs.readFileSync(report, 'utf8') : REPORT_SKELETON;
  fs.mkdirSync(path.dirname(report), { recursive: true }); fs.writeFileSync(report, replaceMarked(source, renderComparison(comparison)));
  return comparison;
}
const errorText = (error) => String(error?.stack ?? error);

async function probeWebkitHeadless() {
  const browser = await webkit.launch({ headless: true, timeout: 30000 });
  try { const context = await browser.newContext(); try { await installSilentSink(context); const page = await context.newPage(); await page.setContent('<canvas></canvas>'); return Boolean(await page.evaluate(() => document.querySelector('canvas').getContext('webgl2'))); } finally { await context.close(); } }
  finally { await browser.close(); }
}
async function launchBrowser(name, headless) {
  const engine = name === 'chromium' ? chromium : webkit;
  return engine.launch({ headless, timeout: 30000, args: name === 'chromium' ? ['--mute-audio', '--use-angle=metal', '--enable-gpu', '--ignore-gpu-blocklist'] : [] });
}
async function firstViewport(browser, origin, renderer, workload) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1 });
  try {
    await installSilentSink(context); const page = await context.newPage();
    await page.goto(`${origin}/?perf=1&renderer=${renderer}`, { waitUntil: 'domcontentloaded', timeout: 60000 });
    await page.waitForFunction(() => Boolean(window.__vactrPerf), { timeout: 30000 });
    let mountToFirstFrameMs = null;
    try { await page.waitForFunction(() => window.__vactrPerf.presented().length > 0, undefined, { timeout: 5000 }); mountToFirstFrameMs = await page.evaluate(() => window.__vactrPerf.presented()[0]?.frameMs ?? null); } catch { /* Report unavailable when no frame is presented. */ }
    const t0 = await page.evaluate(() => performance.now()); await page.locator('.vact-code-input-bridge textarea').fill(workload.text);
    let loadToViewportMs = null;
    try {
      const handle = await page.waitForFunction(({ t0, length }) => {
        const api = window.__vactrPerf;
        if (api.counters().textPending !== false) return false;
        const revision = api.revision();
        if (api.doc().length !== length) return false;
        const rows = api.presented();
        for (let i = rows.length - 1; i >= 0; i--) {
          const row = rows[i];
          if (row.frameMs > t0 && row.revision === revision) return { frameMs: row.frameMs };
        }
        return false;
      }, { t0, length: workload.text.length }, { timeout: 60000, polling: 'raf' });
      try { loadToViewportMs = (await handle.jsonValue()).frameMs - t0; }
      finally { await handle.dispose(); }
    } catch { loadToViewportMs = null; }
    return { mountToFirstFrameMs, loadToViewportMs };
  } finally { await context.close(); }
}

async function main() {
  let options;
  try { options = parseArgs(process.argv.slice(2)); }
  catch (error) { console.error(JSON.stringify({ blocked: true, error: errorText(error) })); process.exitCode = 2; return; }
  if (options.writeReport) {
    try {
      let environment = { runId: options.runId, generatedAt: new Date().toISOString(), mode: 'write-report' };
      const environmentPath = path.join(options.out, 'environment.json'); if (fs.existsSync(environmentPath)) environment = JSON.parse(fs.readFileSync(environmentPath, 'utf8'));
      let attempts = []; const attemptsPath = path.join(options.out, 'attempts.json'); if (fs.existsSync(attemptsPath)) attempts = JSON.parse(fs.readFileSync(attemptsPath, 'utf8'));
      writeReport(options.out, options.report, environment, attempts); console.log(JSON.stringify({ runId: options.runId, pass: true, blocked: false, testsRun: 0, testsPassed: 0, failureCount: 0, gatePassed: null, incomplete: readRecords(options.out).length === 0 })); return;
    } catch (error) { console.error(errorText(error)); process.exitCode = 1; return; }
  }
  const lockPath = path.resolve(process.env.VACTR_MEASURE_LOCK ?? path.join(repoRoot, '..', '.measure-lock'));
  let owner = null;
  try { owner = checkLock(fs.readFileSync(path.join(lockPath, 'owner'), 'utf8')); } catch { owner = { ok: false, owner: null }; }
  if (!owner.ok) { console.log(JSON.stringify({ runId: options.runId, pass: false, blocked: true, testsRun: 0, testsPassed: 0, failureCount: 0, gatePassed: false, incomplete: true, reason: 'measurement lock owner file is missing or empty' })); process.exitCode = 2; return; }
  const dist = path.join(editorRoot, 'dist');
  if (!fs.existsSync(path.join(dist, 'index.html'))) { console.log(JSON.stringify({ runId: options.runId, pass: false, blocked: true, testsRun: 0, testsPassed: 0, failureCount: 0, gatePassed: false, incomplete: true, reason: `built editor dist missing: ${dist}` })); process.exitCode = 2; return; }
  let wasm;
  try { const preflight = await gatingPreflight({ distWasm: path.join(dist, 'vactr.wasm'), releasePath: path.join(repoRoot, 'target/wasm32-unknown-unknown/release/vactr.wasm'), debugPath: path.join(repoRoot, 'target/wasm32-unknown-unknown/debug/vactr.wasm'), writeEvidence: true }); wasm = preflight.wasm; if (preflight.refusal) throw new Error(preflight.refusal); }
  catch (error) { console.log(JSON.stringify({ runId: options.runId, pass: false, blocked: true, testsRun: 0, testsPassed: 0, failureCount: 0, gatePassed: false, incomplete: true, reason: errorText(error) })); process.exitCode = 2; return; }

  const command = `cd editor && node test/e2e/compare.mjs --run-id ${options.runId}`;
  const environment = { runId: options.runId, hostname: os.hostname(), platform: os.platform(), release: os.release(), arch: os.arch(), node: process.version, cpus: os.cpus().length, lockOwner: owner.owner,
    wasm, distBuild: 'VACTR_REQUIRE_SESSION_ABI=1 npm run build', command, startedAt: new Date().toISOString(), browserModes: {} };
  const attemptsPath = path.join(options.out, 'attempts.json'); let attempts = [];
  if (fs.existsSync(attemptsPath)) { try { attempts = JSON.parse(fs.readFileSync(attemptsPath, 'utf8')); } catch { attempts = []; } }
  fs.mkdirSync(path.join(options.out, 'runs'), { recursive: true });
  let webkitHeadless = true;
  if (options.browsers.includes('webkit')) { try { webkitHeadless = await probeWebkitHeadless(); } catch { webkitHeadless = false; } }
  environment.browserModes.webkit = { headless: webkitHeadless, probe: 'headless WebGL2' };
  let server;
  let completed = 0; let attempted = 0; let incomplete = 0; let gatePassed = true; let behaviorFailures = 0;
  try {
    server = await startServer({ dist });
    if (options.behavior) {
      for (const browserName of options.browsers) for (const renderer of options.renderers) {
        attempted++; let browser;
        try {
          browser = await launchBrowser(browserName, browserName === 'webkit' ? webkitHeadless : true);
          const result = renderer === 'dom' ? await runDomBehavior(browser, server.origin, browserName) : await runBehavior(browser, server.origin, browserName);
          const file = path.join(options.out, 'behavior', `${browserName}-${renderer}-behavior.json`); atomicJson(file, result);
          const failed = result.checks.filter((item) => item.status !== 'limitation' && !item.pass).length;
          if (failed) behaviorFailures += failed; else completed++;
        } catch (error) { behaviorFailures++; atomicJson(path.join(options.out, 'behavior', `${browserName}-${renderer}-behavior.json`), { name: browserName, checks: [{ id: 'browser-setup', pass: false, detail: errorText(error) }], passed: 0, total: 1, limitations: [] }); }
        finally { await browser?.close().catch(() => {}); }
      }
      gatePassed = behaviorFailures === 0;
    } else {
      const baseWorkload = createLargeDocument();
      for (const lineCount of options.lines) for (const browserName of options.browsers) {
        const workload = lineCount === 20000 ? baseWorkload : sliceWorkload(baseWorkload, lineCount);
        const schedule = runOrder(options.runs, options.renderers);
        for (const item of schedule) {
          const key = runKey({ lines: lineCount, browser: browserName, renderer: item.renderer, run: item.run });
          const rawPath = path.join(options.out, 'runs', `${key}.json`);
          if (options.resume && fs.existsSync(rawPath)) { try { if (JSON.parse(fs.readFileSync(rawPath, 'utf8')).complete === true) { completed++; attempted++; continue; } } catch { /* Retry incomplete/corrupt file below. */ } }
          attempted++;
          let success = false;
          for (let attempt = 1; attempt <= 2 && !success; attempt++) {
            const startedAt = new Date().toISOString(); const loadavgStart = os.loadavg(); let browser; let context;
            try {
              browser = await launchBrowser(browserName, browserName === 'webkit' ? webkitHeadless : true);
          const version = browser.version(); environment.browserModes[browserName] = { headless: browserName === 'webkit' ? webkitHeadless : true, version }; const viewport = await firstViewport(browser, server.origin, item.renderer, workload);
              context = await browser.newContext({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1 }); await installSilentSink(context); const page = await context.newPage();
              await page.goto(`${server.origin}/?perf=1&renderer=${item.renderer}`, { waitUntil: 'domcontentloaded', timeout: 60000 });
              const measured = await runMeasurement(page, context, browserName, { profile: 'all', runId: options.runId, headless: browserName === 'webkit' ? webkitHeadless : true, workload });
              const rendererStats = measured.metrics.rendererStats ?? null;
              const rawMetrics = { ...measured.metrics }; for (const field of ['syncSamples', 'lateFrameAudits']) delete rawMetrics[field]; if (rawMetrics.beatResidual?.frames) rawMetrics.beatResidual = { ...rawMetrics.beatResidual, frames: undefined };
              const endedAt = new Date().toISOString(); const loadavgEnd = os.loadavg(); const isContended = contended(Math.max(loadavgStart[0], loadavgEnd[0]), os.cpus().length);
              const record = { key, lines: lineCount, browser: browserName, renderer: item.renderer, run: item.run, position: item.position, complete: true, headless: browserName === 'webkit' ? webkitHeadless : true,
                browserVersion: version, startedAt, endedAt, loadavgStart, loadavgEnd, contended: isContended, metrics: extractRunMetrics(measured, viewport), rawMetrics, rendererStats,
                gate: { pass: measured.pass, failures: measured.failures, limitations: measured.limitations } };
              const sampled = downsampleSamples(measured.samples); let jsonl = sampled.rows.map((row) => JSON.stringify(row)).join('\n') + '\n'; let stride = sampled.stride;
              while (Buffer.byteLength(jsonl) > MAX_RUN_JSONL_BYTES) { stride *= 2; const next = downsampleSamples(measured.samples, stride); jsonl = next.rows.map((row) => JSON.stringify(row)).join('\n') + '\n'; }
              record.sampleStride = stride;
              const jsonlPath = path.join(options.out, 'runs', `${key}.jsonl`); const jsonlTemp = `${jsonlPath}.${process.pid}.tmp`;
              fs.writeFileSync(jsonlTemp, jsonl); fs.renameSync(jsonlTemp, jsonlPath); atomicJson(rawPath, record);
              completed++; gatePassed &&= measured.pass; success = true;
            } catch (error) {
              const attemptRow = { key, attempt, startedAt, endedAt: new Date().toISOString(), error: errorText(error) }; attempts.push(attemptRow); atomicJson(attemptsPath, attempts);
              if (attempt === 2) { incomplete++; atomicJson(rawPath, { key, lines: lineCount, browser: browserName, renderer: item.renderer, run: item.run, position: item.position, complete: false, error: attemptRow.error }); }
            } finally { await context?.close().catch(() => {}); await browser?.close().catch(() => {}); }
          }
        }
      }
    }
  } catch (error) { incomplete++; attempts.push({ key: 'invocation', attempt: 1, error: errorText(error) }); atomicJson(attemptsPath, attempts); }
  finally { await server?.close(); }
  environment.endedAt = new Date().toISOString(); environment.browserModes.webkit = { headless: webkitHeadless, probe: 'headless WebGL2' };
  atomicJson(path.join(options.out, 'environment.json'), environment); atomicJson(attemptsPath, attempts);
  if (!options.behavior) writeReport(options.out, options.report, environment, attempts);
  const failureCount = incomplete + behaviorFailures;
  const result = { runId: options.runId, pass: failureCount === 0, blocked: false, testsRun: attempted, testsPassed: completed, failureCount, gatePassed, incomplete: failureCount > 0 };
  console.log(JSON.stringify(result)); process.exitCode = failureCount ? 1 : 0;
}

await main();
