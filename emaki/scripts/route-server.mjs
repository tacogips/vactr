import { existsSync, statSync, readFileSync } from 'node:fs';
import { resolve, relative, extname, isAbsolute } from 'node:path';

export const ORIGIN = 'https://emaki.test';
const contentTypes = {
  '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8', '.json': 'application/json',
  '.svg': 'image/svg+xml', '.wasm': 'application/wasm', '.map': 'application/json',
};

export function distReady(distDir) {
  return existsSync(resolve(distDir, 'index.html'));
}

export async function routeDist(context, distDir) {
  const root = resolve(distDir);
  await context.route(ORIGIN + '/**', async (route) => {
    try {
      const path = decodeURIComponent(new URL(route.request().url()).pathname);
      if (path.split(/[\\/]/).includes('..')) return await route.fulfill({ status: 404 });
      let file = resolve(root, '.' + path);
      const rel = relative(root, file);
      if (rel === '..' || rel.startsWith('../') || isAbsolute(rel)) {
        return await route.fulfill({ status: 404 });
      }
      if (existsSync(file) && statSync(file).isDirectory()) file = resolve(file, 'index.html');
      if (!existsSync(file) || !statSync(file).isFile()) return await route.fulfill({ status: 404 });
      return await route.fulfill({ status: 200, contentType: contentTypes[extname(file)] ?? 'application/octet-stream', body: readFileSync(file) });
    } catch {
      return await route.fulfill({ status: 404 });
    }
  });
}
