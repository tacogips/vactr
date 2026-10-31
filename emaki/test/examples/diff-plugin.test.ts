import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createEditor, type Rgba } from '../../src/index';
import { CanvasRenderer } from '../../src/render/renderer';
import { createDiffPair } from '../../examples/diff/diff-plugin';
import { rasterizer, recordingGL } from '../support/recording-gl';

const colors: { add: Rgba; del: Rgba; addStrong: Rgba; delStrong: Rgba } = {
  add: [0, 0.4, 0, 1], del: [0.4, 0, 0, 1], addStrong: [0, 0.8, 0, 1], delStrong: [0.8, 0, 0, 1],
};
function setup(doc: string) {
  let next = 0; const callbacks = new Map<number, FrameRequestCallback>();
  const frameHost = { devicePixelRatio: 1, innerHeight: 800,
    requestAnimationFrame(cb: FrameRequestCallback) { const id = ++next; callbacks.set(id, cb); return id; },
    cancelAnimationFrame(id: number) { callbacks.delete(id); },
    step() { const item = callbacks.entries().next().value as [number, FrameRequestCallback] | undefined; if (item) { callbacks.delete(item[0]); item[1](10); } },
  };
  const host = document.createElement('div'); document.body.append(host);
  host.getBoundingClientRect = () => ({ x: 0, y: 0, left: 0, top: 0, right: 500, bottom: 100, width: 500, height: 100, toJSON() {} });
  const raster = rasterizer(), gl = recordingGL();
  const editor = createEditor(host, { doc, gl: gl.gl, frameHost, createCanvas: raster.createCanvas });
  return { editor, frameHost };
}
function stepBoth(left: ReturnType<typeof setup>, right: ReturnType<typeof setup>) {
  right.frameHost.step(); left.frameHost.step(); right.frameHost.step(); left.frameHost.step();
}
beforeEach(() => {
  const ctx = { font: '', measureText: (s: string) => ({ width: s.length * 8 }), scale() {}, clearRect() {}, fillRect() {}, beginPath() {}, moveTo() {}, lineTo() {}, stroke() {} };
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(ctx as unknown as CanvasRenderingContext2D);
});
afterEach(() => { document.body.replaceChildren(); vi.restoreAllMocks(); });

describe('diff pair', () => {
  it('applies line backgrounds and word marks and aligns an inserted row', () => {
    const render = vi.spyOn(CanvasRenderer.prototype, 'render');
    const left = setup('a\nb\nc'), right = setup('a\nx\nc\nd');
    const pair = createDiffPair({ left: left.editor, right: right.editor, colors });
    stepBoth(left, right);
    const inputs = render.mock.calls.map(call => call[0]!);
    expect(inputs.some(input => input.lineFormats?.some(f => f.line === 1 && JSON.stringify(f.style.background) === JSON.stringify(colors.del)))).toBe(true);
    expect(inputs.some(input => input.lineFormats?.some(f => f.line === 1 && JSON.stringify(f.style.background) === JSON.stringify(colors.add)))).toBe(true);
    expect(inputs.some(input => input.lineFormats?.some(f => f.line === 3 && JSON.stringify(f.style.background) === JSON.stringify(colors.add)))).toBe(true);
    expect(inputs.some(input => input.marks?.some(m => JSON.stringify(m.style.background) === JSON.stringify(colors.delStrong)))).toBe(true);
    expect(inputs.some(input => input.marks?.some(m => JSON.stringify(m.style.background) === JSON.stringify(colors.addStrong)))).toBe(true);
    expect(pair.stats).toMatchObject({ modified: 1, added: 1, fillerRowsLeft: 1, fillerRowsRight: 0 });
    expect(left.editor.lineTop(2)).toBe(36);
    expect(left.editor.lineAtHeight(54)).toBe(2);
    pair.dispose(); left.editor.destroy(); right.editor.destroy();
  });
  it('coalesces edits until the next frame', () => {
    const left = setup('a\nb\nc'), right = setup('a\nb\nc');
    const pair = createDiffPair({ left: left.editor, right: right.editor, colors });
    stepBoth(left, right); const revision = pair.stats.diffRevision;
    right.editor.dispatch({ changes: { from: 0, insert: 'x' } });
    right.editor.dispatch({ changes: { from: 0, insert: 'y' } });
    expect(pair.stats.diffRevision).toBe(revision);
    right.frameHost.step();
    expect(pair.stats.diffRevision).toBe(revision + 1);
    expect(pair.stats.rightRevision).toBe(right.editor.revision);
    pair.dispose(); left.editor.destroy(); right.editor.destroy();
  });
  it('follows caret-driven scroll', () => {
    const doc = Array.from({ length: 100 }, (_, n) => `line ${n}`).join('\n');
    const left = setup(doc), right = setup(doc);
    const pair = createDiffPair({ left: left.editor, right: right.editor, colors });
    stepBoth(left, right);
    right.editor.dispatch({ selection: { anchor: right.editor.state.doc.line(90).from } });
    stepBoth(left, right); stepBoth(left, right);
    expect(right.editor.scrollTop).toBeGreaterThan(0);
    expect(left.editor.scrollTop).toBe(right.editor.scrollTop);
    pair.dispose(); left.editor.destroy(); right.editor.destroy();
  });
  it('synchronizes scroll without feedback recursion', () => {
    const doc = Array.from({ length: 100 }, (_, n) => `line ${n}`).join('\n');
    const left = setup(doc), right = setup(doc);
    const pair = createDiffPair({ left: left.editor, right: right.editor, colors });
    stepBoth(left, right);
    const toLeft = vi.spyOn(left.editor, 'scrollTo'), toRight = vi.spyOn(right.editor, 'scrollTo');
    left.editor.scrollTo({ top: 300 });
    expect(toLeft).toHaveBeenCalledTimes(1); expect(toRight).toHaveBeenCalledTimes(1);
    expect(left.editor.scrollTop).toBe(300); expect(right.editor.scrollTop).toBe(300);
    pair.dispose(); left.editor.destroy(); right.editor.destroy();
  });
});
