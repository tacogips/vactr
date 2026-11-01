// Actual session/wasm coverage for the header's playback semantics and menus.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { chromium, webkit } from 'playwright';
import { startServer, repoRoot } from './serve.mjs';
import { installSilentSink } from './silent-sink.mjs';

const output = path.join(repoRoot, 'tmp/editor-header');
fs.mkdirSync(output, { recursive: true });
const server = await startServer();
const results = [];
try {
  for (const [engine, launcher] of Object.entries({ chromium, webkit })) {
    const browser = await launcher.launch({ headless: true });
    const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
    await installSilentSink(context);
    await context.addInitScript(() => {
      // Exercise the browser adapter without changing the host user's clipboard.
      Object.defineProperty(navigator, 'clipboard', { configurable: true, value: {
        writeText: async text => { window.__vactrTestClipboard = text; },
        readText: async () => window.__vactrTestClipboard ?? '',
      } });
    });
    const page = await context.newPage();
    try {
      await page.goto(`${server.origin}/?perf=1`, { waitUntil: 'domcontentloaded' });
      await page.waitForSelector('.vact-play-toggle');
      await page.waitForFunction(() => Boolean(window.__vactrPerf));
      const position = page.locator('.vact-position-value');
      const status = page.locator('.vact-playback-state');
      assert.equal(await status.textContent(), 'Stopped');
      await page.waitForTimeout(500);
      assert.equal(await position.textContent(), '-.-');
      assert.equal(await page.locator('.vact-pause').isDisabled(), true);
      assert.equal(await page.locator('.vact-run').isDisabled(), true);
      await page.screenshot({ path: path.join(output, `${engine}-desktop-stopped.png`) });

      const input = page.locator('.vact-code-input-bridge textarea');
      const replaceDocument = async (text) => {
        await input.focus();
        await input.press('ControlOrMeta+a');
        await input.fill(text);
        await page.waitForFunction((expected) => window.__vactrPerf.doc() === expected, text);
      };
      await replaceDocument('let answer 42');
      await page.locator('.vact-play-toggle').click();
      await page.waitForFunction(() => document.querySelector('.vact-eval-status')?.dataset.status === 'ok');
      await page.waitForTimeout(200);
      assert.equal(await status.textContent(), 'Stopped');
      assert.equal(await position.textContent(), '-.-');

      await replaceDocument('let a 1\nlet b 2\ns [nil nil] > d1');
      await page.locator('.vact-play-toggle').click();
      await page.waitForFunction(() => document.querySelector('.vact-playback-state')?.textContent === 'Playing');
      await page.locator('.vact-play-toggle').click();
      await input.focus();
      await input.press('ControlOrMeta+Home'); await input.press('End'); await input.press('Backspace');
      await input.press('ArrowDown'); await input.press('End'); await input.press('Shift+ArrowLeft');
      await page.keyboard.insertText('3');
      await input.press('ControlOrMeta+End'); await page.keyboard.insertText('\n# addition');
      assert.equal(await page.evaluate(() => window.__vactrPerf.doc()), 'let a \nlet b 3\ns [nil nil] > d1\n# addition');
      await page.waitForTimeout(250);
      await page.screenshot({ path: path.join(output, `${engine}-unapplied-colors.png`) });

      await replaceDocument('s [nil nil] > d1');
      await page.locator('.vact-play-toggle').click();
      await page.waitForFunction(() => document.querySelector('.vact-playback-state')?.textContent === 'Playing');
      assert.equal(await page.locator('.vact-run').isDisabled(), false);
      await page.waitForFunction(() => window.__vactrPerf.onsets().some(event => event.from === 3 && event.to === 6));
      await page.waitForFunction(() => window.__vactrPerf.presented().at(-1)?.activeKey.includes('3-6'));
      await page.screenshot({ path: path.join(output, `${engine}-nil-active.png`) });
      await page.locator('.vact-play-toggle').click();
      assert.equal(await page.locator('.vact-run').isDisabled(), true);

      await replaceDocument('s :analog > note [60 64 67 72] > gain 0.1 > d1');
      await page.locator('.vact-play-toggle').click();
      await page.waitForFunction(() => document.querySelector('.vact-playback-state')?.textContent === 'Playing');
      await page.waitForFunction(() => document.querySelector('.vact-position-value')?.textContent !== '-.-');
      const playing = await position.textContent();
      await page.waitForFunction((before) => document.querySelector('.vact-position-value')?.textContent !== before, playing);
      await page.locator('.vact-pause').click();
      await page.waitForFunction(() => document.querySelector('.vact-playback-state')?.textContent === 'Paused');
      await page.waitForTimeout(200); // let the authoritative pause sample arrive
      const paused = await position.textContent();
      await page.waitForTimeout(1200);
      assert.equal(await position.textContent(), paused);
      assert.equal(await page.locator('.vact-play-toggle').textContent(), 'Stop');
      await page.screenshot({ path: path.join(output, `${engine}-desktop-paused.png`) });
      await page.locator('.vact-pause').click();
      await page.waitForFunction(() => document.querySelector('.vact-playback-state')?.textContent === 'Playing');
      await page.waitForFunction((before) => document.querySelector('.vact-position-value')?.textContent !== before, paused);
      await page.locator('.vact-play-toggle').click();
      await page.waitForTimeout(300);
      assert.equal(await status.getAttribute('data-state'), 'stopped');
      assert.equal(await position.textContent(), '-.-');

      const removable = 's :analog > note [60 64 67 72] > gain 0.1 > d1';
      await replaceDocument(removable);
      await page.locator('.vact-play-toggle').click();
      await page.waitForFunction(() => document.querySelector('.vact-playback-state')?.textContent === 'Playing');
      await replaceDocument(`# ${removable}`);
      await page.locator('.vact-run').click();
      await page.waitForFunction(() => document.querySelector('.vact-eval-status')?.dataset.status === 'ok'
        && window.__vactrPerf.transportSample()?.active === false);
      assert.equal(await status.textContent(), 'Stopped');
      assert.equal(await page.locator('.vact-run').isDisabled(), true);
      const accepted = await page.evaluate(() => window.__vactrPerf.counters().highlight.accepted);
      await page.waitForTimeout(1200);
      assert.equal(await page.evaluate(() => window.__vactrPerf.counters().highlight.accepted), accepted);
      await page.screenshot({ path: path.join(output, `${engine}-comment-update-stopped.png`) });

      await page.locator('.vact-editor-options summary').click();
      assert.equal(await page.locator('.vact-editor-options').getAttribute('open'), '');
      await page.keyboard.press('Escape');
      assert.equal(await page.locator('.vact-editor-options').getAttribute('open'), null);
      await page.locator('.vact-doc-picker-toggle').click();
      await page.locator('.vact-doc-picker-filter').waitFor({ state: 'visible' });
      await page.keyboard.press('Escape');
      assert.equal(await page.locator('.vact-doc-picker').count(), 0);

      await page.locator('.vact-editor-options summary').click();
      await page.locator('.vact-vim-toggle').click();
      await page.keyboard.press('Escape');
      await input.focus();
      await input.press('x'); await input.press('u');
      const beforeLoad = await page.evaluate(() => window.__vactrPerf.doc());
      await input.press('y'); await input.press('y');
      await page.waitForFunction(expected => window.__vactrTestClipboard === expected, beforeLoad.split('\n')[0]);
      await page.locator('.vact-doc-picker-toggle').click();
      await page.locator('.vact-doc-picker-item').filter({ hasText: 'Acid' }).first().click();
      await page.getByRole('button', { name: 'Replace', exact: true }).click();
      await page.waitForFunction(before => window.__vactrPerf.doc() !== before, beforeLoad);
      const loaded = await page.evaluate(() => window.__vactrPerf.doc());
      await input.focus(); await input.press('Control+r');
      assert.equal(await page.evaluate(() => window.__vactrPerf.doc()), loaded);
      await input.press('x');
      const edited = await page.evaluate(() => window.__vactrPerf.doc());
      assert.notEqual(edited, loaded);
      await input.press('u');
      assert.equal(await page.evaluate(() => window.__vactrPerf.doc()), loaded);
      await input.press('Control+r');
      assert.equal(await page.evaluate(() => window.__vactrPerf.doc()), edited);
      await page.locator('.vact-editor-options summary').click();
      await page.locator('.vact-vim-toggle').click();
      await page.keyboard.press('Escape');

      for (const width of [1440, 768, 390]) {
        await page.setViewportSize({ width, height: 900 });
        await page.screenshot({ path: path.join(output, `${engine}-${width}.png`) });
        const overflow = await page.locator('.pane-transport').evaluate((header) => {
          const bounds = header.getBoundingClientRect();
          return [...header.querySelectorAll('button, summary, .vact-position')].filter((el) => {
            if (!el.checkVisibility()) return false;
            const r = el.getBoundingClientRect();
            return r.left < 0 || r.right > innerWidth + 1 || r.bottom > bounds.bottom + 1;
          }).map((el) => el.outerHTML);
        });
        assert.deepEqual(overflow, [], `${engine} header overflow at ${width}`);
        for (const [trigger, panel] of [
          ['.vact-editor-options summary', '.vact-editor-options .vact-header-popover'],
          ['.vact-doc-picker-toggle', '.vact-doc-picker'],
        ]) {
          await page.locator(trigger).click();
          const rect = await page.locator(panel).boundingBox();
          assert.ok(rect && rect.x >= 0 && rect.x + rect.width <= width + 1, `${engine} ${panel} at ${width}`);
          const occluded = await page.locator(panel).evaluate((element) => {
            const r = element.getBoundingClientRect();
            for (const x of [0.25, 0.5, 0.75]) for (const y of [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9]) {
              if (!element.contains(document.elementFromPoint(r.x + r.width * x, r.y + r.height * y))) return true;
            }
            return false;
          });
          assert.equal(occluded, false, `${engine} ${panel} is covered at ${width}`);
          await page.screenshot({ path: path.join(output, `${engine}-${width}-${panel.includes('popover') ? 'editor' : 'documents'}.png`) });
          await page.keyboard.press('Escape');
        }
      }
      assert.equal((await page.evaluate(() => window.__vactrSink.report())).post.peak, 0);
      results.push({ engine, pass: true, paused, widths: [1440, 768, 390] });
    } catch (error) {
      console.error(`${engine}: ${error.message}`);
      await page.screenshot({ path: path.join(output, `${engine}-failure.png`) });
      results.push({ engine, pass: false, error: String(error.stack ?? error) });
    } finally {
      await context.close();
      await browser.close();
    }
  }
} finally {
  await server.close();
}
fs.writeFileSync(path.join(output, 'results.json'), JSON.stringify(results, null, 2));
console.log(JSON.stringify(results, null, 2));
if (results.some((result) => !result.pass)) process.exitCode = 1;
