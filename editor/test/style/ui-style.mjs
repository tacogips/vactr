import fs from 'node:fs';
import path from 'node:path';
import { chromium, webkit } from 'playwright';
import { editorRoot, repoRoot, startServer } from '../e2e/serve.mjs';

const outDir = path.join(repoRoot, 'tmp/ui-style/after');
fs.mkdirSync(outDir, { recursive: true });
const failures = [];
const results = [];

function fail(engine, viewport, check, selector, value) {
  failures.push({ engine, viewport, check, selector, value });
}

function makeProbe(viewportName) {
  const probe = document.createElement('div');
  probe.dataset.styleProbe = '';
  probe.dataset.styleViewport = viewportName;
  probe.setAttribute('style', 'display:flex;flex-wrap:wrap;gap:8px');
  probe.innerHTML = `
    <input type="text" aria-label="style probe text">
    <button type="button">probe</button>
    <button type="button" disabled>disabled</button>
    <button type="button" class="vact-primary">primary</button>
    <button type="button" class="vact-icon-button" aria-label="probe icon">+</button>
    <select aria-label="style probe select"><option>one</option><option>two</option></select>
    <input type="url" aria-label="style probe url">
    <label><input type="checkbox"> probe check</label>
    <input type="range" aria-label="style probe range">
    <input type="file" aria-label="style probe file">
    <label class="pkg-proxy" hidden>Proxy <input type="url" aria-label="hidden proxy probe"></label>`;
  document.querySelector('.pane-right')?.append(probe);
  return probe;
}

