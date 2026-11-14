export async function runVimKeymapChecks(browser, origin, name) {
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
    await page.waitForSelector('.vact-vim-status[data-mode="normal"]');
  };
  const freshPage = async () => {
    await page.close();
    await openPage();
  };
  const press = async (...keys) => {
    const textarea = page.locator('.vact-code-input-bridge textarea');
    await textarea.focus();
    for (const key of keys) await textarea.press(key);
  };
  const openPanel = async () => {
    await page.locator('.vact-vim-keymap-button').click();
    await page.waitForSelector('.vact-vim-keymap');
  };
  const applyPreset = async () => {
    await openPanel();
    await page.getByRole('button', { name: 'mise-darwin' }).click();
    await page.locator('.vact-vim-keymap-apply').click();
    await page.waitForFunction(() => {
      const raw = localStorage.getItem('vactr.vimKeymap');
      if (!raw) return false;
      try { return JSON.parse(raw).version === 1; } catch { return false; }
    });
    if (await page.locator('.vact-vim-keymap-issues li').count() !== 0) throw Error('mise-darwin preset has validation issues');
    await page.locator('.vact-vim-keymap-close').click();
  };
  const resetDoc = async (text) => {
    const textarea = page.locator('.vact-code-input-bridge textarea');
    await textarea.focus();
    await textarea.press('i');
    await textarea.press('ControlOrMeta+a');
    await textarea.press('Backspace');
    await page.waitForFunction(() => window.__vactrPerf.doc() === '');
    await textarea.fill(text);
    await textarea.press('Escape');
    await page.waitForFunction((expected) => window.__vactrPerf.doc() === expected, text);
    await page.waitForSelector('.vact-vim-status[data-mode="normal"]');
  };

  try {
    context = await browser.newContext({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1 });
    await context.addInitScript(() => localStorage.setItem('vactr.vimMode', 'true'));
    await openPage();

    await check('vim-keymap-preset', async () => {
      await applyPreset();
      return 'mise-darwin schema version 1 saved with no validation issues';
    });

    await check('vim-keymap-kj', async () => {
      await freshPage();
      await resetDoc('');
      await press('i');
      const textarea = page.locator('.vact-code-input-bridge textarea');
      await textarea.type('ab');
      await press('k', 'j');
      await page.waitForSelector('.vact-vim-status[data-mode="normal"]');
      const doc = await page.evaluate(() => window.__vactrPerf.doc());
      if (doc !== 'ab') throw Error(`kj inserted unexpected text: ${JSON.stringify(doc)}`);
      return 'insert-mode kj exits without inserting either mapping key';
    });

    await check('vim-keymap-hints', async () => {
      await freshPage();
      await press('Space');
      await page.waitForSelector('.vact-vim-hints', { timeout: 3000 });
      const hints = page.locator('.vact-vim-hints');
      const text = await hints.innerText();
      if (!/(^|\n)y(\s|$)/m.test(text) || !/(^|\n)p(\s|$)/m.test(text)) throw Error(`leader hints missing y or p: ${text}`);
      const summary = await page.locator('.vact-vim-hints-summary').textContent();
      if (!summary?.trim()) throw Error('hint aria-live summary is empty');
      await press('Escape');
      await page.waitForSelector('.vact-vim-hints', { state: 'detached' });
      return 'leader hints show y/p and an aria-live summary, then Escape closes the panel';
    });

    await check('vim-keymap-hop', async () => {
      await freshPage();
      await resetDoc('ab\ncd\nem');
      await press('g', 'g', '0', 'g', 'j', 'm');
      const pending = await page.locator('.vact-vim-pending').textContent();
      if (!pending?.trim().toLowerCase().startsWith('hop')) throw Error(`expected hop target prompt, got ${JSON.stringify(pending)}`);
      await press('f');
      await page.waitForFunction(() => window.__vactrPerf.selection().head === 7);
      return 'gj selects the single m target and its f label moves the cursor to position 7';
    });

    await check('vim-keymap-hop-off', async () => {
      await freshPage();
      await openPanel();
      const editor = page.locator('.vact-vim-keymap-text');
      const edited = await editor.evaluate((element) => {
        const document = JSON.parse(element.value);
        document.hop.enabled = false;
        return JSON.stringify(document, null, 2);
      });
      await editor.fill(edited);
      await page.locator('.vact-vim-keymap-apply').click();
      await page.locator('.vact-vim-keymap-close').click();
      await resetDoc('ab\ncd');
      await press('g', 'g', '0', 'g', 'j');
      const pending = await page.locator('.vact-vim-pending').textContent();
      if (pending?.trim().toLowerCase().startsWith('hop')) throw Error(`hop prompt remained after disabling: ${pending}`);
      await page.waitForFunction(() => window.__vactrPerf.selection().head === 3);
      return 'disabled hop leaves gj as a plain line motion and shows no hop prompt';
    });
  } catch (error) {
    checks.push({ id: 'vim-keymap-setup', pass: false, detail: String(error?.stack ?? error) });
  } finally {
    try {
      if (page) {
        if (!(await page.locator('.vact-vim-keymap').count())) await openPanel();
        await page.locator('.vact-vim-keymap-reset').click();
        await page.waitForFunction(() => localStorage.getItem('vactr.vimKeymap') === null);
      }
    } catch (error) {
      checks.push({ id: 'vim-keymap-reset', pass: false, detail: String(error?.stack ?? error) });
    }
    await page?.close().catch(() => {});
    await context?.close();
  }
  return { checks, limitations };
}
