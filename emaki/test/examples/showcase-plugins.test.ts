import { afterEach, describe, expect, it, vi } from 'vitest';
import { createEditor } from '../../src/index';
import { CanvasRenderer } from '../../src/render/renderer';
import { recordingGL, rasterizer } from '../support/recording-gl';
import { formatPlugin } from '../../examples/plugin-showcase/format-plugin';
import { videoWidgetPlugin } from '../../examples/plugin-showcase/video-widget';
import { waveWidgetPlugin } from '../../examples/plugin-showcase/wave-widget';

function setup() {
  let next = 0;
  const callbacks = new Map<number, FrameRequestCallback>();
  const frameHost = { devicePixelRatio: 1, innerHeight: 800,
    requestAnimationFrame(cb: FrameRequestCallback) { const id = ++next; callbacks.set(id, cb); return id; },
    cancelAnimationFrame(id: number) { callbacks.delete(id); },
    step(time: number) { const item = callbacks.entries().next().value as [number, FrameRequestCallback] | undefined;
      if (item) { callbacks.delete(item[0]); item[1](time); } },
  };
  const host = document.createElement('div'); document.body.append(host);
  host.getBoundingClientRect = () => ({ x: 0, y: 0, left: 0, top: 0, right: 500, bottom: 300, width: 500, height: 300, toJSON() {} });
  const lines: number[][] = [];
  const context = { font: '', fillStyle: '', strokeStyle: '', lineWidth: 1,
    scale() {}, clearRect() {}, fillRect() {}, beginPath() {}, moveTo() {}, stroke() {}, fillText() {},
    lineTo(x: number, y: number) { lines.push([x, y]); }, measureText(text: string) { return { width: text.length * 8 }; },
  };
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(() => context as unknown as CanvasRenderingContext2D);
  const gl = recordingGL(); const raster = rasterizer();
  const editor = createEditor(host, { doc: 'bold keyword\nunderlined span\nstruck span\ncolored glow text\nlast line', gl: gl.gl, frameHost, createCanvas: raster.createCanvas });
  return { host, editor, frameHost, lines };
}
afterEach(() => { document.body.replaceChildren(); vi.restoreAllMocks(); });

describe('showcase plugins', () => {
  it('passes static marks, line formats and animated glow to the renderer', () => {
    const render = vi.spyOn(CanvasRenderer.prototype, 'render'); const { editor, frameHost, host } = setup();
    editor.addPlugin(formatPlugin()); frameHost.step(10);
    const input = render.mock.calls.at(-1)![0]!;
    expect(input.marks).toEqual(expect.arrayContaining([
      expect.objectContaining({ style: expect.objectContaining({ fontWeight: 'bold' }) }),
      expect.objectContaining({ style: expect.objectContaining({ underline: expect.anything() }) }),
      expect.objectContaining({ style: expect.objectContaining({ strike: expect.anything() }) }),
      expect.objectContaining({ style: expect.objectContaining({ color: expect.anything() }) }),
    ]));
    expect(input.lineFormats).toHaveLength(3);
    expect(input.lineFormats).toEqual(expect.arrayContaining([
      expect.objectContaining({ style: expect.objectContaining({ bar: expect.anything() }) }),
      expect.objectContaining({ style: expect.objectContaining({ background: expect.anything() }) }),
      expect.objectContaining({ style: expect.objectContaining({ gutterMarker: expect.objectContaining({ text: '*' }) }) }),
    ]));
    expect(input.animatedMarks).toEqual([expect.objectContaining({ style: expect.objectContaining({ glow: expect.anything() }) })]);
    expect(host.querySelector('.emaki-widget canvas')).not.toBeNull();
    editor.destroy();
  });
  it('draws a visible 64px waveform with phase varying by frame time', () => {
    const { editor, frameHost, lines } = setup(); editor.addPlugin(waveWidgetPlugin({ pos: 0 }));
    frameHost.step(10); expect(editor.lineTop(1)).toBe(82); expect(lines.length).toBeGreaterThan(0);
    const first = lines.slice(); lines.length = 0; frameHost.step(510);
    expect(lines.length).toBeGreaterThan(0); expect(lines).not.toEqual(first); editor.destroy();
  });
  it('shows the canvas fallback and cancels its animation when unmounted', () => {
    const cancel = vi.spyOn(window, 'cancelAnimationFrame');
    const { editor, frameHost, host } = setup(); const remove = editor.addPlugin(videoWidgetPlugin({ pos: 0 }));
    frameHost.step(10); expect(host.querySelector('.emaki-widget [data-fallback]')).not.toBeNull();
    remove(); frameHost.step(20); expect(cancel).toHaveBeenCalled(); editor.destroy();
  });
});