async function inspectPage(page, engine, viewportName, probe) {
  const observed = await page.evaluate((fixture) => {
    const viewportName = fixture.dataset.styleViewport;
    const problems = [];
    const corners = ['borderTopLeftRadius', 'borderTopRightRadius', 'borderBottomLeftRadius', 'borderBottomRightRadius'];
    const add = (check, selector, value) => problems.push({ check, selector, value });
    const isVisible = (element) => {
      const rect = element.getBoundingClientRect();
      if (!rect.width || !rect.height) return false;
      for (let node = element; node instanceof Element; node = node.parentElement) {
        const style = getComputedStyle(node);
        if (style.display === 'none' || style.visibility === 'hidden' || Number(style.opacity) <= 0) return false;
      }
      return true;
    };
    const tightSelector = '.vact-slot, .params-tabs, .params-xy-pick, .vact-eval-status';
    const tightAncestor = (element) => element.closest(tightSelector);
    const gapCheck = (first, second, label) => {
      if (!first || !second || !isVisible(first) || !isVisible(second)) return;
      const a = first.getBoundingClientRect();
      const b = second.getBoundingClientRect();
      const gap = b.left - a.right;
      const firstGroup = tightAncestor(first);
      const secondGroup = tightAncestor(second);
      const minimum = firstGroup && firstGroup === secondGroup ? 4 : 8;
      if (gap < minimum) add('gap', label, { gap, minimum });
    };
    const luminance = (color) => {
      const match = color.match(/^rgba?\(\s*([\d.]+)[, ]+([\d.]+)[, ]+([\d.]+)(?:\s*[,/]\s*([\d.]+))?\s*\)$/i);
      if (!match) return { alpha: 1, value: Number.NaN };
      const channel = match.slice(1, 4).map(Number).map((part) => {
        const srgb = part / 255;
        return srgb <= 0.04045 ? srgb / 12.92 : ((srgb + 0.055) / 1.055) ** 2.4;
      });
      return { alpha: match[4] === undefined ? 1 : Number(match[4]), value: 0.2126 * channel[0] + 0.7152 * channel[1] + 0.0722 * channel[2] };
    };
    for (const element of document.querySelectorAll('*')) {
      const style = getComputedStyle(element);
      for (const corner of corners) if (style[corner] !== '0px') add('radius', element.tagName.toLowerCase(), { property: corner, value: style[corner] });
    }
    for (const element of document.querySelectorAll('button, select, input')) {
      if (!isVisible(element)) continue;
      const style = getComputedStyle(element);
      const appearance = style.appearance || style.webkitAppearance;
      if (appearance !== 'none') add('appearance', element.tagName.toLowerCase(), appearance);
    }
    const select = fixture.querySelector('select');
    if (select && getComputedStyle(select).backgroundImage === 'none') add('select-chevron', 'probe select', 'none');
    const file = fixture.querySelector('input[type=file]');
    const fileStyle = getComputedStyle(file, '::file-selector-button');
    let filePseudo = 'checked';
    if (fileStyle.borderTopLeftRadius === '') filePseudo = 'unsupported';
    else {
      if (fileStyle.borderTopLeftRadius !== '0px') add('file-radius', '::file-selector-button', fileStyle.borderTopLeftRadius);
      const expected = getComputedStyle(fixture.querySelector('button:not([disabled])')).backgroundColor;
      if (fileStyle.backgroundColor !== expected) add('file-fill', '::file-selector-button', { actual: fileStyle.backgroundColor, expected });
    }
    for (const element of document.querySelectorAll('button, select')) {
      if (!isVisible(element)) continue;
      const fill = getComputedStyle(element).backgroundColor;
      const value = luminance(fill);
      if (value.alpha !== 0 && (!Number.isFinite(value.value) || value.value > 0.5)) add('native-fill', element.tagName.toLowerCase(), { fill, luminance: value.value });
    }
    const controls = [...document.querySelectorAll('button, select, input')].filter(isVisible).map((element) => {
      if (element instanceof HTMLInputElement && element.type === 'checkbox') return element.closest('label') ?? element;
      return element;
    }).filter((element, index, all) => all.indexOf(element) === index).map((element) => ({ element, inProbe: fixture.contains(element) }));
    for (const { element: first, inProbe: firstInProbe } of controls) {
      const a = first.getBoundingClientRect();
      const candidates = controls.filter(({ element: second, inProbe: secondInProbe }) => {
        if (second === first || firstInProbe !== secondInProbe) return false;
        const b = second.getBoundingClientRect();
        const overlap = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
        return b.left >= a.left && overlap >= Math.min(a.height, b.height) / 2;
      }).map(({ element: second }) => ({ element: second, gap: second.getBoundingClientRect().left - a.right })).sort((x, y) => x.gap - y.gap);
      if (candidates.length) gapCheck(first, candidates[0].element, `${first.tagName.toLowerCase()} -> ${candidates[0].element.tagName.toLowerCase()}`);
    }
    gapCheck(document.querySelector('.song-apply'), document.querySelector('[data-song-controls] [role=status]'), 'song-apply -> status');
    const map = document.querySelector('.vact-sample-map');
    if (map) gapCheck(map.querySelector('input'), map.querySelector('button'), 'sample map input -> button');
    for (const bank of document.querySelectorAll('.vact-bank')) {
      if (isVisible(bank)) gapCheck(bank.querySelector(':scope > span'), bank.querySelector(':scope > button'), 'bank label -> button');
    }
    if (viewportName === '1180x820-coarse') {
      for (const element of document.querySelectorAll('button, select, input')) {
        if (!isVisible(element) || (element instanceof HTMLInputElement && element.type === 'checkbox')) continue;
        const height = element.getBoundingClientRect().height;
        if (height < 36) add('coarse-target', element.tagName.toLowerCase(), height);
      }
      const label = fixture.querySelector('input[type=checkbox]').closest('label');
      if (label.getBoundingClientRect().height < 36) add('coarse-target', 'probe checkbox label', label.getBoundingClientRect().height);
    }
    fixture.querySelector('input[type=text]').focus();
    const proxyInput = [...document.querySelectorAll('.pkg-proxy input[type=url]')].find((element) => !fixture.contains(element));
    const hidden = fixture.querySelector('label.pkg-proxy[hidden]');
    const nativeHiddenDisplay = getComputedStyle(hidden).display;
    const proxy = proxyInput ? (() => {
      const style = getComputedStyle(proxyInput);
      return { present: true, visible: isVisible(proxyInput), radius: corners.map((corner) => style[corner]), background: style.backgroundColor, borderTop: style.borderTopWidth, height: proxyInput.getBoundingClientRect().height, nativeHiddenDisplay };
    })() : { present: false, visible: false, radius: null, background: null, borderTop: null, height: null, nativeHiddenDisplay };
    if (nativeHiddenDisplay !== 'none') add('proxy-hidden-display', 'label.pkg-proxy[hidden]', nativeHiddenDisplay);
    if (!proxy.present || !proxy.visible) add('proxy-present', '.pkg-proxy input[type=url]', proxy);
    if (proxy.present) {
      if (proxy.radius.some((radius) => radius !== '0px')) add('proxy-radius', '.pkg-proxy input[type=url]', proxy.radius);
      const expected = getComputedStyle(fixture.querySelector('input[type=text]')).backgroundColor;
      if (proxy.background !== expected) add('proxy-fill', '.pkg-proxy input[type=url]', { actual: proxy.background, expected });
      if (proxy.borderTop !== '1px') add('proxy-border', '.pkg-proxy input[type=url]', proxy.borderTop);
      if (viewportName === '1180x820-coarse' && proxy.height < 36) add('proxy-coarse-height', '.pkg-proxy input[type=url]', proxy.height);
    }
    return { problems, filePseudo, proxy };
  }, probe);
  for (const item of observed.problems) fail(engine, viewportName, item.check, item.selector, item.value);
  await page.keyboard.press('Tab');
  const focus = await page.evaluate(() => ({
    isProbeButton: document.activeElement === document.querySelector('[data-style-probe] button:not([disabled])'),
    outlineStyle: getComputedStyle(document.activeElement).outlineStyle,
    outlineWidth: getComputedStyle(document.activeElement).outlineWidth,
  }));
  if (!focus.isProbeButton || focus.outlineStyle !== 'solid' || focus.outlineWidth !== '2px') fail(engine, viewportName, 'focus-ring', 'data-style-probe button', focus);
  return { filePseudo: observed.filePseudo, proxy: observed.proxy };
}

