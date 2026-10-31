import { createEditor, type Rgba } from '../../src/index';
import { generateJsonFixture, jsonLanguage } from '../../src/language/json';
import { createDiffPair } from './diff-plugin';
import { installDiffHook } from './hook';

const requestedLines = Number(new URLSearchParams(location.search).get('lines') ?? 400);
const lines = Number.isInteger(requestedLines) && requestedLines >= 3 ? requestedLines : 400;
const master = generateJsonFixture({ lines, seed: 1 });
const working = master.split('\n');
if (working.length > 10) working[9] += ' changed';
if (working.length > 30) working.splice(29, 1);
if (working.length > 50) working.splice(49, 0, '    "new item": true,', '    "another item": false,');

function color(token: string): Rgba {
  const value = getComputedStyle(document.documentElement).getPropertyValue(token).trim();
  const hex = /^#([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(value);
  if (!hex) throw new Error(`Expected a six-digit hex color for ${token}`);
  const rgb = Number.parseInt(hex[1]!, 16);
  return [(rgb >> 16 & 255) / 255, (rgb >> 8 & 255) / 255, (rgb & 255) / 255,
    hex[2] ? Number.parseInt(hex[2], 16) / 255 : 1];
}
const leftHost = document.querySelector<HTMLElement>('#left-editor');
const rightHost = document.querySelector<HTMLElement>('#right-editor');
if (!leftHost || !rightHost) throw new Error('Diff editor hosts are missing');
const left = createEditor(leftHost, { doc: master, language: jsonLanguage, readOnly: true, lineNumbers: true });
const right = createEditor(rightHost, { doc: working.join('\n'), language: jsonLanguage, lineNumbers: true });
const pair = createDiffPair({ left, right, colors: {
  add: color('--emaki-diff-add'), del: color('--emaki-diff-del'),
  addStrong: color('--emaki-diff-add-strong'), delStrong: color('--emaki-diff-del-strong'),
} });
installDiffHook(left, right, pair);
