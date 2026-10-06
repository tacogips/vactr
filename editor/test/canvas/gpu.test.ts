import { Text } from '@codemirror/state';
import { describe, expect, it, vi } from 'vitest';
import { TextLayout, type LayoutFont } from '../../src/code/layout';
import { GlyphAtlas, type CanvasFactory } from '../../src/code/atlas';
import { CanvasRenderer, GPU_TOKEN_COLORS } from '../../src/code/renderer';
import { MiB, ResourceLedger, RESOURCE_LIMITS, effectiveSize } from '../../src/code/resources';
import type { CodeAnnotation } from '../../src/app/apis';
import { FALLBACK_PALETTE, readPalette, rgbaCss } from '../../src/code/palette';

const font: LayoutFont = { font: '16px test-mono', fallback: 'test-ja', lineHeight: 20, baseline: 16 };
function width(text: string): number {
  let result = 0;
  for (const { segment } of new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(text)) result += /[日語本]/u.test(segment) ? 16 : /\p{Extended_Pictographic}/u.test(segment) ? 16 : 8;
  return result - (text.match(/ffi/g)?.length ?? 0) * 6;
}
function layout(text = '', cacheBytes?: number): TextLayout {
  const l = new TextLayout({ font: '', measureText: text => ({ width: width(text) }) }, font, cacheBytes); l.setDocument(text); return l;
}
function oldBoundary(text: string, pos: number, bias: -1 | 1): number {
  pos = Math.max(0, Math.min(text.length, pos));
  const starts = [...text.matchAll(/\r\n|\r|\n/g)];
  let from = 0, to = text.length, next = text.length;
  for (const match of starts) {
    if (pos < match.index! + match[0].length) { to = match.index!; next = to + match[0].length; break; }
    from = match.index! + match[0].length;
  }
  if (pos > to) return bias < 0 ? to : next;
  const relative = pos - from, line = text.slice(from, to);
  for (const part of new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(line)) {
    const end = part.index + part.segment.length;
    if (part.index === relative || end === relative) return pos;
    if (part.index < relative && relative < end) return from + (bias < 0 ? part.index : end);
  }
  return pos;
}
const view = { width: 320, height: 100, scrollLeft: 0, scrollTop: 0, left: 10, top: 20, gutter: 48 };
function rasterizer(): { createCanvas: CanvasFactory; text: { text: string; x: number; y: number; font: string; color: string }[]; crops: number[][]; drawImages: number; canvases: number } {
  const text: { text: string; x: number; y: number; font: string; color: string }[] = []; const crops: number[][] = [];
  let drawImages = 0; let canvases = 0;
  return { text, crops,
    createCanvas: () => {
      canvases++;
      const canvas = document.createElement('canvas');
      const ctx = { font: '', fillStyle: '', textBaseline: '', setTransform() {}, clearRect() {}, save() {}, restore() {}, beginPath() {}, clip() {}, drawImage() { drawImages++; },
        rect(...args: number[]) { crops.push(args); },
        getImageData(_x: number, _y: number, w: number, h: number) { return { data: new Uint8ClampedArray(w * h * 4) }; },
        measureText(t: string) { return { width: width(t) }; },
        fillText(t: string, x: number, y: number) { text.push({ text: t, x, y, font: this.font, color: this.fillStyle }); },
      };
      Object.defineProperty(canvas, 'getContext', { value: () => ctx }); return canvas;
    },
    get drawImages() { return drawImages; }, get canvases() { return canvases; },
  };
}
function recordingGL(maxTexture = 1024) {
  let next = 1; let error = 0; let failUpload = false;
  const live = new Map<object, string>(); const deleted: string[] = []; const calls: { name: string; args: unknown[] }[] = [];
  let rect: number[] = []; let color: number[] = []; let bound: object | null = null; let scissor = false;
  const draws: { rect: number[]; color: number[]; texture: object | null; textureLiveAtDraw: boolean; scissor: boolean }[] = [];
  const gl: Record<string, unknown> = {};
  ['MAX_TEXTURE_SIZE', 'TEXTURE_2D', 'TEXTURE_MIN_FILTER', 'TEXTURE_MAG_FILTER', 'LINEAR', 'NEAREST', 'TEXTURE_WRAP_S', 'TEXTURE_WRAP_T', 'CLAMP_TO_EDGE', 'UNPACK_PREMULTIPLY_ALPHA_WEBGL', 'RGBA', 'UNSIGNED_BYTE', 'VERTEX_SHADER', 'FRAGMENT_SHADER', 'COMPILE_STATUS', 'LINK_STATUS', 'ARRAY_BUFFER', 'STATIC_DRAW', 'FLOAT', 'FRAMEBUFFER', 'SCISSOR_TEST', 'BLEND', 'ONE', 'ONE_MINUS_SRC_ALPHA', 'COLOR_BUFFER_BIT', 'TEXTURE0', 'TRIANGLES'].forEach((name, i) => gl[name] = i + 1);
  gl.NO_ERROR = 0;
  for (const kind of ['Texture', 'Shader', 'Program', 'Buffer', 'VertexArray']) {
    gl[`create${kind}`] = () => { const object = { id: next++ }; live.set(object, kind); return object; };
    gl[`delete${kind}`] = (obj: object) => { expect(live.get(obj)).toBe(kind); live.delete(obj); deleted.push(kind); };
  }
  for (const name of ['texParameteri', 'pixelStorei', 'shaderSource', 'compileShader', 'attachShader', 'linkProgram', 'bindVertexArray', 'bindBuffer', 'bufferData', 'enableVertexAttribArray', 'vertexAttribPointer', 'bindFramebuffer', 'useProgram', 'viewport', 'disable', 'enable', 'blendFunc', 'clearColor', 'clear', 'uniform2f', 'uniform1i', 'activeTexture', 'scissor']) gl[name] = (...args: unknown[]) => {
    if (args[0] === gl.SCISSOR_TEST) scissor = name === 'enable';
    calls.push({ name, args });
  };
  gl.getParameter = (...args: unknown[]) => { calls.push({ name: 'getParameter', args }); return maxTexture; }; gl.getShaderParameter = () => true; gl.getProgramParameter = () => true;
  gl.isTexture = (obj: object) => live.get(obj) === 'Texture';
  gl.getShaderInfoLog = () => ''; gl.getProgramInfoLog = () => ''; gl.getAttribLocation = () => 0; gl.getUniformLocation = (_p: object, name: string) => ({ name });
  gl.isContextLost = () => false;
  gl.bindTexture = (_target: number, texture: object | null) => { bound = texture; };
  gl.uniform4f = (loc: { name: string }, ...args: number[]) => { if (loc.name === 'u_rect') rect = args; if (loc.name === 'u_color') color = args; };
  gl.drawArrays = () => draws.push({ rect: [...rect], color: [...color], texture: bound, textureLiveAtDraw: bound !== null && live.get(bound) === 'Texture', scissor });
  for (const name of ['texImage2D', 'texSubImage2D']) gl[name] = (...args: unknown[]) => { calls.push({ name, args }); if (failUpload) { error = 1285; failUpload = false; } };
  gl.getError = () => { const result = error; error = 0; return result; };
  return { gl: gl as unknown as WebGL2RenderingContext, live, deleted, calls, draws, failNextUpload: () => { failUpload = true; }, lose: () => live.clear() };
}