const server = await startServer({ dist: path.join(editorRoot, 'dist') });
try {
  for (const [engineName, engine] of [['chromium', chromium], ['webkit', webkit]]) {
    const browser = await engine.launch(engineName === 'chromium' ? { headless: true, args: ['--mute-audio'] } : { headless: true });
    try {
      for (const viewport of [
        { name: '1440x900', width: 1440, height: 900 },
        { name: '1180x820-coarse', width: 1180, height: 820, hasTouch: true },
      ]) {
        const contextOptions = { viewport: { width: viewport.width, height: viewport.height }, deviceScaleFactor: 2, ...(viewport.hasTouch ? { hasTouch: true, isMobile: true } : {}) };
        let context;
        let coarseMode = 'native';
        try {
          context = await browser.newContext(contextOptions);
        } catch (error) {
          if (!viewport.hasTouch) throw error;
          const { isMobile, ...fallback } = contextOptions;
          context = await browser.newContext(fallback);
          coarseMode = 'emulated';
          void isMobile;
        }
        const page = await context.newPage();
        let probeAdded = false;
        try {
          await page.goto(server.origin + '/');
          await page.waitForSelector('.vact-transport');
          await page.waitForSelector('.pane-side');
          await page.evaluate(() => document.fonts.ready);
          await page.waitForTimeout(1000);
          if (viewport.hasTouch && !(await page.evaluate(() => matchMedia('(pointer: coarse)').matches))) {
            const coarseCss = await page.evaluate(() => {
              const rules = [];
              for (const sheet of document.styleSheets) {
                try {
                  for (const rule of sheet.cssRules) {
                    if (rule instanceof CSSMediaRule && rule.conditionText.includes('pointer: coarse')) rules.push([...rule.cssRules].map((child) => child.cssText).join('\n'));
                  }
                } catch {}
              }
              return rules.join('\n');
            });
            if (coarseCss) await page.addStyleTag({ content: coarseCss });
            coarseMode = 'forced';
          }
          await page.screenshot({ path: path.join(outDir, `${engineName}-${viewport.name}@2x.png`) });
          const samples = await page.evaluate(() => {
            const details = [...document.querySelectorAll('details.vact-samples')];
            window.__uiStyleOpenedDetails = details.filter((element) => !element.open);
            window.__uiStyleOpenedDetails.forEach((element) => { element.open = true; });
            return { opened: details.filter((element) => element.open).length, mapPresent: Boolean(document.querySelector('.vact-sample-map')), bankCount: document.querySelectorAll('.vact-bank').length };
          });
          const probe = await page.evaluateHandle(makeProbe, viewport.name);
          probeAdded = true;
          const inspected = await inspectPage(page, engineName, viewport.name, probe);
          const summary = { engine: engineName, viewport: viewport.name, coarseMode, filePseudo: inspected.filePseudo, samplesOpened: samples.opened, sampleMapPresent: samples.mapPresent, bankCount: samples.bankCount, proxy: inspected.proxy };
          results.push(summary);
          console.log(`${engineName} ${viewport.name}: coarse-mode=${coarseMode}, file-pseudo=${inspected.filePseudo}, samples=${samples.opened}, proxy=${inspected.proxy.present ? 'present' : 'missing'}`);
        } finally {
          if (probeAdded) await page.locator('[data-style-probe]').evaluate((element) => element.remove()).catch(() => {});
          await page.evaluate(() => window.__uiStyleOpenedDetails?.forEach((element) => { element.open = false; })).catch(() => {});
          await context.close();
        }
      }
    } finally {
      await browser.close();
    }
  }
} finally {
  await server.close();
}

fs.writeFileSync(path.join(outDir, 'report.json'), `${JSON.stringify({ results, failures }, null, 2)}\n`);
if (failures.length) {
  console.error(JSON.stringify(failures, null, 2));
  process.exitCode = 1;
} else {
  console.log(`Style assertions passed for ${results.length} engine/viewport combinations.`);
}
