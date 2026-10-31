import { createEditor, type EmakiPlugin } from '../../src/index';
import { formatPlugin } from './format-plugin';
import { videoWidgetPlugin } from './video-widget';
import { waveWidgetPlugin } from './wave-widget';

const lines = [
  'const showcase = "emaki plugins";',
  'underlined text can annotate a line',
  'struck text can show a revision',
  'color and glow animate here',
  'video widget below',
  'waveform widget below',
  ...Array.from({ length: 54 }, (_, i) => `  line_${String(i + 7).padStart(2, '0')} = sample(${i});`),
];
const doc = lines.join('\n');
const position = (index: number) => lines.slice(0, index).reduce((sum, text) => sum + text.length + 1, 0);
const host = document.querySelector<HTMLElement>('#editor');
const controls = document.querySelector<HTMLElement>('#controls');
if (!host || !controls) throw new Error('Showcase elements missing');
const editor = createEditor(host, { doc });
const plugins: [string, () => EmakiPlugin][] = [
  ['Video', () => videoWidgetPlugin({ pos: position(4) })],
  ['Waveform', () => waveWidgetPlugin({ pos: position(5) })],
  ['Formats', formatPlugin],
];
for (const [name, factory] of plugins) {
  const label = document.createElement('label');
  const input = document.createElement('input'); input.type = 'checkbox'; input.checked = true;
  let remove = editor.addPlugin(factory());
  input.addEventListener('change', () => { if (input.checked) remove = editor.addPlugin(factory()); else remove(); });
  label.append(input, document.createTextNode(name)); controls.append(label);
}