describe('GPU resource admission', () => {
  it('enforces total and category caps without changing counters on refusal', () => {
    const b = new ResourceLedger(); const a = b.allocate('atlas', 16 * MiB)!;
    expect(b.allocate('atlas', 1)).toBeNull(); expect(b.usedBytes).toBe(16 * MiB);
    const rest = b.allocate('other', 80 * MiB)!; expect(b.reserve(1)).toBe(false);
    rest.release(); a.release(); a.release(); expect(b.usedBytes).toBe(0);
    expect(b.reserve(32)).toBe(true); b.release(32); expect(b.usedBytes).toBe(0);
    expect(() => b.release(1)).toThrow('Unowned');
  });
  it('rejects invalid values and sized resources before admission', () => {
    const b = new ResourceLedger();
    for (const n of [NaN, Infinity, -1, 0.1]) expect(b.allocate('other', n)).toBeNull();
    expect(b.allocate('video', 1280 * 721 * 4, { width: 1280, height: 721 })).toBeNull();
    expect(b.allocate('visualTarget', 1025 * 1024 * 4, { width: 1025, height: 1024 })).toBeNull();
    expect(b.allocate('canvas', 4)).toBeNull(); expect(b.allocate('canvas', 4, { width: 2, height: 2 })).toBeNull();
    expect(b.allocate('geometry', 8 * MiB + 1)).toBeNull(); expect(b.allocate('visualText', 8 * MiB + 1)).toBeNull(); expect(b.usedBytes).toBe(0);
  });
  it('limits eight visual targets, one video and shared canvas/backdrop pixels', () => {
    const b = new ResourceLedger(); const reservations = [];
    for (let i = 0; i < 8; i++) reservations.push(b.allocate('visualTarget', 1024 ** 2 * 4, { width: 1024, height: 1024 })!);
    expect(b.allocate('visualTarget', 4, { width: 1, height: 1 })).toBeNull();
    reservations.push(b.allocate('canvas', 2_000_000 * 4, { width: 2000, height: 1000 })!);
    reservations.push(b.allocate('backdrop', 2_000_000 * 4, { width: 2000, height: 1000 })!);
    expect(b.allocate('canvas', 4, { width: 1, height: 1 })).toBeNull();
    reservations.push(b.allocate('video', 1280 * 720 * 4, { width: 1280, height: 720 })!);
    expect(b.allocate('video', 4, { width: 1, height: 1 })).toBeNull();
    for (const r of reservations) r.release(); expect(b.counters.pixels).toBe(0); expect(b.counters.visualTargets).toBe(0); expect(b.usedBytes).toBe(0);
  });
  it('reserves transient replacements and leaves the old ownership intact on refusal', () => {
    const b = new ResourceLedger(100); const old = b.allocate('other', 80)!;
    expect(b.allocate('other', 80)).toBeNull(); expect(b.usedBytes).toBe(80);
    old.release(); expect(b.allocate('other', 80)).not.toBeNull();
  });
  it('reduces DPR against both dimensions and aggregate pixels', () => {
    const s = effectiveSize(2000, 2000, 2, 4096); expect(s.width * s.height).toBeLessThanOrEqual(4_000_000); expect(s.dpr).toBe(1); expect(s.reduced).toBe(true);
    const tiny = effectiveSize(1000, 10, 2, 128); expect(tiny.width).toBe(128);
    expect(() => effectiveSize(0, 10, 1, 1024)).toThrow();
  });
});

