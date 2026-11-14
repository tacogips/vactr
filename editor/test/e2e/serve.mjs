import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const here = path.dirname(fileURLToPath(import.meta.url));
const editorRoot = path.resolve(here, '../..');
const repoRoot = path.resolve(editorRoot, '..');
const mime = { '.html':'text/html; charset=utf-8','.js':'text/javascript; charset=utf-8','.mjs':'text/javascript; charset=utf-8','.css':'text/css; charset=utf-8','.json':'application/json','.wasm':'application/wasm','.svg':'image/svg+xml','.png':'image/png' };
export function startServer({ dist = path.join(editorRoot, 'dist') } = {}) {
  if (!fs.existsSync(path.join(dist, 'index.html'))) throw new Error(`editor dist missing: ${dist}`);
  const server = http.createServer((req, res) => {
    let pathname;
    try { pathname = decodeURIComponent(new URL(req.url, 'http://127.0.0.1').pathname); } catch { res.writeHead(400).end(); return; }
    const candidate = path.resolve(dist, `.${pathname === '/' ? '/index.html' : pathname}`);
    if (candidate !== dist && !candidate.startsWith(dist + path.sep)) { res.writeHead(403).end(); return; }
    fs.readFile(candidate, (error, data) => {
      if (error) { res.writeHead(404).end('not found'); return; }
      res.writeHead(200, { 'Content-Type': mime[path.extname(candidate)] || 'application/octet-stream', 'Cache-Control':'no-store' }); res.end(data);
    });
  });
  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => resolve({ server, origin: `http://127.0.0.1:${server.address().port}`, close: () => new Promise((done) => server.close(done)) }));
  });
}
export { editorRoot, repoRoot };
