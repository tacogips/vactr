#!/usr/bin/env node
// Headless runner for the Vactr dev harness (design 12.8.11).
//
// Node built-ins only. Serves the repository root on 127.0.0.1:<free port>
// (a secure context, so AudioWorklet is available), launches Chrome from
// $CHROME or the default macOS path with --headless=new (or a visible
// window with --headed, the operator path of B1), receives the page's JSON
// report by POST, writes it next to the log under target/fe-logs/, and
// exits:
//   0  every check passed
//   1  a check failed, the report is malformed, or the wasm module is
//      missing (build it with
//      `cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm`)
//   2  BLOCKED: no Chrome, or the AudioContext never reached `running`.
//      A blocked run is never a pass; the operator runs `--headed`.
// The whole run times out after 120 s (exit 1).
//
// Options: --headed, --out <path.json>, --timeout <seconds>.

import http from 'node:http';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..', '..');
const args = process.argv.slice(2);
const headed = args.includes('--headed');
const opt = (name, dflt) => {
  const i = args.indexOf(name);
  return i >= 0 && i + 1 < args.length ? args[i + 1] : dflt;
};
const timeoutSec = Number(opt('--timeout', '120'));
const logDir = path.join(root, 'target', 'fe-logs');

// The cdylib: `cargo build` also links the `vactr` bin to the same
// uplifted file name, so the module with the ABI exports is chosen here.
function findModule() {
  const candidates = [
    path.join(root, 'target/wasm32-unknown-unknown/debug/vactr.wasm'),
    path.join(root, 'target/wasm32-unknown-unknown/debug/deps/vactr.wasm'),
  ];
  if (!fs.existsSync(candidates[0])) return { missing: candidates[0] };
  for (const file of candidates) {
    if (!fs.existsSync(file)) continue;
    try {
      const m = new WebAssembly.Module(fs.readFileSync(file));
      const names = WebAssembly.Module.exports(m).map((e) => e.name);
      if (names.includes('main_init') && names.includes('worklet_init')) return { file };
    } catch {
      // not a loadable module; try the next one
    }
  }
  return { missing: candidates.join(' or ') + ' (with the main_init/worklet_init exports)' };
}

function defaultOut() {
  fs.mkdirSync(logDir, { recursive: true });
  let n = 1;
  while (fs.existsSync(path.join(logDir, `be-wasm-harness-s182-${n}.json`))) n += 1;
  return path.join(logDir, `be-wasm-harness-s182-${n}.json`);
}

function chromePath() {
  const candidates = [
    process.env.CHROME,
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/usr/bin/google-chrome',
    '/usr/bin/chromium',
  ].filter(Boolean);
  return candidates.find((p) => fs.existsSync(p));
}

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.wasm': 'application/wasm',
  '.json': 'application/json',
  '.md': 'text/plain; charset=utf-8',
};

function finish(code, message) {
  console.log(message);
  console.log(`harness exit=${code}`);
  process.exit(code);
}

const mod = findModule();
if (mod.missing) finish(1, `FAIL: wasm module missing: ${mod.missing}`);
const chrome = chromePath();
if (!chrome) finish(2, 'BLOCKED: no Chrome found ($CHROME or the default path); run --headed on a machine with Chrome');
const out = path.resolve(opt('--out', defaultOut()));
console.log(`module: ${path.relative(root, mod.file)}`);
console.log(`chrome: ${chrome}${headed ? ' (headed)' : ' (headless)'}`);

let child = null;
let profile = null;
const server = http.createServer((req, res) => {
  const url = new URL(req.url, 'http://127.0.0.1');
  if (req.method === 'POST') {
    let body = '';
    req.on('data', (c) => {
      body += c;
    });
    req.on('end', () => {
      res.writeHead(204);
      res.end();
      if (url.pathname === '/log') {
        console.log(`[page] ${body}`);
      } else if (url.pathname === '/report') {
        onReport(body);
      }
    });
    return;
  }
  let file;
  if (url.pathname === '/vactr.wasm') {
    file = mod.file;
  } else {
    file = path.join(root, decodeURIComponent(url.pathname));
    if (!file.startsWith(root + path.sep)) {
      res.writeHead(403);
      res.end();
      return;
    }
  }
  fs.readFile(file, (err, data) => {
    if (err) {
      res.writeHead(404);
      res.end();
      return;
    }
    res.writeHead(200, {
      'Content-Type': MIME[path.extname(file)] || 'application/octet-stream',
      'Cache-Control': 'no-store',
    });
    res.end(data);
  });
});

function cleanup() {
  if (child && child.exitCode === null) child.kill('SIGKILL');
  if (profile) fs.rmSync(profile, { recursive: true, force: true });
  server.close();
}

function onReport(body) {
  let rep;
  try {
    rep = JSON.parse(body);
  } catch {
    cleanup();
    finish(1, 'FAIL: the page posted a malformed report');
  }
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, JSON.stringify(rep, null, 2) + '\n');
  console.log(`report: ${path.relative(root, out)}`);
  cleanup();
  if (rep.blocked) finish(2, `BLOCKED: ${rep.reason}. Run \`node editor/dev-harness/run-headless.mjs --headed\` (B1).`);
  const checks = Array.isArray(rep.checks) ? rep.checks : [];
  for (const c of checks) console.log(`${c.pass ? 'PASS' : 'FAIL'} ${c.id}: ${c.detail}`);
  console.log(`memoryStable=${rep.memoryStable} userAgent=${rep.userAgent}`);
  const ok = checks.length > 0 && checks.every((c) => c.pass) && rep.memoryStable === true;
  finish(ok ? 0 : 1, ok ? `PASS: ${checks.length} checks` : 'FAIL: see the failed checks above');
}

server.listen(0, '127.0.0.1', () => {
  const { port } = server.address();
  profile = fs.mkdtempSync(path.join(os.tmpdir(), 'vactr-harness-'));
  const page = `http://127.0.0.1:${port}/editor/dev-harness/index.html?auto=1`;
  const flags = [
    '--autoplay-policy=no-user-gesture-required',
    '--no-first-run',
    '--no-default-browser-check',
    `--user-data-dir=${profile}`,
  ];
  if (!headed) flags.unshift('--headless=new', '--disable-gpu');
  console.log(`page: ${page}`);
  child = spawn(chrome, [...flags, page], { stdio: ['ignore', 'ignore', 'pipe'] });
  child.on('error', (e) => {
    cleanup();
    finish(2, `BLOCKED: Chrome did not start (${e.message})`);
  });
});

setTimeout(() => {
  cleanup();
  finish(1, `FAIL: no report within ${timeoutSec} s`);
}, timeoutSec * 1000).unref();