describe('shaped UTF-16 layout', () => {
  it('renders Text-backed and string-backed documents equivalently, including CRLF and CR line breaks', () => {
    const raw = 'alpha\r\nbeta\rgamma';
    const fromString = layout(raw), fromText = new TextLayout({ font: '', measureText: text => ({ width: width(text) }) }, font);
    fromText.setText(Text.of(['alpha', 'beta', 'gamma']));
    expect(fromText.lineCount).toBe(fromString.lineCount);
    for (let n = 0; n < 3; n++) {
      expect(fromText.shape(n).runs.map(run => run.text)).toEqual(fromString.shape(n).runs.map(run => run.text));
      expect(fromText.coordsAtPos(fromText.shape(n).from, view)).toEqual(fromString.coordsAtPos(fromString.shape(n).from, view));
    }
  });
  it('keeps CRLF offsets, one break and four-space tab stops', () => {
    const l = layout('a\tb\r\n日語'); expect(l.lineCount).toBe(2);
    expect(l.shape(0).width).toBe(40); expect(l.shape(1).from).toBe(5);
    expect(l.coordsAtPos(2, view)?.left).toBe(90); expect(l.coordsAtPos(5, view)?.top).toBe(40);
    expect(l.boundary(4, -1)).toBe(3); expect(l.boundary(4, 1)).toBe(5);
  });
  it('never returns a surrogate, combining or ZWJ interior', () => {
    const l = layout('a😀e\u0301👨‍👩‍👧‍👦z');
    expect(l.boundary(2)).toBe(1); expect(l.boundary(2, 1)).toBe(3); expect(l.boundary(4)).toBe(3); expect(l.boundary(4, 1)).toBe(5);
    const valid = new Set([0, 1, 3, 5, 16, 17]);
    for (let x = 58; x < 150; x++) expect(valid.has(l.posAtCoords({ x, y: 21 }, view)!)).toBe(true);
  });
  it('keeps cached and fallback boundaries identical to the previous grapheme loop', () => {
    for (const text of ['plain ASCII', '日本語の行', '👨‍👩‍👧‍👦x', 'e\u0301x', 'a😀b', 'one\r\ntwo\rthree']) {
      const l = layout(text); l.shape(0);
      for (let pos = 0; pos <= text.length; pos++) for (const bias of [-1, 1] as const) {
        expect(l.boundary(pos, bias), `cached ${JSON.stringify(text)} ${pos}/${bias}`).toBe(oldBoundary(text, pos, bias));
      }
      if (text === 'plain ASCII') expect(l.stats.segmentations).toBe(0);
      l.invalidate();
      for (let pos = 0; pos <= text.length; pos++) for (const bias of [-1, 1] as const) {
        expect(l.boundary(pos, bias), `fallback ${JSON.stringify(text)} ${pos}/${bias}`).toBe(oldBoundary(text, pos, bias));
      }
      if (text === 'plain ASCII') expect(l.stats.segmentations).toBe(0);
    }
  });
  it('caps cached clusters and charges eight bytes per pair', () => {
    const ascii = layout('x'.repeat(8)); ascii.shape(0);
    const emoji = layout('😀'.repeat(4)); emoji.shape(0);
    expect(emoji.cacheBytes - ascii.cacheBytes).toBe(32);
    const many = layout('😀'.repeat(4097)); many.shape(0);
    expect((many as unknown as { cache: Map<number, { clusters: number[] | null }> }).cache.get(0)?.clusters).toBeNull();
    const segmentations = many.stats.segmentations;
    expect(many.boundary(1, -1)).toBe(0);
    expect(many.stats.segmentations).toBe(segmentations + 1);
  });
  it('uses whole-run shaping and measured Japanese/ligature caret advances', () => {
    const l = layout('ffi日本'); const line = l.shape(0);
    expect(line.runs.map(r => r.text)).toEqual(['ffi日本']); expect(line.width).toBe(50);
    expect(l.coordsAtPos(3, view)?.left).toBe(76); expect(l.coordsAtPos(4, view)?.left).toBe(92);
    expect(l.posAtCoords({ x: 94, y: 21 }, view)).toBe(4);
  });
  it('reuses the additive advance table across ASCII lines and caret queries', () => {
    const measureText = vi.fn((text: string) => ({ width: text.length * 8 }));
    const l = new TextLayout({ font: '', measureText }, font);
    l.setText(Text.of(Array.from({ length: 51 }, (_, index) => `let value${index} = alpha`)));
    l.shape(0);
    const initialCalls = l.stats.measuredTextCalls;
    for (let number = 1; number < 51; number += 1) l.shape(number);
    l.coordsAtPos(l.document.indexOf('alpha'), view);
    l.posAtCoords({ x: 100, y: 21 }, view);
    expect(l.stats.measuredTextCalls).toBe(initialCalls);
  });
  it('measures each distinct additive-font Japanese cluster once', () => {
    const measureText = vi.fn((text: string) => ({ width: text.length * 8 }));
    const l = new TextLayout({ font: '', measureText }, font);
    l.setText(Text.of(['日本', '日語']));
    l.shape(0);
    expect(measureText.mock.calls.filter(([text]) => text === '日')).toHaveLength(1);
    l.shape(1);
    expect(measureText.mock.calls.filter(([text]) => text === '日')).toHaveLength(1);
    expect(measureText.mock.calls.filter(([text]) => text === '語')).toHaveLength(1);
  });
  it('maps viewport origin/scroll and clips source selection at gutter', () => {
    const l = layout('abcdef\nsecond'); const v = { ...view, scrollLeft: 16, scrollTop: 20 };
    expect(l.coordsAtPos(9, v)).toEqual({ left: 58, right: 59, top: 20, bottom: 40 });
    expect(l.posAtCoords({ x: 58, y: 25 }, v)).toBe(9);
    const rs = l.rangeRects({ from: 0, to: 10 }, v); expect(rs).toHaveLength(1); expect(rs[0]!.left).toBe(58);
  });
  it('bounds LRU, reuses idle layouts and invalidates font generation', () => {
    const l = layout('a\nb\nc', 250); l.shape(0); l.shape(0); expect(l.stats.builds).toBe(1);
    l.shape(1); l.shape(2); expect(l.cacheBytes).toBeLessThanOrEqual(250); expect(l.stats.evictions).toBeGreaterThan(0);
    l.setFont({ ...font, generation: 1 }); expect(l.cacheBytes).toBe(0); l.shape(2); expect(l.stats.builds).toBe(4);
  });
  it('reports RTL without claiming incorrect visual-order hit testing', () => {
    const l = layout('abc אבג'); expect(l.shape(0).rtlUnsupported).toBe(true); expect(l.posAtCoords({ x: 90, y: 21 }, view)).toBeNull();
  });
  it('tiles long text without per-character geometry, and keeps CR/LF endings', () => {
    const l = layout('x'.repeat(20_000) + '\r\n\n'); expect(l.shape(0).width).toBe(160_000); expect(l.cacheBytes).toBeLessThan(50_000);
    expect(l.lineCount).toBe(3); expect(l.coordsAtPos(20_003, view)?.top).toBe(60);
  });
});

