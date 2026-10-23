// Comment-driven presentation, using real media decoding and real wasm audio.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { chromium, webkit } from 'playwright';
import { editorRoot, repoRoot, startServer } from './serve.mjs';
import { installSilentSink } from './silent-sink.mjs';

const evidence = path.join(repoRoot, 'tmp/editor-background');
fs.mkdirSync(evidence, { recursive: true });
const videoFile = path.join(editorRoot, 'dist', 'background-test.mp4');
execFileSync('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-y', '-f', 'lavfi', '-i', 'testsrc2=size=320x180:rate=12',
  '-t', '2', '-an', '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', videoFile]);
const server = await startServer();
const results = [];
const directive = (settings) => `#@ editor ${JSON.stringify(settings)}`;
const audio = 's :analog > note [60 64 67 72] > gain 0.2 > d1';
try {
  for (const [engine, launcher] of Object.entries({ chromium, webkit })) {
    const browser = await launcher.launch({ headless: true });
    const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
    await installSilentSink(context);
    const page = await context.newPage();
    try {
      await page.goto(`${server.origin}/?perf=1`, { waitUntil: 'domcontentloaded' });
      await page.waitForFunction(() => Boolean(window.__vactrPerf));
      const input = page.locator('.vact-code-input-bridge textarea');
      const replace = async (source) => {
        await input.focus(); await input.press('ControlOrMeta+a'); await input.fill(source);
        await page.waitForFunction((expected) => window.__vactrPerf.doc() === expected, source);
      };
      const imageConfig = { background: { kind: 'image', src: '/examples/editor-background.svg', opacity: 1, scrim: 0.25 } };
      await replace(directive(imageConfig));
      await page.waitForFunction(() => { const img = document.querySelector('.vact-background-media'); return img?.complete && img.naturalWidth > 0; });
      assert.equal(await page.locator('.vact-eval-status').getAttribute('data-status'), 'idle');
      await page.screenshot({ path: path.join(evidence, `${engine}-image.png`) });
      const image = await page.locator('.pane-code').screenshot();
      await replace(directive({ background: { kind: 'none', scrim: 0.25 } }));
      const plain = await page.locator('.pane-code').screenshot();
      const difference = await page.evaluate(async ([a, b]) => {
        const decode = async (base64) => {
          const img = new Image(); img.src = `data:image/png;base64,${base64}`; await img.decode();
          const canvas = document.createElement('canvas'); canvas.width = img.width; canvas.height = img.height;
          const ctx = canvas.getContext('2d'); ctx.drawImage(img, 0, 0); return ctx.getImageData(0, 0, img.width, img.height);
        };
        const first = await decode(a), second = await decode(b); let changed = 0;
        for (let y = 160; y < Math.min(first.height, second.height); y += 20) for (let x = 80; x < Math.min(first.width, second.width); x += 20) {
          const i = (y * first.width + x) * 4, j = (y * second.width + x) * 4;
          if (Math.abs(first.data[i] - second.data[j]) + Math.abs(first.data[i + 1] - second.data[j + 1]) + Math.abs(first.data[i + 2] - second.data[j + 2]) > 15) changed++;
        }
        return changed;
      }, [image.toString('base64'), plain.toString('base64')]);
      assert.ok(difference > 100, `image is not visible below code: ${difference}`);

      const scopeSource = `${directive({ ...imageConfig, visualizers: [
        { kind: 'oscilloscope', opacity: 1, position: 'bottom' },
        { kind: 'spectrum', opacity: 0.6, position: 'top' },
        { kind: 'spectrogram', opacity: 0.3, position: 'full' },
        { kind: 'level', opacity: 0.1, position: 'top' },
      ] })}\n${audio}`;
      await replace(scopeSource);
      assert.equal(await page.locator('.vact-eval-status').getAttribute('data-status'), 'idle');
      await page.locator('.vact-play-toggle').click();
      await page.waitForFunction(() => document.querySelector('.vact-eval-status')?.dataset.status === 'ok');
      await page.waitForFunction(() => {
        const canvas = document.querySelector('.vact-background-visualizer[data-kind="oscilloscope"]');
        if (!canvas || canvas.hidden) return false;
        const pixels = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data;
        let first = -1, last = -1;
        for (let y = 0; y < canvas.height; y++) for (let x = 0; x < canvas.width; x++) if (pixels[(y * canvas.width + x) * 4 + 3]) {
          if (first < 0) first = y; last = y; break;
        }
        return last - first > 3;
      });
      assert.equal(await page.locator('.vact-background-visualizer:not([hidden])').count(), 4);
      await page.screenshot({ path: path.join(evidence, `${engine}-oscilloscope.png`) });
      const scope = await page.locator('[data-kind="oscilloscope"]').evaluate((canvas) => ({ width: canvas.width, height: canvas.height }));
      await page.locator('.vact-hush').click();

      const videoSource = `${directive({ background: { kind: 'video', src: '/background-test.mp4', opacity: 0.9, scrim: 0.3 } })}\nlet x 1`;
      await replace(videoSource);
      await page.waitForFunction(() => { const video = document.querySelector('.vact-background-media'); return video?.tagName === 'VIDEO' && video.readyState >= 2 && !video.paused; });
      const video = await page.locator('.vact-background-media').evaluate((video) => ({ muted: video.muted, volume: video.volume, loop: video.loop, inline: video.playsInline, time: video.currentTime }));
      assert.ok(video.muted && video.volume === 0 && video.loop && video.inline);
      await page.waitForFunction((before) => document.querySelector('.vact-background-media').currentTime !== before, video.time);
      await page.screenshot({ path: path.join(evidence, `${engine}-video.png`) });
      await page.evaluate(() => { window.__backgroundTestVideo = document.querySelector('video.vact-background-media'); });
      await page.waitForTimeout(550);
      await replace('let x 1');
      assert.equal(await page.locator('.vact-background-media').count(), 0);
      assert.equal(await page.evaluate(() => window.__backgroundTestVideo.paused && !window.__backgroundTestVideo.hasAttribute('src') && !window.__backgroundTestVideo.isConnected), true);
      await input.press('ControlOrMeta+z');
      await page.waitForSelector('video.vact-background-media');
      await page.waitForFunction(() => !document.querySelector('video.vact-background-media').paused);

      await replace(directive({ background: { kind: 'image', src: '/missing-background.png' } }));
      await page.waitForFunction(() => document.querySelector('.visual-editor-settings-status')?.textContent.includes('could not be loaded'));
      await replace('#@ editor {"background":{"kind":"image","src":"javascript:alert(1)"}}');
      await page.waitForFunction(() => document.querySelector('.visual-editor-settings-status')?.textContent.includes('scheme'));
      assert.equal(await page.locator('.vact-background-media').count(), 0);
      await replace('let editor_still_works 42');
      assert.equal(await page.locator('.visual-editor-settings-status').isVisible(), false);
      const shaderSource = `${directive({ background: { kind: 'shader', output: 1, opacity: 0.8, scrim: 0.3 } })}\nsolid 0.2 0.3 0.5 > out o1`;
      await replace(shaderSource);
      await page.waitForFunction(() => document.querySelector('.visual-editor-settings-status')?.textContent.includes('Awaiting shader output o1'));
      await page.locator('.vact-play-toggle').click();
      await page.waitForFunction(() => document.querySelector('.vact-eval-status')?.dataset.status === 'ok');
      await page.waitForFunction(() => document.querySelector('.visual-editor-settings-status')?.hidden);
      assert.equal(await page.locator('[data-role="pane-select"]').inputValue(), 'o1');
      await page.screenshot({ path: path.join(evidence, `${engine}-shader.png`) });
      await page.locator('.vact-hush').click();

      await page.goto(`${server.origin}/?perf=1&example=editor-background`, { waitUntil: 'domcontentloaded' });
      await page.waitForFunction(() => document.querySelector('.vact-document-title')?.textContent === 'Editor Background');
      await page.waitForFunction(() => { const img = document.querySelector('img.vact-background-media'); return img?.complete && img.naturalWidth > 0; });
      assert.equal(await page.locator('.vact-eval-status').getAttribute('data-status'), 'idle');
      await page.locator('.vact-play-toggle').click();
      await page.waitForFunction(() => document.querySelector('.vact-eval-status')?.dataset.status === 'ok');
      await page.waitForFunction(() => { const scope = document.querySelector('[data-kind="oscilloscope"]'); return scope && !scope.hidden; });
      await page.screenshot({ path: path.join(evidence, `${engine}-example.png`) });
      assert.equal((await page.evaluate(() => window.__vactrSink.report())).post.peak, 0);
      results.push({ engine, pass: true, compositePixelsChanged: difference, scope, video, undoRestoredVideo: true, shaderOutput: 'o1', bundledExample: true });
    } catch (error) {
      const media = await page.evaluate(() => {
        const v = document.querySelector('video.vact-background-media');
        return v ? { error: v.error ? { code: v.error.code, message: v.error.message } : null,
          ready: v.readyState, network: v.networkState, src: v.currentSrc,
          mp4: v.canPlayType('video/mp4; codecs="avc1.42E01E"'), status: document.querySelector('.visual-editor-settings-status')?.textContent } : null;
      });
      await page.screenshot({ path: path.join(evidence, `${engine}-failure.png`) });
      results.push({ engine, pass: false, error: String(error.stack ?? error), media });
      console.error(engine, error.message);
    } finally { await context.close(); await browser.close(); }
  }
} finally { await server.close(); fs.rmSync(videoFile, { force: true }); }
fs.writeFileSync(path.join(evidence, 'results.json'), JSON.stringify(results, null, 2));
console.log(JSON.stringify(results, null, 2));
if (results.some((result) => !result.pass)) process.exitCode = 1;
