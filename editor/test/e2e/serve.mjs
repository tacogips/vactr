import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const here = path.dirname(fileURLToPath(import.meta.url));
const editorRoot = path.resolve(here, '../..');
const repoRoot = path.resolve(editorRoot, '..');
const mime = { '.html':'text/html; charset=utf-8','.js':'text/javascript; charset=utf-8','.mjs':'text/javascript; charset=utf-8','.css':'text/css; charset=utf-8','.json':'application/json','.wasm':'application/wasm','.svg':'image/svg+xml','.png':'image/png','.jpg':'image/jpeg','.jpeg':'image/jpeg','.gif':'image/gif','.mp4':'video/mp4','.webm':'video/webm' };
export function startServer({ dist = path.join(editorRoot, 'dist') } = {}) {
  if (!fs.existsSync(path.join(dist, 'index.html'))) throw new Error(`editor dist missing: ${dist}`);
  const server = http.createServer((req, res) => {
    let pathname;
    try { pathname = decodeURIComponent(new URL(req.url, 'http://127.0.0.1').pathname); } catch { res.writeHead(400).end(); return; }
    const candidate = path.resolve(dist, `.${pathname === '/' ? '/index.html' : pathname}`);
    if (candidate !== dist && !candidate.startsWith(dist + path.sep)) { res.writeHead(403).end(); return; }
    fs.readFile(candidate, (error, data) => {
      if (error) { res.writeHead(404).end('not found'); return; }
      const headers = { 'Content-Type': mime[path.extname(candidate)] || 'application/octet-stream', 'Cache-Control':'no-store', 'Accept-Ranges':'bytes' };
      if (req.headers.range) {
        const range = /^bytes=(\d*)-(\d*)$/.exec(req.headers.range);
        const suffix = range && !range[1] ? Number(range[2]) : null;
        const start = suffix !== null ? Math.max(0, data.length - suffix) : Number(range?.[1]);
        const end = suffix !== null || !range?.[2] ? data.length - 1 : Math.min(Number(range[2]), data.length - 1);
        if (!range || !Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || start > end || start >= data.length) {
          res.writeHead(416, { ...headers, 'Content-Range': `bytes */${data.length}` }).end(); return;
        }
        res.writeHead(206, { ...headers, 'Content-Range': `bytes ${start}-${end}/${data.length}`, 'Content-Length': end - start + 1 });
        res.end(data.subarray(start, end + 1)); return;
      }
      res.writeHead(200, { ...headers, 'Content-Length': data.length }); res.end(data);
    });
  });
  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => resolve({ server, origin: `http://127.0.0.1:${server.address().port}`, close: () => new Promise((done) => server.close(done)) }));
  });
}
export { editorRoot, repoRoot };
