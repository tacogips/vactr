import { afterEach, describe, expect, it, vi } from 'vitest';
import { createEditor } from '../src/editor';
import { jsonLanguage } from '../src/language/json';
import { CanvasRenderer } from '../src/render/renderer';
import { recordingGL, rasterizer } from './support/recording-gl';

function setup(options: Parameters<typeof createEditor>[1] = {}) {
  let next = 0; const callbacks = new Map<number, FrameRequestCallback>();
  const frameHost = { devicePixelRatio: 1, innerHeight: 800, requestAnimationFrame(cb: FrameRequestCallback) { const id = ++next; callbacks.set(id, cb); return id; },
    cancelAnimationFrame(id: number) { callbacks.delete(id); }, step() { const item = callbacks.entries().next().value as [number, FrameRequestCallback] | undefined; if (item) { callbacks.delete(item[0]); item[1](10); } } };
  const host = document.createElement('div'); document.body.append(host);
  host.getBoundingClientRect = () => ({ x: 0, y: 0, left: 0, top: 0, right: 500, bottom: 300, width: 500, height: 300, toJSON() {} });
  const raster = rasterizer(); const gl = recordingGL();
  const editor = createEditor(host, { doc: 'a\nb', gl: gl.gl, frameHost, createCanvas: raster.createCanvas, ...options });
  return { host, editor, frameHost, gl };
}
afterEach(() => { document.body.replaceChildren(); vi.restoreAllMocks(); });
describe('createEditor', () => {
  it('renders, reports presentation and input, and disposes idempotently', () => {
    const { host, editor, frameHost } = setup(); const present = vi.fn(), input = vi.fn();
    editor.perf.onPresent(present); editor.perf.onInput(input);
    expect(host.querySelectorAll('canvas')).toHaveLength(1); const textarea = host.querySelector('textarea')!; expect(textarea).toBeTruthy();
    frameHost.step(); expect(present).toHaveBeenLastCalledWith(expect.objectContaining({ revision: 0 }));
    textarea.dispatchEvent(new InputEvent('beforeinput', { bubbles: true, cancelable: true, inputType: 'insertText', data: 'x' }));
    expect(editor.revision).toBe(1); expect(input).toHaveBeenCalledWith(expect.objectContaining({ revisionAfter: 1, workMs: expect.any(Number) }));
    expect(input.mock.calls[0]![0].workMs).toBeGreaterThanOrEqual(0);
    frameHost.step(); expect(present).toHaveBeenLastCalledWith(expect.objectContaining({ revision: 1 }));
    editor.destroy(); editor.destroy(); expect(host.children).toHaveLength(0);
  });
  it('tags input samples with their event type and the revision before the event', () => {
    const { host, editor } = setup(); const input = vi.fn(); editor.perf.onInput(input);
    const textarea = host.querySelector('textarea')!;
    textarea.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'x' }));
    textarea.dispatchEvent(new InputEvent('beforeinput', { bubbles: true, cancelable: true, inputType: 'insertText', data: 'x' }));
    const [keydown, beforeinput] = input.mock.calls.map(call => call[0]);
    expect(keydown).toEqual(expect.objectContaining({ type: 'keydown', revisionBefore: 0, revisionAfter: 0 }));
    expect(beforeinput).toEqual(expect.objectContaining({ type: 'beforeinput', revisionBefore: 0, revisionAfter: 1 }));
  });
  it('rejects user input in read-only mode but accepts programmatic dispatch', () => {
    const { host, editor } = setup({ readOnly: true });
    host.querySelector('textarea')!.dispatchEvent(new InputEvent('beforeinput', { bubbles: true, cancelable: true, inputType: 'insertText', data: 'x' }));
    expect(editor.state.doc.toString()).toBe('a\nb'); editor.dispatch({ changes: { from: 0, insert: 'x' } }); expect(editor.state.doc.toString()).toBe('xa\nb'); editor.destroy();
  });
  it('maps block widgets into the heightmap', () => {
    const { editor, frameHost } = setup();
    editor.addPlugin({ name: 'widget', attach(host) { host.widgets('x').set([{ key: 'x', pos: 0, side: 'below', height: 40, content: { kind: 'dom', mount: () => () => {} } }]); } });
    frameHost.step(); expect(editor.lineTop(1)).toBe(58); editor.destroy();
  });
  it('passes JSON tokens into rendering', () => {
    const render = vi.spyOn(CanvasRenderer.prototype, 'render'); const { editor, frameHost } = setup({ doc: '{"key": 1}', language: jsonLanguage });
    frameHost.step(); expect(render.mock.calls[0]![0]!.tokens).toEqual(expect.arrayContaining([expect.objectContaining({ tag: 'json-key' })])); editor.destroy();
  });
  it('keeps frames active for an animated plugin and stops after removal', () => {
    const { editor, frameHost } = setup(); const tick = vi.fn(() => true);
    const remove = editor.addPlugin({ name: 'animation', attach: () => ({ frame: tick }) });
    frameHost.step(); frameHost.step(); expect(tick).toHaveBeenCalledTimes(2);
    remove(); frameHost.step(); expect(tick).toHaveBeenCalledTimes(2); editor.destroy();
  });
  it('reports incomplete span highlighting and notifies the provider on edits', () => {
    const noteChanges = vi.fn(); let invalidate: (() => void) | undefined;
    const language = { kind: 'spans' as const, name: 'test', spans: () => ({ spans: [], truncated: true }), noteChanges,
      setInvalidate: (cb: () => void) => { invalidate = cb; } };
    const { editor, frameHost } = setup({ language }); const present = vi.fn(); editor.perf.onPresent(present);
    frameHost.step(); expect(present).toHaveBeenCalledWith(expect.objectContaining({ visibleComplete: false }));
    editor.dispatch({ changes: { from: 0, insert: 'z' } }); expect(noteChanges).toHaveBeenCalledOnce();
    invalidate!(); frameHost.step(); editor.destroy();
  });
  it('supports enabling and disabling line numbers', () => {
    const a = setup({ lineNumbers: true }); a.frameHost.step(); expect(a.editor.stats.gutterBuilds).toBeGreaterThan(0); a.editor.destroy();
    const b = setup({ lineNumbers: false }); b.frameHost.step(); expect(b.editor.stats.gutterBuilds).toBe(0); b.editor.destroy();
  });
});