describe('GPU shaped-run atlas', () => {
  it('reuses one ledger-accounted raster canvas until disposal', () => {
    const r = recordingGL(); const raster = rasterizer(); const createCanvas = vi.fn(raster.createCanvas);
    const b = new ResourceLedger(); const a = new GlyphAtlas(r.gl, b, createCanvas);
    const run = { text: 'abc', from: 0, to: 3, x: 0, width: 24 };
    a.tile({ run, font, dpr: 1, x: 0, width: 24 });
    a.tile({ run: { ...run, from: 3, to: 6 }, font, dpr: 1, x: 0, width: 24 });
    expect(createCanvas).toHaveBeenCalledTimes(1);
    expect(r.calls.filter(call => call.name === 'texImage2D')).toHaveLength(2);
    const geometryWithStaging = b.counters.byKind.geometry;
    expect(geometryWithStaging).toBeGreaterThanOrEqual(24 * 20 * 4);
    a.endFrame(); expect(b.counters.byKind.geometry).toBeLessThan(geometryWithStaging);
    a.beginFrame(); a.tile({ run: { ...run, from: 6, to: 9 }, font, dpr: 1, x: 0, width: 24 });
    expect(createCanvas).toHaveBeenCalledTimes(1);
    a.dispose(); expect(b.usedBytes).toBe(0);
  });
  it('crops the whole run and masks syntax without reshaping substrings', () => {
    const r = recordingGL(64); const raster = rasterizer(); const b = new ResourceLedger(); const a = new GlyphAtlas(r.gl, b, raster.createCanvas);
    const run = { text: 'ffi日本', from: 0, to: 5, x: 0, width: 50 };
    const tile = a.tile({ run, font, dpr: 1, x: 32, width: 18, styles: [{ from: 3, to: 5, color: '#ff0000' }] })!;
    expect(tile.width).toBe(18); expect(raster.text.map(t => t.text)).toEqual(['ffi日本', 'ffi日本']); expect(raster.text[0]!.x).toBe(-32);
    expect(raster.crops).toContainEqual([-14, 0, 32, 20]);
    a.dispose(); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0);
  });
  it('rasterizes a styled multi-tile run once and releases its bounded raster', () => {
    const r = recordingGL(64); const raster = rasterizer(); const b = new ResourceLedger(); const a = new GlyphAtlas(r.gl, b, raster.createCanvas);
    const run = { text: 'a'.repeat(32), from: 0, to: 32, x: 0, width: 256 };
    const styles = [{ from: 8, to: 12, color: '#ff0000' }];
    for (const x of [0, 64, 128, 192]) expect(a.tile({ run, font, dpr: 1, x, width: 64, styles })).not.toBeNull();
    expect(raster.text.map(t => t.text)).toEqual([run.text, run.text]);
    expect(r.calls.filter(call => call.name === 'texImage2D')).toHaveLength(4);
    const cachedRasterBytes = b.counters.byKind.geometry;
    expect(cachedRasterBytes).toBeGreaterThan(0);
    a.endFrame(); expect(b.counters.byKind.geometry).toBeLessThan(cachedRasterBytes);
    a.dispose(); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0);
  });
  it('caches by text/font/fallback/DPR/style and deletes evicted tiles', () => {
    const r = recordingGL(); const b = new ResourceLedger(17000); const a = new GlyphAtlas(r.gl, b, rasterizer().createCanvas);
    const run = { text: 'abc', from: 0, to: 3, x: 0, width: 24 };
    const req = { run, font, dpr: 1, x: 0, width: 24 };
    const first = a.tile(req)!; expect(a.tile(req)!.texture).toBe(first.texture); expect(a.stats.uploads).toBe(1);
    a.tile({ ...req, font: { ...font, fallback: 'other' } });
    a.tile({ ...req, color: '#ff0000' }); a.tile({ ...req, dpr: 2 });
    expect(a.stats.evictions).toBeGreaterThan(0); expect(b.usedBytes).toBeLessThanOrEqual(17000);
    a.dispose(); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0);
  });
  it('reuses syntax tiles when an unchanged run shifts in document offsets', () => {
    const r = recordingGL(); const b = new ResourceLedger(); const a = new GlyphAtlas(r.gl, b, rasterizer().createCanvas);
    const run = { text: 'abc', from: 10, to: 13, x: 0, width: 24 };
    const first = a.tile({ run, font, dpr: 1, x: 0, width: 24, styles: [{ from: 11, to: 12, color: '#ff0000' }] })!;
    run.from += 10; run.to += 10;
    const shifted = a.tile({ run, font, dpr: 1, x: 0, width: 24, styles: [{ from: 21, to: 22, color: '#ff0000' }] })!;
    expect(shifted.texture).toBe(first.texture);
    expect(a.stats).toMatchObject({ uploads: 1, hits: 1 });
    a.dispose(); expect(b.usedBytes).toBe(0);
  });
  it('rolls back reservations and textures on upload failure', () => {
    const r = recordingGL(); const b = new ResourceLedger(); const a = new GlyphAtlas(r.gl, b, rasterizer().createCanvas); r.failNextUpload();
    expect(() => a.tile({ run: { text: 'a', from: 0, to: 1, x: 0, width: 8 }, font, dpr: 1, x: 0, width: 8 })).toThrow('refused');
    expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0); expect(a.size).toBe(0);
  });
  it('bounds atlas misses per frame while allowing resident hits', () => {
    const r = recordingGL(); const b = new ResourceLedger(); const a = new GlyphAtlas(r.gl, b, rasterizer().createCanvas);
    const req = { run: { text: 'a', from: 0, to: 1, x: 0, width: 8 }, font, dpr: 1, x: 0, width: 8 };
    a.beginFrame(640); expect(a.tile(req)).not.toBeNull();
    a.beginFrame(0); expect(a.tile(req)).not.toBeNull();
    expect(a.tile({ ...req, x: 8 })).toBeNull(); expect(a.stats.uploads).toBe(1);
    a.dispose(); expect(b.usedBytes).toBe(0);
  });
});

