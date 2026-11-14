import { chromium } from 'playwright';
import { existsSync, readFileSync } from 'node:fs';
import assert from 'node:assert/strict';

const result = { testsRun: 0, testsPassed: 0, failureCount: 0 };
let browser;
function check(action) {
  result.testsRun++;
  action();
  result.testsPassed++;
}
try {
  const installed = JSON.parse(readFileSync(new URL('../../node_modules/playwright/package.json', import.meta.url)));
  check(() => assert.equal(installed.version, '1.62.1'));
  result.executablePath = chromium.executablePath();
  check(() => assert.ok(existsSync(result.executablePath)));
  browser = await chromium.launch({ headless: true, timeout: 30000 });
  result.version = browser.version();
  const page = await browser.newPage();
  await page.setContent('<canvas width="64" height="64"></canvas>');
  result.gpu = await page.evaluate(() => {
    const gl = document.querySelector('canvas').getContext('webgl2');
    if (!gl) return { webgl2: false };
    gl.clearColor(0, 1, 0, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    const pixel = new Uint8Array(4);
    gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, pixel);
    return { webgl2: true, pixel: Array.from(pixel) };
  });
  check(() => assert.equal(result.gpu.webgl2, true));
  check(() => assert.deepEqual(result.gpu.pixel, [0, 255, 0, 255]));
} catch (error) {
  result.failureCount++;
  result.error = String(error);
  process.exitCode = 1;
} finally {
  try { await browser?.close(); }
  catch (error) { result.failureCount++; result.closeError = String(error); process.exitCode = 1; }
  console.log(JSON.stringify(result));
}
