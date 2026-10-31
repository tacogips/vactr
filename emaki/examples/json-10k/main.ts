import { createEditor } from '../../src/index';
import { generateJsonFixture, jsonLanguage } from '../../src/language/json';
import { installPerfHook } from './perf-hook';

const params = new URLSearchParams(location.search);
const requested = Number(params.get('lines'));
const lines = [1000, 10000, 100000].includes(requested) ? requested : 10000;
const text = generateJsonFixture({ lines, seed: 1 });
const host = document.querySelector<HTMLElement>('#editor');
const count = document.querySelector<HTMLElement>('#line-count');
if (!host || !count) throw new Error('JSON editor elements are missing');
count.textContent = `${lines.toLocaleString()} lines`;
for (const button of document.querySelectorAll<HTMLButtonElement>('button[data-lines]')) {
  button.setAttribute('aria-pressed', String(Number(button.dataset.lines) === lines));
  button.addEventListener('click', () => {
    const next = new URL(location.href);
    next.searchParams.set('lines', button.dataset.lines!);
    location.assign(next.href);
  });
}
const handle = createEditor(host, { language: jsonLanguage });
const hook = installPerfHook(handle, text);
if (params.get('autoload') !== '0') void hook.load();