describe('GPU code compositor', () => {
  it('keeps Text-backed and string-backed renderer commands identical', () => {
    const first = fixture(), second = fixture();
    const text = 'let x = 1\n日本';
    first.renderer.setDocument(text); second.renderer.setText(Text.of(['let x = 1', '日本']));
    const feedback = { annotations: [{ kind: 'syntax' as const, from: 0, to: 3, className: 'vact-tok-head' }], cursor: 4, textRevision: 1 };
    expect(first.renderer.render(feedback)).toBe(true); expect(second.renderer.render(feedback)).toBe(true);
    expect(second.r.draws.map(({ rect, color }) => ({ rect, color }))).toEqual(first.r.draws.map(({ rect, color }) => ({ rect, color })));
    first.renderer.dispose(); second.renderer.dispose();
  });
  function fixture(text = 'let x = 1\n日本') {
    const r = recordingGL(); const raster = rasterizer(); const b = new ResourceLedger(); const c = document.createElement('canvas'); const l = layout(text);
    const renderer = new CanvasRenderer(c, l, { gl: r.gl, ledger: b, createCanvas: raster.createCanvas }); renderer.setViewport(view, 1);
    return { r, raster, b, c, l, renderer };
  }
  it('draws all required feedback in order on GPU and clips source against gutter', () => {
    const f = fixture();
    const annotations: CodeAnnotation[] = [
      { kind: 'selection', from: 0, to: 3 }, { kind: 'playing', from: 4, to: 5 }, { kind: 'eval', from: 0, to: 9 },
      { kind: 'syntax', from: 0, to: 3, className: 'vact-tok-head' }, { kind: 'diagnostic', from: 4, to: 5 },
      { kind: 'composition', from: 6, to: 9 }, { kind: 'binding', from: 4, to: 5, label: 'CC 1' },
    ];
    expect(f.renderer.render({ annotations, cursor: 5, handles: [{ pos: 0 }, { pos: 9, end: true }] })).toBe(true);
    const colors = f.r.draws.map(d => d.color);
    expect(colors.slice(0, 4)).toEqual([[0.04, 0.05, 0.07, 0.85], [0.12, 0.22, 0.32, 0.55], [0.35, 0.29, 0.13, 0.5], [0.2, 0.32, 0.36, 0.5]]);
    const diagnostic = colors.findIndex(c => c[0] === 0.75); const cursor = colors.findIndex(c => c[0] === 0.9); const badge = colors.findIndex(c => c[0] === 0.15);
    expect(diagnostic).toBeGreaterThan(3); expect(cursor).toBeGreaterThan(diagnostic); expect(badge).toBeGreaterThan(cursor);
    expect(colors.slice(-2)).toEqual([[0.53, 0.75, 0.82, 1], [0.53, 0.75, 0.82, 1]]);
    expect(f.r.calls.some(c => c.name === 'scissor' && c.args[0] === 48)).toBe(true);
    expect(f.raster.text.some(t => t.text === 'let x = 1' && t.color === '#ebcb8b')).toBe(true); expect(f.raster.text.some(t => t.text === '1')).toBe(true);
    f.renderer.dispose(); expect(f.b.usedBytes).toBe(0); expect(f.r.live.size).toBe(0);
  });
  it('idle frames reuse text uploads, layout and static geometry; edits rebuild only dirty tiles', () => {
    const f = fixture(); expect(f.renderer.render()).toBe(true); const uploads = f.renderer.atlasStats.uploads; const builds = f.l.stats.builds;
    for (let i = 0; i < 4; i++) expect(f.renderer.render({ cursor: i })).toBe(true);
    expect(f.renderer.atlasStats.uploads).toBe(uploads); expect(f.l.stats.builds).toBe(builds); expect(f.renderer.stats.bufferUploads).toBe(1);
    f.renderer.setDocument('let x = 2\n日本'); f.renderer.render(); expect(f.renderer.atlasStats.uploads).toBe(uploads + 1);
    f.renderer.dispose();
  });
  it('animation-only frames replay cached text without shaping or uploads', () => {
    const f = fixture(); f.renderer.render({ textRevision: 1 });
    const builds = f.renderer.stats.textBuilds, uploads = f.renderer.atlasStats.uploads, buffers = f.renderer.stats.bufferUploads;
    const shape = vi.spyOn(f.l, 'shape');
    expect(f.renderer.render({ textRevision: 1, animated: [{ kind: 'playing', from: 0, to: 3 }] })).toBe(true);
    expect(shape).not.toHaveBeenCalled(); expect(f.renderer.stats.textBuilds).toBe(builds);
    expect(f.renderer.atlasStats.uploads).toBe(uploads); expect(f.renderer.stats.bufferUploads).toBe(buffers);
    expect(f.renderer.render({ textRevision: 1, animated: [{ kind: 'playing', from: 4, to: 8 }] })).toBe(true);
    expect(shape).not.toHaveBeenCalled(); expect(f.renderer.atlasStats.uploads).toBe(uploads); f.renderer.dispose();
  });
  it('does no liveness, error or segmentation probes on animation-only frames', () => {
    const f = fixture(); const isTexture = vi.spyOn(f.r.gl, 'isTexture'), getError = vi.spyOn(f.r.gl, 'getError');
    f.renderer.setViewport({ ...view, width: view.width + 1 });
    expect(getError).toHaveBeenCalled();
    expect(f.renderer.render({ textRevision: 1 })).toBe(true);
    expect(f.l.stats.segmentations).toBeGreaterThan(0);
    const segmentations = f.l.stats.segmentations, builds = f.renderer.stats.textBuilds, uploads = f.renderer.atlasStats.uploads;
    const draws = f.r.draws.length, texture = [...f.r.live.keys()].find((key) => f.r.live.get(key) === 'Texture')!;
    expect(f.r.gl.isTexture(texture)).toBe(true); expect(isTexture).toHaveBeenCalledTimes(1); isTexture.mockClear(); getError.mockClear();
    for (let i = 0; i < 10; i++) expect(f.renderer.render({ textRevision: 1, animated: [{ kind: 'playing', from: 0, to: 3 }, { kind: 'playing', from: 11, to: 13 }] })).toBe(true);
    expect(isTexture).not.toHaveBeenCalled(); expect(getError).not.toHaveBeenCalled();
    expect(f.l.stats.segmentations).toBe(segmentations); expect(f.renderer.stats.textBuilds).toBe(builds);
    expect(f.renderer.atlasStats.uploads).toBe(uploads); expect(f.r.draws.length).toBeGreaterThan(draws);
    f.renderer.dispose();
  });
  it('does not draw commands that reference textures deleted by context loss', () => {
    const f = fixture(); expect(f.renderer.render({ textRevision: 1 })).toBe(true); f.r.lose();
    f.c.dispatchEvent(new Event('webglcontextlost', { cancelable: true }));
    f.c.dispatchEvent(new Event('webglcontextrestored'));
    const start = f.r.draws.length;
    expect(f.renderer.render({ textRevision: 1, animated: [{ kind: 'playing', from: 0, to: 3 }] })).toBe(true);
    expect(f.r.draws.slice(start).every(draw => draw.textureLiveAtDraw)).toBe(true);
    f.renderer.dispose();
  });
  it('uses annotation revisions to reuse and invalidate static draw commands', () => {
    const f = fixture(); const first = [{ kind: 'syntax' as const, from: 0, to: 3, className: 'vact-tok-head' }];
    expect(f.renderer.render({ annotations: first, textRevision: 1, annotationsRevision: 1 })).toBe(true);
    const builds = f.renderer.stats.textBuilds;
    expect(f.renderer.render({ annotations: first, textRevision: 1, annotationsRevision: 1 })).toBe(true);
    expect(f.renderer.stats.textBuilds).toBe(builds);
    expect(f.renderer.render({ annotations: [{ ...first[0]!, className: 'vact-tok-number' }], textRevision: 1, annotationsRevision: 2 })).toBe(true);
    expect(f.renderer.stats.textBuilds).toBe(builds + 1); f.renderer.dispose();
  });
  it('replays gutter numbers unscissored and source text clipped on cached frames', () => {
    const f = fixture();
    const frameDraws = (start: number) => f.r.draws.slice(start).filter(draw => draw.color.every(channel => channel === 1) && draw.rect[3] === font.lineHeight);
    const gutterDraws = (draws: ReturnType<typeof frameDraws>) => draws.filter(draw => draw.rect[0] + draw.rect[2] <= 48);
    const sourceDraws = (draws: ReturnType<typeof frameDraws>) => draws.filter(draw => draw.rect[0] >= 48);
    const firstStart = f.r.draws.length;
    expect(f.renderer.render({ textRevision: 1 })).toBe(true);
    const rebuildDraws = frameDraws(firstStart);
    const builds = f.renderer.stats.textBuilds;
    const cachedStart = f.r.draws.length;
    expect(f.renderer.render({ textRevision: 1, animated: [{ kind: 'playing', from: 0, to: 3 }] })).toBe(true);
    expect(f.renderer.stats.textBuilds).toBe(builds);
    const cachedDraws = frameDraws(cachedStart);
    const rebuildGutter = gutterDraws(rebuildDraws), cachedGutter = gutterDraws(cachedDraws);
    const rebuildSource = sourceDraws(rebuildDraws), cachedSource = sourceDraws(cachedDraws);
    expect(rebuildGutter.length).toBeGreaterThan(0); expect(cachedGutter.length).toBe(rebuildGutter.length);
    expect(rebuildSource.length).toBeGreaterThan(0); expect(cachedSource.length).toBeGreaterThan(0);
    expect(rebuildGutter.every(draw => !draw.scissor)).toBe(true); expect(cachedGutter.every(draw => !draw.scissor)).toBe(true);
    expect(rebuildSource.every(draw => draw.scissor)).toBe(true); expect(cachedSource.every(draw => draw.scissor)).toBe(true);
    f.renderer.dispose();
  });
  it('rebuilds static draw lists on revision and draws call-head underline without a label', () => {
    const f = fixture('call()'); f.renderer.render({ textRevision: 1 }); const builds = f.renderer.stats.textBuilds;
    expect(f.renderer.render({ textRevision: 2, annotations: [{ kind: 'call-head', from: 0, to: 4, label: 'must-not-render' }] })).toBe(true);
    expect(f.renderer.stats.textBuilds).toBe(builds + 1);
    expect(f.r.draws.some(draw => draw.color[0] === 0.55 && draw.color[1] === 0.6)).toBe(true);
    expect(f.raster.text.some(entry => entry.text === 'must-not-render')).toBe(false); f.renderer.dispose();
  });
  it('keeps textPending until a bounded per-frame atlas upload completes', () => {
    const r = recordingGL(1024); const raster = rasterizer(); const b = new ResourceLedger();
    const text = Array.from({ length: 35 }, () => 'x'.repeat(800)).join('\n');
    const renderer = new CanvasRenderer(document.createElement('canvas'), layout(text), { gl: r.gl, ledger: b, createCanvas: raster.createCanvas });
    renderer.setViewport({ width: 640, height: 1000, scrollLeft: 0, scrollTop: 0, gutter: 0 });
    expect(renderer.render({ textRevision: 1 })).toBe(true); expect(renderer.textPending).toBe(true);
    expect(renderer.atlasStats.uploads).toBeLessThan(35);
    for (let i = 0; i < 3 && renderer.textPending; i++) expect(renderer.render({ textRevision: 1 })).toBe(true);
    expect(renderer.textPending).toBe(false); expect(renderer.atlasStats.uploads).toBe(35);
    renderer.dispose(); expect(b.usedBytes).toBe(0);
  });
  it('animation frames never copy or upload a backdrop', () => {
    const f = fixture(); expect(f.renderer.render({ textRevision: 1 })).toBe(true);
    const textureUploads = f.r.calls.filter(c => c.name === 'texImage2D' || c.name === 'texSubImage2D').length;
    const canvases = f.raster.canvases; const drawImages = f.raster.drawImages;
    const textures = [...f.r.live.values()].filter(v => v === 'Texture').length;
    for (let frame = 0; frame < 100; frame++) expect(f.renderer.render({ textRevision: 1, animated: [{ kind: 'playing', from: 0, to: 3 }] })).toBe(true);
    expect(f.r.calls.filter(c => c.name === 'texImage2D' || c.name === 'texSubImage2D')).toHaveLength(textureUploads);
    expect(f.raster.canvases).toBe(canvases); expect(f.raster.drawImages).toBe(drawImages);
    expect([...f.r.live.values()].filter(v => v === 'Texture')).toHaveLength(textures);
    expect(f.b.counters.byKind.backdrop).toBe(0);
    f.renderer.dispose(); expect(f.b.usedBytes).toBe(0);
  });
  it('restores CPU source after context loss with fresh resources and empty targets', () => {
    const f = fixture(); f.renderer.render({ textRevision: 1 }); const builds = f.renderer.stats.textBuilds; f.r.lose();
    const lost = new Event('webglcontextlost', { cancelable: true }); f.c.dispatchEvent(lost);
    expect(lost.defaultPrevented).toBe(true); expect(f.b.usedBytes).toBe(0); expect(f.renderer.render()).toBe(false); expect(f.renderer.status.saveText()).toBe('let x = 1\n日本');
    f.renderer.setDocument('retained edit'); f.c.dispatchEvent(new Event('webglcontextrestored')); expect(f.renderer.render({ textRevision: 1 })).toBe(true); expect(f.renderer.stats.textBuilds).toBe(builds + 1); expect(f.renderer.status.saveText()).toBe('retained edit');
    f.renderer.dispose(); expect(f.b.usedBytes).toBe(0); expect(f.r.live.size).toBe(0);
    f.c.dispatchEvent(new Event('webglcontextrestored')); expect(f.b.usedBytes).toBe(0);
  });
  it('retains text and save access after allocation/upload failure without DOM fallback', () => {
    const f = fixture(); f.r.failNextUpload(); expect(f.renderer.render()).toBe(false);
    expect(f.renderer.status.kind).toBe('unavailable'); expect(f.renderer.status.saveText()).toBe('let x = 1\n日本'); expect(f.b.usedBytes).toBe(0); expect(f.r.live.size).toBe(0);
    f.renderer.setDocument('save me'); expect(f.renderer.status.saveText()).toBe('save me'); f.renderer.dispose();
  });
  it('reduces effective DPR, invalidates atlas and releases resized targets', () => {
    const f = fixture(); f.renderer.render(); f.renderer.setViewport({ ...view, width: 1600, height: 1000 }, 3);
    expect(f.renderer.status.kind).toBe('degraded'); expect(f.c.width).toBeLessThanOrEqual(1024); expect(f.b.counters.pixels).toBeLessThanOrEqual(4_000_000);
    expect(f.renderer.render()).toBe(true); f.renderer.setViewport(view, 1); expect(f.renderer.render()).toBe(true);
    expect(f.b.counters.pixels).toBe(view.width * view.height); f.renderer.dispose(); expect(f.b.usedBytes).toBe(0);
  });
  it('updates DPR when CSS viewport changes but backing dimensions stay equal', () => {
    const f = fixture(); f.renderer.setViewport({ ...view, width: 100, height: 100 }, 2); f.renderer.render();
    expect(f.c.width).toBe(200);
    f.renderer.setViewport({ ...view, width: 200, height: 200 }, 1); expect(f.c.width).toBe(200);
    expect(f.renderer.status.effectiveDpr).toBe(1); expect(f.renderer.render()).toBe(true);
    expect(f.r.calls.filter(c => c.name === 'scissor').at(-1)?.args[0]).toBe(48); f.renderer.dispose();
  });
  it('large viewports stay within geometry and aggregate pixel caps without a backdrop reservation', () => {
    const f = fixture(); expect(f.renderer.render()).toBe(true);
    expect(f.b.counters.byKind.geometry).toBeLessThanOrEqual(RESOURCE_LIMITS.geometry); expect(f.b.counters.pixels).toBeLessThanOrEqual(4_000_000);
    const uploads = f.r.calls.filter(c => c.name === 'texImage2D');
    const copied = uploads.find(c => c.args.length === 6 && (c.args[5] as HTMLCanvasElement).width > 0);
    // A canvas-backed texture source here would indicate a backdrop copy; atlas uploads use typed arrays.
    expect(copied).toBeUndefined(); expect(f.b.counters.byKind.backdrop).toBe(0); f.renderer.dispose(); expect(f.b.usedBytes).toBe(0);
  });
  it('keeps text metadata within geometry caps without backdrop staging', () => {
    const r = recordingGL(4096); const raster = rasterizer(); const b = new ResourceLedger(); const c = document.createElement('canvas');
    const text = Array.from({ length: 40 }, (_, i) => `line ${i}`).join('\n');
    const renderer = new CanvasRenderer(c, layout(text), { gl: r.gl, ledger: b, createCanvas: raster.createCanvas });
    renderer.setViewport({ width: 640, height: 800, scrollLeft: 0, scrollTop: 0 });
    expect(renderer.render({ textRevision: 1 })).toBe(true);
    expect(renderer.render({ textRevision: 1, animated: [{ kind: 'playing', from: 0, to: 3 }] })).toBe(true);
    expect(b.counters.byKind.geometry).toBeLessThanOrEqual(RESOURCE_LIMITS.geometry);
    expect(b.counters.byKind.backdrop).toBe(0); renderer.dispose(); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0);
  });
  it('retains every unchanged text tile across animation frames without backdrop uploads', () => {
    const r = recordingGL(4096); const raster = rasterizer(); const b = new ResourceLedger();
    const text = Array.from({ length: 40 }, (_, i) => `line ${i}`).join('\n');
    const renderer = new CanvasRenderer(document.createElement('canvas'), layout(text), { gl: r.gl, ledger: b, createCanvas: raster.createCanvas });
    renderer.setViewport({ width: 640, height: 800, scrollLeft: 0, scrollTop: 0 });
    const allocate = b.allocate.bind(b);
    b.allocate = (...args) => {
      const allocation = allocate(...args);
      expect(b.usedBytes).toBeLessThanOrEqual(RESOURCE_LIMITS.total);
      expect(b.counters.byKind.geometry).toBeLessThanOrEqual(RESOURCE_LIMITS.geometry);
      expect(b.counters.byKind.atlas).toBeLessThanOrEqual(RESOURCE_LIMITS.atlas);
      expect(b.counters.pixels).toBeLessThanOrEqual(RESOURCE_LIMITS.canvasPixels);
      return allocation;
    };
    let firstUploads = 0; let firstTextures: unknown[] = []; let firstBytes = 0;
    try {
      expect(renderer.render({ textRevision: 1 })).toBe(true);
      for (let revision = 1; revision <= 4; revision++) {
        const start = r.draws.length;
        expect(renderer.render({ textRevision: 1, animated: [{ kind: 'playing', from: revision, to: revision + 1 }] })).toBe(true);
        const textures = r.draws.slice(start).map(draw => draw.texture);
        if (revision === 1) {
          firstUploads = renderer.atlasStats.uploads; firstTextures = textures; firstBytes = b.usedBytes;
          expect(firstUploads).toBe(80); // 40 visible source runs and 40 line numbers.
          expect(raster.text.filter(entry => entry.text.startsWith('line '))).toHaveLength(40);
        } else {
          expect(renderer.atlasStats.uploads).toBe(firstUploads);
          expect(textures).toEqual(firstTextures); // No skipped or replaced visible tiles.
          expect(b.usedBytes).toBe(firstBytes);
        }
        expect(renderer.atlasStats.evictions).toBe(0);
        expect(b.counters.byKind.backdrop).toBe(0);
      }
    expect(r.calls.filter(call => call.name === 'texSubImage2D')).toHaveLength(0);
      expect(r.calls.filter(call => call.name === 'texImage2D')).toHaveLength(81); // White and 80 glyph tiles; no backdrop texture.
    } finally { renderer.dispose(); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0); }
  });
  it('geometry pressure during animation does not evict text or allocate backdrop storage', () => {
    const f = fixture(); expect(f.renderer.render({ textRevision: 1 })).toBe(true);
    const uploads = f.renderer.atlasStats.uploads;
    const pressure = f.b.allocate('geometry', RESOURCE_LIMITS.geometry - f.b.counters.byKind.geometry - 100_000)!;
    expect(pressure).not.toBeNull();
    try {
      expect(f.renderer.render({ textRevision: 1, animated: [{ kind: 'playing', from: 0, to: 3 }] })).toBe(true);
      expect(f.b.counters.byKind.backdrop).toBe(0);
      expect(f.renderer.atlasStats.uploads).toBe(uploads); expect(f.renderer.atlasStats.evictions).toBe(0);
    } finally { pressure.release(); f.renderer.dispose(); expect(f.b.usedBytes).toBe(0); expect(f.r.live.size).toBe(0); }
  });
  it('keeps texture-limit queries off viewport and animation frames and refreshes on restore', () => {
    const f = fixture();
    const initReads = f.r.calls.filter(call => call.name === 'getParameter').length;
    expect(initReads).toBeGreaterThanOrEqual(2); // The renderer and atlas initialization reads are allowed.
    f.r.calls.length = 0;
    for (let frame = 0; frame < 100; frame++) {
      f.renderer.setViewport({ ...view, width: view.width + frame % 2 });
      expect(f.renderer.render({ textRevision: 1, animated: [{ kind: 'playing', from: 0, to: 3 }] })).toBe(true);
    }
    expect(f.r.calls.filter(call => call.name === 'getParameter')).toHaveLength(0);
    f.c.dispatchEvent(new Event('webglcontextlost', { cancelable: true }));
    f.c.dispatchEvent(new Event('webglcontextrestored'));
    expect(f.r.calls.filter(call => call.name === 'getParameter' && call.args[0] === f.r.gl.MAX_TEXTURE_SIZE).length).toBeGreaterThanOrEqual(1);
    f.renderer.dispose(); expect(f.b.usedBytes).toBe(0);
  });
  it('maps the diagnostic underline and fallback palette to the UI tokens and legacy colors', () => {
    const f = fixture();
    f.renderer.setPalette({ ...FALLBACK_PALETTE, diagnostic: [1, 0, 0, 1] });
    expect(f.renderer.render({ annotations: [{ kind: 'diagnostic', from: 0, to: 3 }] })).toBe(true);
    expect(f.r.draws.some(draw => draw.color[0] === 1 && draw.color[1] === 0 && draw.color[2] === 0)).toBe(true);
    const expected = {
      text: '#d8dee9', gutter: '#7a7f87', diagnostic: '#bf3340',
      composition: '#87bfd1', callHead: 'rgba(140, 153, 168, 0.6)', cursor: '#e6ebf0',
      labelFill: 'rgba(38, 48, 64, 0.95)', labelText: '#ebcb8b', handle: '#87bfd1',
    };
    expect(Object.fromEntries(Object.entries(expected).map(([name]) => [name, rgbaCss(FALLBACK_PALETTE[name as keyof typeof expected])]))).toEqual(expected);
    expect(Object.fromEntries(Object.entries(FALLBACK_PALETTE.token).map(([name, color]) => [name, rgbaCss(color)]))).toEqual(GPU_TOKEN_COLORS);
    const root = document.createElement('div'); root.style.setProperty('--vt-danger', '#ff0000'); document.body.append(root);
    expect(readPalette(root).diagnostic).toEqual([1, 0, 0, 1]);
    root.style.setProperty('--vt-danger', 'invalid');
    expect(readPalette(root).diagnostic).toEqual(FALLBACK_PALETTE.diagnostic);
    root.remove(); f.renderer.dispose();
  });
  it('dense offscreen syntax on a 1MiB line cannot overflow visible-tile metadata', () => {
    const f = fixture('x'.repeat(1024 * 1024));
    const annotations: CodeAnnotation[] = Array.from({ length: 150_000 }, (_, i) => ({ kind: 'syntax', from: i * 6, to: i * 6 + 3, className: 'vact-tok-head' }));
    expect(f.renderer.render({ annotations })).toBe(true);
    expect(f.b.counters.byKind.geometry).toBeLessThan(10_000);
    expect(f.raster.crops.length).toBeLessThan(32); f.renderer.dispose(); expect(f.b.usedBytes).toBe(0);
  });
  it('atlas does not retain historical full source strings outside the layout cache', () => {
    const f = fixture('x'.repeat(100_000));
    for (let i = 0; i < 12; i++) { f.renderer.setDocument('x'.repeat(99_999) + String(i)); expect(f.renderer.render()).toBe(true); }
    expect(f.l.cacheBytes).toBeLessThan(201_000); expect(f.b.counters.byKind.geometry).toBeLessThanOrEqual(RESOURCE_LIMITS.geometry);
    f.renderer.dispose(); expect(f.b.usedBytes).toBe(0);
  });
  it('keeps rendering all visible lines when atlas working set exceeds available budget', () => {
    const r = recordingGL(); const b = new ResourceLedger(2_100_000); const c = document.createElement('canvas'); const raster = rasterizer();
    const text = Array.from({ length: 50 }, (_, i) => `${i}:` + 'x'.repeat(58)).join('\n');
    const renderer = new CanvasRenderer(c, layout(text), { gl: r.gl, ledger: b, createCanvas: raster.createCanvas }); renderer.setViewport({ width: 500, height: 1000, scrollLeft: 0, scrollTop: 0, gutter: 0 });
    const renderedPrefixes = new Set<number>();
    const assertNewlyRasterizedLinesWereDrawn = (rasterStart: number, drawStart: number): void => {
      const rasterized = raster.text.slice(rasterStart).map(entry => /^(\d+):/.exec(entry.text)?.[1]).filter((line): line is string => line !== undefined).map(Number);
      const frameDraws = r.draws.slice(drawStart);
      expect(frameDraws.every(draw => draw.textureLiveAtDraw)).toBe(true);
      const drawnLines = new Set(frameDraws.filter(draw => draw.rect[3] === 20 && draw.color.every(channel => channel === 1)).map(draw => draw.rect[1] / 20));
      for (const line of rasterized) {
        renderedPrefixes.add(line);
        expect(drawnLines.has(line), `rasterized line ${line} has no same-frame text draw; drawn lines: ${JSON.stringify([...drawnLines])}`).toBe(true);
      }
    };
    let rasterStart = raster.text.length; let drawStart = r.draws.length;
    expect(renderer.render()).toBe(true); expect(renderer.atlasStats.evictions).toBeGreaterThan(0);
    assertNewlyRasterizedLinesWereDrawn(rasterStart, drawStart);
    for (let i = 0; i < 10 && renderer.textPending; i++) {
      rasterStart = raster.text.length; drawStart = r.draws.length;
      expect(renderer.render()).toBe(true); assertNewlyRasterizedLinesWereDrawn(rasterStart, drawStart);
    }
    expect(renderer.textPending).toBe(true);
    expect([...renderedPrefixes].sort((a, b) => a - b)).toEqual(Array.from({ length: 50 }, (_, i) => i));
    expect(b.usedBytes).toBeLessThanOrEqual(2_100_000); renderer.dispose(); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0);
  });
  it('releases initialization resources when shader or object setup fails', () => {
    const r = recordingGL(); Object.assign(r.gl, { getShaderParameter: () => false }); const b = new ResourceLedger();
    const renderer = new CanvasRenderer(document.createElement('canvas'), layout('safe'), { gl: r.gl, ledger: b });
    expect(renderer.status.kind).toBe('unavailable'); expect(renderer.status.saveText()).toBe('safe'); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0); renderer.dispose();
  });
});
