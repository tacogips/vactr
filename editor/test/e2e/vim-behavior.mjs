import { installSilentSink } from './silent-sink.mjs';

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export async function runVimChecks(browser, origin, name) {
  const checks = [];
  const limitations = [];
  let context;
  let page;
  const check = async (id, action) => {
    try {
      const detail = await action();
      if (detail?.status === 'limitation') {
        limitations.push(detail.detail);
        checks.push({ id, status: 'limitation', pass: null, detail: detail.detail });
      } else checks.push({ id, pass: true, detail: detail ?? 'verified' });
    } catch (error) {
      checks.push({ id, pass: false, detail: String(error?.stack ?? error) });
    }
  };
  const openPage = async () => {
    page = await context.newPage();
    await page.goto(`${origin}/?perf=1`, { waitUntil: 'domcontentloaded', timeout: 60000 });
    await page.waitForFunction(() => Boolean(window.__vactrPerf), { timeout: 30000 });
    await page.waitForSelector('.vact-code-canvas', { timeout: 30000 });
    return page;
  };
  const enableVim = async () => {
    const toggle = page.locator('.vact-vim-toggle');
    if (await toggle.getAttribute('aria-pressed') !== 'true') await toggle.click();
    await page.waitForSelector('.vact-vim-status[data-mode="normal"]');
  };
  const resetDoc = async (text) => {
    const ta = page.locator('.vact-code-input-bridge textarea');
    await ta.focus();
    await ta.press('i');
    await ta.fill(text);
    await ta.press('Escape');
    await page.waitForFunction((expected) => window.__vactrPerf.doc() === expected, text);
    await page.waitForSelector('.vact-vim-status[data-mode="normal"]');
  };
  const press = async (...keys) => {
    const ta = page.locator('.vact-code-input-bridge textarea');
    await ta.focus();
    for (const key of keys) await ta.press(key);
  };
  const expectDoc = async (expected, label) => {
    const actual = await page.evaluate(() => window.__vactrPerf.doc());
    if (actual !== expected) throw Error(`${label}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  };

  try {
    context = await browser.newContext({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1, hasTouch: true,
      permissions: name === 'chromium' ? ['clipboard-read', 'clipboard-write'] : [] });
    await installSilentSink(context);
    await openPage();

    await check('vim-toggle', async () => {
      const toggle = page.locator('.vact-vim-toggle');
      if (await toggle.getAttribute('aria-pressed') !== 'false') throw Error('Vim toggle did not start off');
      await toggle.click();
      await page.waitForSelector('.vact-vim-status[data-mode="normal"]');
      await page.reload({ waitUntil: 'domcontentloaded' });
      await page.waitForSelector('.vact-vim-status[data-mode="normal"]');
      if (await page.locator('.vact-vim-toggle').getAttribute('aria-pressed') !== 'true') throw Error('Vim preference was not restored after reload');
      await page.locator('.vact-vim-toggle').click();
      await page.waitForSelector('.vact-vim-status', { state: 'detached' });
      if (await page.locator('.vact-vim-toggle').getAttribute('aria-pressed') !== 'false') throw Error('Vim toggle did not turn off');
      await page.locator('.vact-vim-toggle').click();
      await page.waitForSelector('.vact-vim-status[data-mode="normal"]');
      return 'default off; enable, reload persistence, disable and re-enable verified';
    });

    await check('vim-dcyp', async () => {
      await page.close(); await openPage(); await enableVim();
      await resetDoc('one two\nthree four');
      await press('g', 'g', '0', 'd', 'w'); await expectDoc('two\nthree four', 'dw');
      await press('c', 'w'); await page.locator('.vact-code-input-bridge textarea').type('foo'); await press('Escape');
      await expectDoc('foo\nthree four', 'cwfoo<Esc>');
      await press('y', 'y', 'p'); await expectDoc('foo\nfoo\nthree four', 'yyp');
      await press('P'); await expectDoc('foo\nfoo\nfoo\nthree four', 'P');
      return 'dw, cwfoo<Esc>, yyp and P document states verified';
    });

    await check('vim-visual', async () => {
      await page.close(); await openPage(); await enableVim();
      await resetDoc('alpha\nbeta\ngamma');
      await press('g', 'g', '0', 'v', 'j');
      await page.waitForSelector('.vact-vim-status[data-mode="visual"]');
      await press('d'); await expectDoc('eta\ngamma', 'vjd');
      await press('V', 'j');
      await page.waitForSelector('.vact-vim-status[data-mode="visual-line"]');
      await press('y'); await expectDoc('eta\ngamma', 'Vjy');
      await press('G', 'p'); await expectDoc('eta\ngamma\neta\ngamma', 'G then p');
      return 'visual charwise delete, linewise yank and put verified';
    });

    await check('vim-dot', async () => {
      await page.close(); await openPage(); await enableVim();
      await resetDoc('foo baz');
      await press('g', 'g', '0', 'c', 'i', 'w'); await page.locator('.vact-code-input-bridge textarea').type('bar'); await press('Escape');
      await press('w', '.'); await expectDoc('bar bar', 'ciwbar<Esc>w.');
      return 'change-inner-word and dot repeat verified';
    });

    await check('vim-ime-normal', async () => {
      await page.close(); await openPage(); await enableVim(); await resetDoc('stable');
      const before = await page.evaluate(() => ({ doc: window.__vactrPerf.doc(), revision: window.__vactrPerf.revision() }));
      const ta = page.locator('.vact-code-input-bridge textarea'); await ta.focus();
      if (name === 'chromium') {
        let session;
        try {
          session = await context.newCDPSession(page);
          await session.send('Input.imeSetComposition', { text: 'にほん', selectionStart: 3, selectionEnd: 3 });
          await session.send('Input.insertText', { text: '日本' });
        } catch (error) {
          return { status: 'limitation', detail: `Chromium CDP IME methods unavailable: ${String(error)}` };
        } finally { await session?.detach(); }
      } else {
        await ta.evaluate((el) => {
          el.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true, data: '' }));
          el.dispatchEvent(new CompositionEvent('compositionupdate', { bubbles: true, data: 'にほん' }));
          el.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true, data: '日本' }));
          el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Process', keyCode: 229, bubbles: true, cancelable: true }));
          el.dispatchEvent(new InputEvent('beforeinput', { inputType: 'insertText', data: '日本', bubbles: true, cancelable: true }));
        });
      }
      await sleep(100);
      const after = await page.evaluate(() => ({ doc: window.__vactrPerf.doc(), revision: window.__vactrPerf.revision() }));
      if (after.doc !== before.doc || after.revision !== before.revision) throw Error(`normal-mode IME changed document: ${JSON.stringify({ before, after })}`);
      if (await page.locator('.vact-vim-status').getAttribute('data-mode') !== 'normal') throw Error('IME left Vim normal mode');
      const message = await page.locator('.vact-vim-message').textContent();
      if (!message?.includes('IME input ignored')) throw Error(`IME discard was not announced: ${message}`);
      return `${name} normal-mode composition and keyCode 229 left document unchanged; discard announced`;
    });

    await check('vim-shortcut', async () => {
      await page.close(); await openPage(); await enableVim(); await resetDoc('(+ 1 2)');
      const status = page.locator('.vact-eval-status');
      const before = await status.getAttribute('data-status');
      await press(process.platform === 'darwin' ? 'Meta+Enter' : 'Control+Enter');
      await page.waitForFunction((previous) => {
        const el = document.querySelector('.vact-eval-status');
        return Boolean(el && el.dataset.status !== previous);
      }, before, { timeout: 3000 }).catch(() => {});
      const after = await status.getAttribute('data-status');
      const sample = await page.evaluate(() => window.__vactrPerf.transportSample());
      if (after === before && !sample) throw Error(`Mod-Enter did not reach eval handler (status=${before}, no transport sample)`);
      return `platform Mod-Enter reached eval path; status ${before} -> ${after}`;
    });
  } catch (error) {
    checks.push({ id: 'vim-setup', pass: false, detail: String(error?.stack ?? error) });
  } finally {
    if (page) await page.close().catch(() => {});
    await context?.close();
  }
  return { checks, limitations };
}
