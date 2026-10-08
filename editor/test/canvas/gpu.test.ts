import { ChangeSet, Text } from '@codemirror/state';
import { describe, expect, it, vi } from 'vitest';
import { TextLayout, type LayoutFont } from '../../src/code/layout';
import { GlyphAtlas, type AtlasCell, type CanvasFactory } from '../../src/code/atlas';
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
function rasterizer(): { createCanvas: CanvasFactory; text: { text: string; x: number; y: number; font: string; color: string }[]; crops: number[][]; drawImages: number; canvases: number; measures: number } {
  const text: { text: string; x: number; y: number; font: string; color: string }[] = []; const crops: number[][] = [];
  let drawImages = 0; let canvases = 0; let measures = 0;
  return { text, crops,
    createCanvas: () => {
      canvases++;
      const canvas = document.createElement('canvas');
      const ctx = { font: '', fillStyle: '', textBaseline: '', setTransform() {}, clearRect() {}, save() {}, restore() {}, beginPath() {}, clip() {}, drawImage() { drawImages++; },
        rect(...args: number[]) { crops.push(args); },
        getImageData(_x: number, _y: number, w: number, h: number) { return { data: new Uint8ClampedArray(w * h * 4) }; },
        measureText(t: string) { measures++; return { width: width(t) }; },
        fillText(t: string, x: number, y: number) { text.push({ text: t, x, y, font: this.font, color: this.fillStyle }); },
      };
      Object.defineProperty(canvas, 'getContext', { value: () => ctx }); return canvas;
    },
    get drawImages() { return drawImages; }, get canvases() { return canvases; }, get measures() { return measures; },
  };
}
function atlasCell(a: GlyphAtlas, request: { run: { text: string; from: number; to: number; x: number; width: number }; font: LayoutFont; dpr: number; x: number; width: number; color?: string; styles?: readonly { from: number; to: number; color: string }[] }): { texture: WebGLTexture; width: number; height: number; x: number; cssWidth: number; cell: AtlasCell } | null {
  const cell = a.cell({ kind: 'run', text: request.run.text, run: request.run, piece: { from: request.x, to: request.x + request.width }, chunkX: request.x, font: request.font, dpr: request.dpr, width: request.width });
  return cell ? { texture: a.texture, width: cell.width, height: cell.height, x: request.x, cssWidth: request.width, cell } : null;
}
function recordingGL(maxTexture = 1024) {
  let next = 1; let error = 0; let failUpload = false; let failAllocation = false;
  const live = new Map<object, string>(); const deleted: string[] = []; const calls: { name: string; args: unknown[]; buffer?: object | null }[] = [];
  let rect: number[] = []; let color: number[] = []; let bound: object | null = null; let arrayBuffer: object | null = null; let slots = new Float32Array(2);
  const instanceBuffers = new Map<object, Uint8Array>();
  const draws: { rect: number[]; color: number[]; texture: object | null; textureLiveAtDraw: boolean; scissor: boolean; layer: string; flags: number; uv: number[]; slot: number; absLeft: number; absTop: number }[] = [];
  const gl: Record<string, unknown> = {};
  ['MAX_TEXTURE_SIZE', 'TEXTURE_2D', 'TEXTURE_MIN_FILTER', 'TEXTURE_MAG_FILTER', 'LINEAR', 'NEAREST', 'TEXTURE_WRAP_S', 'TEXTURE_WRAP_T', 'CLAMP_TO_EDGE', 'UNPACK_PREMULTIPLY_ALPHA_WEBGL', 'UNPACK_SKIP_PIXELS', 'UNPACK_SKIP_ROWS', 'UNPACK_ROW_LENGTH', 'RGBA', 'RG', 'RG32F', 'FLOAT', 'UNSIGNED_SHORT', 'UNSIGNED_BYTE', 'VERTEX_SHADER', 'FRAGMENT_SHADER', 'COMPILE_STATUS', 'LINK_STATUS', 'ARRAY_BUFFER', 'STATIC_DRAW', 'DYNAMIC_DRAW', 'FRAMEBUFFER', 'BLEND', 'ONE', 'ONE_MINUS_SRC_ALPHA', 'COLOR_BUFFER_BIT', 'TEXTURE0', 'TEXTURE1', 'TRIANGLES'].forEach((name, i) => gl[name] = i + 1);
  gl.NO_ERROR = 0;
  for (const kind of ['Texture', 'Shader', 'Program', 'Buffer', 'VertexArray']) {
    gl[`create${kind}`] = () => { const object = { id: next++ }; live.set(object, kind); return object; };
    gl[`delete${kind}`] = (obj: object) => { expect(live.get(obj)).toBe(kind); live.delete(obj); deleted.push(kind); };
  }
  for (const name of ['texParameteri', 'pixelStorei', 'shaderSource', 'compileShader', 'attachShader', 'linkProgram', 'bindVertexArray', 'enableVertexAttribArray', 'vertexAttribPointer', 'vertexAttribIPointer', 'vertexAttribDivisor', 'bindFramebuffer', 'useProgram', 'viewport', 'disable', 'enable', 'blendFunc', 'clearColor', 'clear', 'uniform2f', 'uniform1f', 'uniform1i', 'activeTexture']) gl[name] = (...args: unknown[]) => {
    calls.push({ name, args });
  };
  gl.bufferData = (...args: unknown[]) => { calls.push({ name: 'bufferData', args, buffer: arrayBuffer }); if (failAllocation) { failAllocation = false; error = 1285; } };
  gl.bindBuffer = (_target: number, buffer: object | null) => { arrayBuffer = buffer; calls.push({ name: 'bindBuffer', args: [_target, buffer] }); };
  gl.getParameter = (...args: unknown[]) => { calls.push({ name: 'getParameter', args }); return maxTexture; }; gl.getShaderParameter = () => true; gl.getProgramParameter = () => true;
  gl.isTexture = (obj: object) => live.get(obj) === 'Texture';
  gl.getShaderInfoLog = () => ''; gl.getProgramInfoLog = () => ''; gl.getAttribLocation = () => 0; gl.getUniformLocation = (_p: object, name: string) => ({ name });
  gl.isContextLost = () => false;
  gl.bindTexture = (_target: number, texture: object | null) => { bound = texture; };
  gl.uniform4f = (loc: { name: string }, ...args: number[]) => { if (loc.name === 'u_rect') rect = args; if (loc.name === 'u_color') color = args; };
  gl.bufferSubData = (_target: number, offset: number, data: Uint8Array) => {
    const previous = arrayBuffer ? instanceBuffers.get(arrayBuffer) : undefined;
    const bytes = new Uint8Array(Math.max(previous?.byteLength ?? 0, offset + data.byteLength));
    if (previous) bytes.set(previous);
    bytes.set(data, offset);
    if (arrayBuffer) instanceBuffers.set(arrayBuffer, bytes);
    calls.push({ name: 'bufferSubData', args: [_target, offset, data], buffer: arrayBuffer });
  };
  gl.drawArraysInstanced = (_mode: number, _first: number, _vertices: number, count: number) => {
    const buffers = [...live.entries()].filter(([, kind]) => kind === 'Buffer').map(([obj]) => obj);
    const layer = ['background', 'text', 'overlay', 'overlayText'][buffers.indexOf(arrayBuffer!) - 1] ?? 'overlayText';
    calls.push({ name: 'drawArraysInstanced', args: [_mode, _first, _vertices, count, layer], buffer: arrayBuffer });
    const instances = arrayBuffer ? instanceBuffers.get(arrayBuffer) ?? new Uint8Array() : new Uint8Array();
    const data = new DataView(instances.buffer, instances.byteOffset, instances.byteLength);
    for (let i = 0; i < count; i++) {
      const offset = i * 32;
      const slot = data.getUint16(offset + 28, true);
      const instanceRect = [0, 4, 8, 12].map(n => data.getFloat32(offset + n, true));
      instanceRect[1] += slots[slot * 2 + 1] ?? 0;
      const absLeft = instanceRect[0] + (slots[slot * 2] ?? 0), absTop = instanceRect[1];
      const instanceColor = [24, 25, 26, 27].map(n => data.getUint8(offset + n) / 255);
      const uv = [16, 18, 20, 22].map(n => data.getUint16(offset + n, true) / 65535);
      draws.push({ rect: instanceRect, color: instanceColor, texture: bound, textureLiveAtDraw: bound !== null && live.get(bound) === 'Texture', scissor: false, layer,
        flags: data.getUint16(offset + 30, true), uv, slot, absLeft, absTop });
    }
  };
  for (const name of ['texImage2D', 'texSubImage2D']) gl[name] = (...args: unknown[]) => {
    calls.push({ name, args });
    if (name === 'texSubImage2D' && args[8] instanceof Float32Array) {
      const offset = Number(args[2]) * 2, data = args[8] as Float32Array;
      const next = new Float32Array(Math.max(slots.length, offset + data.length)); next.set(slots); next.set(data, offset); slots = next;
    }
    if (name === 'texImage2D' && failAllocation) { failAllocation = false; error = 1285; }
    if (name === 'texSubImage2D' && failUpload) { failUpload = false; throw new Error('refused upload'); }
  };
  gl.getError = () => { const result = error; error = 0; return result; };
  return { gl: gl as unknown as WebGL2RenderingContext, live, deleted, calls, draws, failNextUpload: () => { failUpload = true; }, failNextAllocation: () => { failAllocation = true; }, lose: () => live.clear() };
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
    atlasCell(a, { run, font, dpr: 1, x: 0, width: 24 });
    atlasCell(a, { run: { ...run, from: 3, to: 6 }, font, dpr: 1, x: 0, width: 24 });
    expect(createCanvas).toHaveBeenCalledTimes(1);
    expect(r.calls.filter(call => call.name === 'texImage2D')).toHaveLength(1);
    const geometryWithStaging = b.counters.byKind.geometry;
    expect(geometryWithStaging).toBeGreaterThanOrEqual(24 * 20 * 4);
    a.endFrame(); expect(b.counters.byKind.geometry).toBe(geometryWithStaging);
    a.beginFrame(); atlasCell(a, { run: { ...run, from: 6, to: 9 }, font, dpr: 1, x: 0, width: 24 });
    expect(createCanvas).toHaveBeenCalledTimes(1);
    a.dispose(); expect(b.usedBytes).toBe(0);
  });
  it('crops the whole run and masks syntax without reshaping substrings', () => {
    const r = recordingGL(64); const raster = rasterizer(); const b = new ResourceLedger(); const a = new GlyphAtlas(r.gl, b, raster.createCanvas);
    const run = { text: 'ffi日本', from: 0, to: 5, x: 0, width: 50 };
    const tile = atlasCell(a, { run, font, dpr: 1, x: 32, width: 18, styles: [{ from: 3, to: 5, color: '#ff0000' }] })!;
    expect(tile.width).toBe(22); expect(raster.text.map(t => t.text)).toEqual(['ffi日本']); expect(raster.text[0]!.x).toBe(-30);
    expect(raster.crops).toContainEqual([2, 0, 18, 20]);
    a.dispose(); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0);
  });
  it('rasterizes a styled multi-tile run once and releases its bounded raster', () => {
    const r = recordingGL(128); const raster = rasterizer(); const b = new ResourceLedger(); const a = new GlyphAtlas(r.gl, b, raster.createCanvas);
    const run = { text: 'a'.repeat(32), from: 0, to: 32, x: 0, width: 256 };
    const styles = [{ from: 8, to: 12, color: '#ff0000' }];
    for (const x of [0, 48, 96, 144]) expect(atlasCell(a, { run, font, dpr: 1, x, width: 40, styles })).not.toBeNull();
    expect(raster.text.map(t => t.text)).toEqual([run.text, run.text, run.text, run.text]);
    expect(r.calls.filter(call => call.name === 'texImage2D')).toHaveLength(1);
    const cachedRasterBytes = b.counters.byKind.geometry;
    expect(cachedRasterBytes).toBeGreaterThan(0);
    a.endFrame(); expect(b.counters.byKind.geometry).toBe(cachedRasterBytes);
    a.dispose(); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0);
  });
  it('caches mask cells by text/font/fallback/DPR while tint stays per instance', () => {
    const r = recordingGL(); const b = new ResourceLedger(5 * MiB); const a = new GlyphAtlas(r.gl, b, rasterizer().createCanvas);
    const run = { text: 'abc', from: 0, to: 3, x: 0, width: 24 };
    const req = { run, font, dpr: 1, x: 0, width: 24 };
    const first = atlasCell(a, req)!; expect(atlasCell(a, req)!.texture).toBe(first.texture); expect(a.stats.uploads).toBe(1);
    atlasCell(a, { ...req, font: { ...font, fallback: 'other' } });
    atlasCell(a, { ...req, color: '#ff0000' }); atlasCell(a, { ...req, dpr: 2 });
    expect(a.stats.evictions).toBe(0); expect(a.stats.uploads).toBe(3); expect(b.usedBytes).toBeLessThanOrEqual(5 * MiB);
    const generation = a.stats.generation; a.invalidate(); a.beginFrame();
    expect(a.stats.resets).toBe(1); expect(a.stats.generation).toBe(generation + 1); expect(a.size).toBe(0);
    const uploads = a.stats.uploads; expect(atlasCell(a, req)).not.toBeNull(); expect(a.stats.uploads).toBe(uploads + 1);
    a.dispose(); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0);
  });
  it('reuses syntax tiles when an unchanged run shifts in document offsets', () => {
    const r = recordingGL(); const b = new ResourceLedger(); const a = new GlyphAtlas(r.gl, b, rasterizer().createCanvas);
    const run = { text: 'abc', from: 10, to: 13, x: 0, width: 24 };
    const first = atlasCell(a, { run, font, dpr: 1, x: 0, width: 24, styles: [{ from: 11, to: 12, color: '#ff0000' }] })!;
    run.from += 10; run.to += 10;
    const shifted = atlasCell(a, { run, font, dpr: 1, x: 0, width: 24, styles: [{ from: 21, to: 22, color: '#ff0000' }] })!;
    expect(shifted.texture).toBe(first.texture);
    expect(a.stats).toMatchObject({ uploads: 1, hits: 1 });
    a.dispose(); expect(b.usedBytes).toBe(0);
  });
  it('rolls back reservations and textures on upload failure', () => {
    const r = recordingGL(); const b = new ResourceLedger(); const a = new GlyphAtlas(r.gl, b, rasterizer().createCanvas); const before = b.usedBytes; r.failNextUpload();
    const req = { run: { text: 'a', from: 0, to: 1, x: 0, width: 8 }, font, dpr: 1, x: 0, width: 8 };
    expect(() => atlasCell(a, req)).toThrow('refused');
    expect(b.usedBytes).toBe(before); expect(a.size).toBe(0); expect(a.stats.uploads).toBe(0);
    expect(atlasCell(a, req)).not.toBeNull(); expect(a.stats.uploads).toBe(1);
    a.dispose(); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0); expect(a.size).toBe(0);
    const failed = recordingGL(); const failedLedger = new ResourceLedger(); failed.failNextAllocation();
    expect(() => new GlyphAtlas(failed.gl, failedLedger, rasterizer().createCanvas)).toThrow('Text atlas allocation failed');
    expect(failedLedger.usedBytes).toBe(0); expect(failed.live.size).toBe(0);
  });
  it('bounds atlas misses per frame while allowing resident hits', () => {
    const r = recordingGL(); const b = new ResourceLedger(); const a = new GlyphAtlas(r.gl, b, rasterizer().createCanvas);
    const req = { run: { text: 'a', from: 0, to: 1, x: 0, width: 8 }, font, dpr: 1, x: 0, width: 8 };
    a.beginFrame(1200); expect(atlasCell(a, req)).not.toBeNull();
    a.beginFrame(0); expect(atlasCell(a, req)).not.toBeNull();
    expect(atlasCell(a, { ...req, x: 8 })).toBeNull(); expect(a.stats.uploads).toBe(1);
    a.dispose(); expect(b.usedBytes).toBe(0);
  });
});

describe('GPU code compositor', () => {
  it('draws momentary labels without accumulating overlay instances', () => {
    const f = fixture('let x = 1');
    const first = { kind: 'momentary' as const, from: 9, to: 9, label: ' ~ 1.2' };
    const countsSince = (start: number) => {
      const counts = new Map<string, number>();
      for (const call of f.r.calls.slice(start)) {
        if (call.name !== 'drawArraysInstanced') continue;
        const layer = String(call.args[4]);
        counts.set(layer, (counts.get(layer) ?? 0) + Number(call.args[3]));
      }
      return counts;
    };
    let start = f.r.calls.length;
    expect(f.renderer.render({ textRevision: 1, animated: [] })).toBe(true);
    const baselineCounts = countsSince(start);
    start = f.r.calls.length;
    expect(f.renderer.render({ textRevision: 1, animated: [first] })).toBe(true);
    const firstCounts = countsSince(start);
    expect(firstCounts.get('overlay') ?? 0).toBeGreaterThan(baselineCounts.get('overlay') ?? 0);
    expect(firstCounts.get('overlayText') ?? 0).toBeGreaterThan(baselineCounts.get('overlayText') ?? 0);
    expect(f.raster.text.some(t => t.text.includes('~'))).toBe(true);
    start = f.r.calls.length;
    expect(f.renderer.render({ textRevision: 1, animated: [{ ...first, label: ' ~ 2.4' }] })).toBe(true);
    const secondCounts = countsSince(start);
    expect(secondCounts).toEqual(firstCounts);
    start = f.r.calls.length;
    expect(f.renderer.render({ textRevision: 1, animated: [] })).toBe(true);
    expect(countsSince(start)).toEqual(baselineCounts);
    f.renderer.dispose();
  });
  it('keeps Text-backed and string-backed renderer commands identical', () => {
    const first = fixture(), second = fixture();
    const text = 'let x = 1\n日本';
    first.renderer.setDocument(text); second.renderer.setText(Text.of(['let x = 1', '日本']));
    const feedback = { annotations: [{ kind: 'syntax' as const, from: 0, to: 3, className: 'vact-tok-head' }], cursor: 4, textRevision: 1 };
    expect(first.renderer.render(feedback), first.renderer.status.message).toBe(true); expect(second.renderer.render(feedback), second.renderer.status.message).toBe(true);
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
    const colors = f.r.draws.map(d => d.color); const q = (rgba: number[]) => rgba.map(channel => Math.round(channel * 255) / 255);
    expect(colors.slice(0, 4)).toEqual([[0.04, 0.05, 0.07, 0.85], [0.12, 0.22, 0.32, 0.55], [0.35, 0.29, 0.13, 0.5], [0.2, 0.32, 0.36, 0.5]].map(q));
    const diagnostic = colors.findIndex(c => Math.abs(c[0]! - 0.75) < 0.01); const cursor = colors.findIndex(c => Math.abs(c[0]! - 0.9) < 0.01); const badge = colors.findIndex(c => Math.abs(c[0]! - 0.15) < 0.01);
    expect(diagnostic).toBeGreaterThan(3); expect(cursor).toBeGreaterThan(diagnostic); expect(badge).toBeGreaterThan(cursor);
    expect(colors.filter(c => Math.abs(c[0]! - 0.53) < 0.01 && Math.abs(c[1]! - 0.75) < 0.01).length).toBeGreaterThanOrEqual(2);
    expect(f.r.calls.some(c => c.name === 'drawArraysInstanced' && Number(c.args[3]) > 1)).toBe(true);
    expect(f.raster.text.some(t => t.text === 'l' && t.color === '#ffffff')).toBe(true);
    expect(f.r.draws.some(d => d.layer === 'text' && Math.abs(d.color[0]! - Math.round(0.92 * 255) / 255) < 0.001)).toBe(true);
    expect(f.raster.text.some(t => t.text === '1')).toBe(true);
    f.renderer.dispose(); expect(f.b.usedBytes).toBe(0); expect(f.r.live.size).toBe(0);
  });
  it('idle frames reuse text uploads, layout and static geometry; edits rebuild only dirty tiles', () => {
    const f = fixture(); expect(f.renderer.render()).toBe(true); const uploads = f.renderer.atlasStats.uploads; const builds = f.l.stats.builds;
    const textBuffer = f.r.calls.find(call => call.name === 'drawArraysInstanced' && call.args[4] === 'text')!.buffer;
    const textWrites = () => f.r.calls.filter(call => call.name === 'bufferSubData' && call.buffer === textBuffer).length;
    const initialWrites = textWrites(); expect(initialWrites).toBe(1); expect(f.renderer.stats.bufferUploads).toBe(initialWrites);
    for (let i = 0; i < 4; i++) expect(f.renderer.render({ cursor: i })).toBe(true);
    expect(f.renderer.atlasStats.uploads).toBe(uploads); expect(f.l.stats.builds).toBe(builds); expect(textWrites()).toBe(initialWrites);
    expect(f.renderer.stats.bufferUploads).toBe(textWrites());
    f.renderer.setDocument('let x = Ω\n日本'); f.renderer.render(); expect(f.renderer.atlasStats.uploads).toBe(uploads + 1);
    expect(textWrites()).toBe(initialWrites + 1); expect(f.renderer.stats.bufferUploads).toBe(textWrites());
    f.renderer.dispose();
  });
  it('rebuild-frame highlights never persist in the retained text layer', () => {
    const f = fixture(); const control = fixture();
    expect(control.renderer.render({ textRevision: 1 })).toBe(true);
    const glyphAndGutterInstances = control.r.calls.find(call => call.name === 'drawArraysInstanced' && call.args[4] === 'text')!.args[3];
    const hasColor = (draws: typeof f.r.draws, color: number[]) => draws.some(draw => color.every((channel, index) => Math.abs(draw.color[index]! - channel) < 0.01));
    const playing = [0.35, 0.29, 0.13, 0.5], selection = [0.12, 0.22, 0.32, 0.55];

    let drawStart = f.r.draws.length;
    expect(f.renderer.render({ textRevision: 1, annotations: [
      { kind: 'playing', from: 0, to: 3 }, { kind: 'selection', from: 4, to: 6 },
    ] })).toBe(true);
    let frameDraws = f.r.draws.slice(drawStart);
    expect(hasColor(frameDraws.filter(draw => draw.layer === 'background'), playing)).toBe(true);
    expect(hasColor(frameDraws.filter(draw => draw.layer === 'background'), selection)).toBe(true);
    expect(hasColor(frameDraws.filter(draw => draw.layer === 'text'), playing)).toBe(false);
    expect(hasColor(frameDraws.filter(draw => draw.layer === 'text'), selection)).toBe(false);
    const textBuffer = f.r.calls.find(call => call.name === 'drawArraysInstanced' && call.args[4] === 'text')!.buffer;
    const textWrites = () => f.r.calls.filter(call => call.name === 'bufferSubData' && call.buffer === textBuffer).length;
    const initialWrites = textWrites();
    const frameTextCount = () => f.r.calls.filter(call => call.name === 'drawArraysInstanced' && call.args[4] === 'text').at(-1)!.args[3];
    expect(frameTextCount()).toBe(glyphAndGutterInstances);

    drawStart = f.r.draws.length;
    expect(f.renderer.render({ textRevision: 1, annotations: [] })).toBe(true);
    frameDraws = f.r.draws.slice(drawStart);
    expect(textWrites()).toBe(initialWrites);
    expect(frameTextCount()).toBe(glyphAndGutterInstances);
    expect(hasColor(frameDraws, playing)).toBe(false);
    expect(hasColor(frameDraws, selection)).toBe(false);

    drawStart = f.r.draws.length;
    expect(f.renderer.render({ textRevision: 1, animated: [{ kind: 'playing', from: 0, to: 3 }] })).toBe(true);
    frameDraws = f.r.draws.slice(drawStart);
    expect(hasColor(frameDraws.filter(draw => draw.layer === 'background'), playing)).toBe(true);
    expect(textWrites()).toBe(initialWrites);
    f.renderer.dispose(); control.renderer.dispose();
  });
  it('animation-only frames replay cached text without shaping or uploads', () => {
    const f = fixture(); f.renderer.render({ textRevision: 1 });
    const builds = f.renderer.stats.textBuilds, uploads = f.renderer.atlasStats.uploads, buffers = f.renderer.stats.bufferUploads;
    const textBuffer = f.r.calls.find(call => call.name === 'drawArraysInstanced' && call.args[4] === 'text')!.buffer;
    const textWrites = () => f.r.calls.filter(call => call.name === 'bufferSubData' && call.buffer === textBuffer).length;
    const initialWrites = textWrites(); const initialInstances = f.r.draws.filter(draw => draw.layer === 'text').map(draw => draw.rect);
    const shape = vi.spyOn(f.l, 'shape');
    expect(f.renderer.render({ textRevision: 1, animated: [{ kind: 'playing', from: 0, to: 3 }] })).toBe(true);
    expect(shape).not.toHaveBeenCalled(); expect(f.renderer.stats.textBuilds).toBe(builds);
    expect(f.renderer.atlasStats.uploads).toBe(uploads); expect(f.renderer.stats.bufferUploads).toBe(buffers); expect(textWrites()).toBe(initialWrites);
    expect(f.renderer.render({ textRevision: 1, animated: [{ kind: 'playing', from: 4, to: 8 }] })).toBe(true);
    expect(shape).not.toHaveBeenCalled(); expect(f.renderer.atlasStats.uploads).toBe(uploads); expect(textWrites()).toBe(initialWrites);
    expect(f.r.draws.filter(draw => draw.layer === 'text').slice(-initialInstances.length).map(draw => draw.rect)).toEqual(initialInstances);
    expect(f.renderer.stats.bufferUploads).toBe(textWrites()); f.renderer.dispose();
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
    expect(f.renderer.render({ annotations: first, textRevision: 1, annotationsRevision: 1, cursor: 0, handles: [] })).toBe(true);
    const textBuffer = f.r.calls.find(call => call.name === 'drawArraysInstanced' && call.args[4] === 'text')?.buffer;
    const overlayBuffer = f.r.calls.find(call => call.name === 'drawArraysInstanced' && call.args[4] === 'overlay')?.buffer;
    const textWrites = () => f.r.calls.filter(call => call.name === 'bufferSubData' && call.buffer === textBuffer).length;
    const builds = f.renderer.stats.textBuilds;
    const writes = textWrites();
    const overlayWrites = f.r.calls.filter(call => call.name === 'bufferSubData' && call.buffer === overlayBuffer).length;
    const caretBefore = f.r.draws.filter(draw => draw.layer === 'overlay').at(-1)?.rect;
    expect(f.renderer.render({ annotations: first, textRevision: 1, annotationsRevision: 1, cursor: 2, handles: [] })).toBe(true);
    expect(f.renderer.stats.textBuilds).toBe(builds);
    expect(textWrites()).toBe(writes);
    expect(f.r.calls.filter(call => call.name === 'bufferSubData' && call.buffer === overlayBuffer).length).toBeGreaterThan(overlayWrites);
    expect(f.r.draws.filter(draw => draw.layer === 'overlay').at(-1)?.rect).not.toEqual(caretBefore);
    expect(f.renderer.stats.lastFrameDraws).toBeLessThanOrEqual(4);
    expect(f.renderer.render({ annotations: [{ ...first[0]!, className: 'vact-tok-number' }], textRevision: 1, annotationsRevision: 2 })).toBe(true);
    expect(f.renderer.stats.textBuilds).toBe(builds + 1); f.renderer.dispose();
  });
  it('replays gutter numbers unscissored and source text clipped on cached frames', () => {
    const f = fixture();
    const frameDraws = (start: number) => f.r.draws.slice(start).filter(draw => draw.layer === 'text' && draw.rect[3] === font.lineHeight);
    const gutterDraws = (draws: ReturnType<typeof frameDraws>) => draws.filter(draw => draw.flags === 0);
    const sourceDraws = (draws: ReturnType<typeof frameDraws>) => draws.filter(draw => (draw.flags & 1) !== 0);
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
    expect(rebuildSource.every(draw => (draw.flags & 1) !== 0)).toBe(true); expect(cachedSource.every(draw => (draw.flags & 1) !== 0)).toBe(true);
    f.renderer.dispose();
  });
  it('rebuilds static draw lists on revision and draws call-head underline without a label', () => {
    const f = fixture('call()'); f.renderer.render({ textRevision: 1 }); const builds = f.renderer.stats.textBuilds;
    expect(f.renderer.render({ textRevision: 2, annotations: [{ kind: 'call-head', from: 0, to: 4, label: 'must-not-render' }] })).toBe(true);
    expect(f.renderer.stats.textBuilds).toBe(builds);
    expect(f.r.draws.some(draw => Math.abs(draw.color[0]! - 0.55) < 0.01 && Math.abs(draw.color[1]! - 0.6) < 0.01)).toBe(true);
    expect(f.raster.text.some(entry => entry.text === 'must-not-render')).toBe(false); f.renderer.dispose();
  });
  it('uploads only the changed line block after a one-character Text edit', () => {
    const lines = Array.from({ length: 60 }, (_, index) => `const line${index} = value`);
    const doc = Text.of(lines), l = new TextLayout({ font: '', measureText: text => ({ width: width(text) }) }, font);
    l.setText(doc); const r = recordingGL(), raster = rasterizer(), b = new ResourceLedger();
    const renderer = new CanvasRenderer(document.createElement('canvas'), l, { gl: r.gl, ledger: b, createCanvas: raster.createCanvas });
    renderer.setViewport({ ...view, height: 1200 }); expect(renderer.render()).toBe(true);
    const textBuffer = r.calls.find(call => call.name === 'drawArraysInstanced' && call.args[4] === 'text')?.buffer;
    expect(textBuffer).toBeDefined();
    const before = { builds: renderer.stats.textBuilds, bytes: renderer.stats.geometryBytes, uploads: renderer.atlasStats.uploads,
      dataCalls: r.calls.filter(call => call.name === 'bufferData' && call.buffer === textBuffer).length,
      subCalls: r.calls.filter(call => call.name === 'bufferSubData' && call.buffer === textBuffer).length };
    const line = doc.line(30), changes = ChangeSet.of({ from: line.from + 6, to: line.from + 7, insert: 'Z' }, doc.length), next = changes.apply(doc);
    l.setText(next, changes); renderer.setText(next); expect(renderer.render()).toBe(true);
    expect(renderer.stats.geometryBytes - before.bytes).toBeLessThanOrEqual(2048);
    expect(renderer.stats.textBuilds - before.builds).toBe(1);
    expect(renderer.atlasStats.uploads - before.uploads).toBeLessThanOrEqual(1);
    expect(r.calls.filter(call => call.name === 'bufferData' && call.buffer === textBuffer)).toHaveLength(before.dataCalls);
    expect(r.calls.filter(call => call.name === 'bufferSubData' && call.buffer === textBuffer).length - before.subCalls).toBeLessThanOrEqual(1);
    const controlBuilds = renderer.stats.textBuilds; l.setFont({ ...font, generation: 1 }); expect(renderer.render()).toBe(true);
    expect(renderer.stats.textBuilds - controlBuilds).toBe(60);
    renderer.dispose();
  });
  it('keeps shifted line segments and rewrites only renumbered gutters on Enter', () => {
    const doc = Text.of(Array.from({ length: 60 }, (_, index) => `line ${index}`));
    const l = new TextLayout({ font: '', measureText: text => ({ width: width(text) }) }, font); l.setText(doc);
    const r = recordingGL(2048), raster = rasterizer(), renderer = new CanvasRenderer(document.createElement('canvas'), l, { gl: r.gl, createCanvas: raster.createCanvas });
    renderer.setViewport({ ...view, height: 1200 }); expect(renderer.render()).toBe(true);
    const cache = (renderer as unknown as { segments: { entriesInOrder(): Array<{ key: { runs: readonly unknown[]; chunk: number }; blocks: number[] }> } }).segments;
    const shiftedRuns = l.shape(30).runs;
    const shiftedBlocks = cache.entriesInOrder().find(entry => entry.key.runs === shiftedRuns && entry.key.chunk === 0)?.blocks;
    const before = { builds: renderer.stats.textBuilds, gutters: renderer.stats.gutterBuilds, slots: renderer.stats.slotTableBytes, uploads: renderer.atlasStats.uploads };
    const line = doc.line(30), changes = ChangeSet.of({ from: line.from, to: line.from, insert: '\n' }, doc.length), next = changes.apply(doc);
    l.setText(next, changes); renderer.setText(next); expect(renderer.render()).toBe(true);
    expect(renderer.stats.textBuilds - before.builds).toBeLessThanOrEqual(2);
    expect(renderer.stats.slotTableBytes).toBeGreaterThan(before.slots);
    expect(renderer.atlasStats.uploads).toBe(before.uploads);
    expect(renderer.stats.gutterBuilds - before.gutters).toBeGreaterThan(0);
    expect(renderer.stats.gutterBuilds - before.gutters).toBeLessThanOrEqual(32);
    expect(cache.entriesInOrder().find(entry => entry.key.runs === shiftedRuns && entry.key.chunk === 0)?.blocks).toEqual(shiftedBlocks);
    renderer.dispose();
  });
  it('builds only the entering line, retains one viewport of overscan, and compacts freed blocks', () => {
    const lines = Array.from({ length: 61 }, (_, index) => `line ${index}`), doc = Text.of(lines);
    const l = new TextLayout({ font: '', measureText: text => ({ width: width(text) }) }, font); l.setText(doc);
    const r = recordingGL(2048), raster = rasterizer(), renderer = new CanvasRenderer(document.createElement('canvas'), l, { gl: r.gl, createCanvas: raster.createCanvas });
    renderer.setViewport({ ...view, height: 1200 }); expect(renderer.render()).toBe(true);
    const builds = renderer.stats.textBuilds;
    const cache = (renderer as unknown as { segments: { entriesInOrder(): Array<{ key: { runs: readonly unknown[]; chunk: number }; blocks: number[]; value: Array<{ slot: number }> }> } }).segments;
    const leavingRuns = l.shape(0).runs;
    const leavingText = cache.entriesInOrder().find(entry => entry.key.runs === leavingRuns && entry.key.chunk === 0)!;
    const leavingGutter = cache.entriesInOrder().find(entry => entry.key.chunk === -1 && entry.value[0]?.slot === leavingText.value[0]?.slot);
    const leavingBlocks = [...leavingText.blocks, ...(leavingGutter?.blocks ?? [])];
    renderer.setViewport({ ...view, height: 1200, scrollTop: 20 }); expect(renderer.render()).toBe(true);
    expect(renderer.stats.textBuilds - builds).toBe(1);
    expect(cache.entriesInOrder().some(entry => entry.key.runs === leavingRuns && entry.key.chunk === 0)).toBe(true);
    const enteringRuns = l.shape(60).runs;
    expect(cache.entriesInOrder().find(entry => entry.key.runs === enteringRuns && entry.key.chunk === 0)?.blocks.some(block => leavingBlocks.includes(block))).toBe(false);
    renderer.setViewport({ ...view, height: 1200, scrollTop: 1220 }); expect(renderer.render()).toBe(true);
    expect(cache.entriesInOrder().some(entry => entry.key.runs === leavingRuns && entry.key.chunk === 0)).toBe(false);
    const beforeCompact = renderer.stats.compactions;
    const compactFrameStart = r.draws.length;
    renderer.setViewport({ ...view, height: 200, scrollTop: 20 }); expect(renderer.render()).toBe(true);
    expect(renderer.stats.compactions).toBe(beforeCompact + 1);
    expect(renderer.stats.decodedTextInstances % 64).toBe(0);
    const visibleText = (draws: typeof r.draws, viewportHeight: number) => draws
      .filter(draw => draw.layer === 'text' && !(draw.rect.every(value => value === 0) && draw.color.every(value => value === 0)))
      .filter(draw => draw.absTop >= 0 && draw.absTop < viewportHeight)
      .map(({ absLeft, absTop, rect, color, flags }) => ({
        absLeft: Math.round(absLeft * 1000) / 1000,
        absTop: Math.round(absTop * 1000) / 1000,
        width: Math.round(rect[2] * 1000) / 1000,
        height: Math.round(rect[3] * 1000) / 1000,
        color: color.map(value => Math.round(value * 1000) / 1000),
        flags,
      }))
      .sort((a, b) => a.absTop - b.absTop || a.absLeft - b.absLeft || a.width - b.width || a.height - b.height || a.flags - b.flags || a.color.join(',').localeCompare(b.color.join(',')));
    const compacted = visibleText(r.draws.slice(compactFrameStart), 200);
    const coldReference = (scrollTop: number) => {
      const referenceLayout = new TextLayout({ font: '', measureText: text => ({ width: width(text) }) }, font); referenceLayout.setText(doc);
      const referenceGL = recordingGL(2048), referenceRaster = rasterizer();
      const referenceRenderer = new CanvasRenderer(document.createElement('canvas'), referenceLayout, { gl: referenceGL.gl, createCanvas: referenceRaster.createCanvas });
      referenceRenderer.setViewport({ ...view, height: 200, scrollTop });
      for (let frame = 0; frame < 8; frame++) {
        expect(referenceRenderer.render()).toBe(true);
        if (!referenceRenderer.textPending) break;
      }
      expect(referenceRenderer.textPending).toBe(false);
      const visible = visibleText(referenceGL.draws, 200);
      referenceRenderer.dispose();
      return visible;
    };
    const expected = coldReference(20);
    expect(compacted).toEqual(expected);
    expect(compacted.length).toBeGreaterThan(0);
    expect(new Set(compacted.map(instance => instance.absTop)).size).toBe(10);
    expect(new Set(expected.map(instance => instance.absTop)).size).toBe(10);
    expect(compacted).not.toEqual(coldReference(40));
    renderer.dispose();
  });
  it('keeps one live slot per visible line through back-and-forth scrolling', () => {
    const doc = Text.of(Array.from({ length: 200 }, (_, index) => `line ${index}`));
    const l = new TextLayout({ font: '', measureText: text => ({ width: width(text) }) }, font); l.setText(doc);
    const r = recordingGL(2048), raster = rasterizer(), renderer = new CanvasRenderer(document.createElement('canvas'), l, { gl: r.gl, createCanvas: raster.createCanvas });
    const lineSlots = (renderer as unknown as { lineSlots: WeakMap<object, number> }).lineSlots;
    const checkVisibleRows = (scrollTop: number, drawStart: number) => {
      const draws = r.draws.slice(drawStart).filter(draw => draw.layer === 'text' && draw.rect[3] > 0 && draw.absTop >= 0 && draw.absTop < 200);
      const visibleSlots = new Set(draws.map(draw => draw.slot));
      const slotsByTop = new Map<number, Set<number>>();
      for (const draw of draws) {
        const slots = slotsByTop.get(draw.absTop) ?? new Set<number>(); slots.add(draw.slot); slotsByTop.set(draw.absTop, slots);
      }
      const firstLine = Math.floor(scrollTop / font.lineHeight);
      const expectedTop = Array.from({ length: 10 }, (_, index) => (firstLine + index) * font.lineHeight - scrollTop);
      expect([...slotsByTop.keys()].sort((a, b) => a - b)).toEqual(expectedTop);
      const slotsAtLines = expectedTop.map((_top, index) => {
        const slot = lineSlots.get(l.shape(firstLine + index).runs as object);
        expect(slot).toBeDefined();
        expect(slotsByTop.get(expectedTop[index]!)?.has(slot!)).toBe(true);
        return slot!;
      });
      expect(new Set(slotsAtLines).size).toBe(10);
      expect(visibleSlots.size).toBe(10);
      return slotsAtLines;
    };
    const renderAt = (scrollTop: number) => {
      const start = r.draws.length; renderer.setViewport({ ...view, height: 200, scrollTop });
      expect(renderer.render()).toBe(true); return checkVisibleRows(scrollTop, start);
    };
    renderAt(400);
    const retainedSlot = lineSlots.get(l.shape(21).runs as object);
    const builds = renderer.stats.textBuilds;
    const at420 = renderAt(420);
    expect(lineSlots.get(l.shape(21).runs as object)).toBe(retainedSlot);
    expect(renderer.stats.textBuilds - builds).toBe(1);
    expect(at420).toContain(retainedSlot);
    renderAt(400); renderAt(340); renderAt(160);
    renderer.dispose();
  });
  it('keeps gutter line numbers fixed across horizontal scrolling and new rows', () => {
    const text = ['x'.repeat(600), 'row 1', 'row 2', '', 'row 3', ...Array.from({ length: 57 }, (_, index) => `row ${index + 4}`)].join('\n');
    const f = fixture(text);
    const gutterAt = (start: number, scrollTop: number) => {
      const draws = f.r.draws.slice(start).filter(draw => draw.layer === 'text' && draw.flags === 0 && draw.rect[3] > 0 && draw.absTop >= 0 && draw.absTop < 100);
      expect(draws.length).toBeGreaterThan(0);
      expect(draws.every(draw => draw.absLeft >= 0 && draw.absLeft < 48)).toBe(true);
      const instances = draws.map(draw => `${draw.absTop}:${draw.absLeft}:${draw.rect[2]}:${draw.rect[3]}:${draw.color.join(',')}`);
      expect(new Set(instances).size).toBe(instances.length);
      const firstLine = Math.floor(scrollTop / font.lineHeight);
      const expectedTops = Array.from({ length: 5 }, (_, index) => (firstLine + index) * font.lineHeight - scrollTop);
      expect([...new Set(draws.map(draw => draw.absTop))].sort((a, b) => a - b)).toEqual(expectedTops);
      return draws;
    };
    const frame = (scrollLeft: number, scrollTop: number) => {
      const start = f.r.draws.length; f.renderer.setViewport({ ...view, height: 100, scrollLeft, scrollTop });
      expect(f.renderer.render()).toBe(true); return { start, draws: f.r.draws.slice(start) };
    };
    const initial = frame(0, 0), initialGutter = gutterAt(initial.start, 0);
    const initialText = initial.draws.filter(draw => draw.layer === 'text' && (draw.flags & 1) !== 0 && draw.absTop === 0);
    expect(initialText.length).toBeGreaterThan(0);
    const scrolled = frame(200, 0), scrolledGutter = gutterAt(scrolled.start, 0);
    const scrolledText = scrolled.draws.filter(draw => draw.layer === 'text' && (draw.flags & 1) !== 0 && draw.absTop === 0);
    const sameCell = initialText.find(before => scrolledText.some(after => after.rect[0] === before.rect[0] && after.rect[2] === before.rect[2] && after.color.join(',') === before.color.join(',')));
    expect(sameCell).toBeDefined();
    const movedCell = scrolledText.find(after => after.rect[0] === sameCell!.rect[0] && after.rect[2] === sameCell!.rect[2] && after.color.join(',') === sameCell!.color.join(','))!;
    expect(movedCell.absLeft - sameCell!.absLeft).toBe(-200);
    expect(scrolledGutter.map(draw => draw.absTop)).toEqual(initialGutter.map(draw => draw.absTop));
    gutterAt(frame(0, 0).start, 0);
    const entered = frame(30, 400); gutterAt(entered.start, 400);
    gutterAt(frame(0, 400).start, 400);
    const farTop = frame(1000, 0); gutterAt(farTop.start, 0);
    expect(farTop.draws.filter(draw => draw.layer === 'text' && (draw.flags & 1) !== 0 && draw.absTop > 0 && draw.absTop < 100)).toHaveLength(0);
    gutterAt(frame(1000, 400).start, 400);
    gutterAt(frame(0, 0).start, 0);
    gutterAt(frame(1000, 0).start, 0);
    f.renderer.dispose();
  });
  it('rebuilds each visible text segment after context restore and DPR change', () => {
    const f = fixture(Array.from({ length: 12 }, (_, index) => `line ${index}`).join('\n'));
    f.renderer.setViewport({ ...view, height: 240 }); expect(f.renderer.render()).toBe(true);
    const expected = f.renderer.stats.textBuilds;
    f.r.lose(); f.c.dispatchEvent(new Event('webglcontextlost', { cancelable: true }));
    f.c.dispatchEvent(new Event('webglcontextrestored')); expect(f.renderer.render()).toBe(true);
    expect(f.renderer.stats.textBuilds - expected).toBe(12);
    const beforeDpr = f.renderer.stats.textBuilds;
    f.renderer.setViewport({ ...view, height: 240 }, 2); expect(f.renderer.render()).toBe(true);
    expect(f.renderer.stats.textBuilds - beforeDpr).toBe(12);
    f.renderer.dispose();
  });
  it('keeps textPending until a bounded per-frame atlas upload completes', () => {
    const r = recordingGL(1024); const raster = rasterizer(); const b = new ResourceLedger();
    const text = Array.from({ length: 35 }, (_, line) => Array.from({ length: 40 }, (_, i) => String.fromCharCode(0xe000 + line * 40 + i)).join('')).join('\n');
    const renderer = new CanvasRenderer(document.createElement('canvas'), layout(text), { gl: r.gl, ledger: b, createCanvas: raster.createCanvas });
    renderer.setViewport({ width: 640, height: 1000, scrollLeft: 0, scrollTop: 0, gutter: 0 });
    expect(renderer.render({ textRevision: 1 })).toBe(true); expect(renderer.textPending).toBe(true);
    expect(renderer.atlasStats.uploads).toBeLessThan(1400);
    for (let i = 0; i < 3 && renderer.textPending; i++) expect(renderer.render({ textRevision: 1 })).toBe(true);
    expect(renderer.textPending).toBe(false); expect(renderer.atlasStats.uploads).toBe(1400);
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
    const f = fixture(); f.r.failNextAllocation(); expect(f.renderer.render()).toBe(false);
    expect(f.renderer.status.kind).toBe('unavailable'); expect(f.renderer.status.saveText()).toBe('let x = 1\n日本'); expect(f.b.usedBytes).toBe(0); expect(f.r.live.size).toBe(0);
    f.renderer.setDocument('save me'); expect(f.renderer.status.saveText()).toBe('save me'); f.renderer.dispose();

    const reset = fixture(); expect(reset.renderer.render()).toBe(true);
    reset.renderer.setViewport({ ...view }, 2); reset.r.failNextAllocation();
    expect(reset.renderer.render()).toBe(false); expect(reset.renderer.status.kind).toBe('unavailable');
    expect(reset.renderer.status.saveText()).toBe('let x = 1\n日本'); expect(reset.b.usedBytes).toBe(0); expect(reset.r.live.size).toBe(0);
    reset.renderer.dispose();
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
    expect(f.r.calls.filter(c => c.name === 'uniform1f' && (c.args[0] as { name: string }).name === 'u_gutter').at(-1)?.args[1]).toBe(48); f.renderer.dispose();
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
          expect(firstUploads).toBe(new Set([...Array.from({ length: 40 }, (_, i) => `line ${i}`)].join('') + Array.from({ length: 40 }, (_, i) => String(i + 1)).join('')).size);
          expect(raster.text.some(entry => entry.text === 'l')).toBe(true);
        } else {
          expect(renderer.atlasStats.uploads).toBe(firstUploads);
          expect(textures).toEqual(firstTextures); // No skipped or replaced visible tiles.
          expect(b.usedBytes).toBe(firstBytes);
        }
        expect(renderer.atlasStats.evictions).toBe(0);
        expect(b.counters.byKind.backdrop).toBe(0);
      }
    expect(r.calls.filter(call => call.name === 'texSubImage2D')).toHaveLength(firstUploads + 2); // White texels, slot table and distinct glyph cells.
      expect(r.calls.filter(call => call.name === 'texImage2D')).toHaveLength(3); // Atlas, slots and white fallback; no backdrop texture.
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
    const plain = fixture('x'.repeat(1024 * 1024)); expect(plain.renderer.render()).toBe(true);
    const baselineGeometry = plain.b.counters.byKind.geometry; plain.renderer.dispose();
    expect(f.b.counters.byKind.geometry - baselineGeometry).toBeLessThan(10_000);
    expect(f.raster.crops.length).toBeLessThan(32); f.renderer.dispose(); expect(f.b.usedBytes).toBe(0);
  });
  it('atlas does not retain historical full source strings outside the layout cache', () => {
    const f = fixture('x'.repeat(100_000));
    for (let i = 0; i < 12; i++) { f.renderer.setDocument('x'.repeat(99_999) + String(i)); expect(f.renderer.render()).toBe(true); }
    expect(f.l.cacheBytes).toBeLessThan(201_000); expect(f.b.counters.byKind.geometry).toBeLessThanOrEqual(RESOURCE_LIMITS.geometry);
    f.renderer.dispose(); expect(f.b.usedBytes).toBe(0);
  });
  it('keeps rendering every visible line while a small atlas working set stays pending', () => {
    const atlasDimension = 64; // Smallest power of two that holds 40 visible line slots plus slot zero.
    const r = recordingGL(atlasDimension); const b = new ResourceLedger(2_100_000); const c = document.createElement('canvas'); const raster = rasterizer();
    const text = Array.from({ length: 40 }, (_, line) => Array.from({ length: 122 }, (_, i) => String.fromCharCode(0xe000 + line * 122 + i)).join('')).join('\n');
    const renderer = new CanvasRenderer(c, layout(text), { gl: r.gl, ledger: b, createCanvas: raster.createCanvas }); renderer.setViewport({ width: 1024, height: 800, scrollLeft: 0, scrollTop: 0, gutter: 0 });
    const cells = 40 * 122, cellsPerAtlas = Math.floor((atlasDimension - 2) / 12) * Math.floor(atlasDimension / 20);
    const loopBound = Math.ceil(cells / cellsPerAtlas) + 2;
    const drawnLines = new Set<number>();
    for (let frame = 0; frame < loopBound && drawnLines.size < 40; frame++) {
      const start = r.draws.length, rasterStart = raster.text.length; expect(renderer.render()).toBe(true);
      const frameDraws = r.draws.slice(start).filter(draw => draw.layer === 'text');
      expect(frameDraws.length).toBeGreaterThan(0);
      expect(frameDraws.every(draw => draw.textureLiveAtDraw)).toBe(true);
      expect(frameDraws.every(draw => draw.uv.every(value => value >= 0 && value <= 1))).toBe(true);
      const rasterizedLines = new Set(raster.text.slice(rasterStart).flatMap(entry => {
        const code = entry.text.codePointAt(0) ?? 0;
        return code >= 0xe000 && code < 0xe000 + cells ? [Math.floor((code - 0xe000) / 122)] : [];
      }));
      const frameLines = frameDraws.map(draw => Math.floor(draw.rect[1] / font.lineHeight));
      for (const line of rasterizedLines) expect(frameLines).toContain(line);
      for (const line of frameLines) drawnLines.add(line);
      expect(renderer.textPending).toBe(true);
      expect(b.usedBytes).toBeLessThanOrEqual(2_100_000);
    }
    expect(renderer.atlasStats.evictions).toBeGreaterThan(0);
    expect([...drawnLines].sort((a, b) => a - b)).toEqual(Array.from({ length: 40 }, (_, i) => i));
    renderer.dispose(); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0);
  });
  it('releases initialization resources when shader or object setup fails', () => {
    const r = recordingGL(); Object.assign(r.gl, { getShaderParameter: () => false }); const b = new ResourceLedger();
    const renderer = new CanvasRenderer(document.createElement('canvas'), layout('safe'), { gl: r.gl, ledger: b });
    expect(renderer.status.kind).toBe('unavailable'); expect(renderer.status.saveText()).toBe('safe'); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0); renderer.dispose();
  });
  it('batches a 60-line viewport into one text instanced draw and at most four layer draws', () => {
    const f = fixture(Array.from({ length: 60 }, (_, i) => `line ${i}`).join('\n'));
    f.renderer.setViewport({ ...view, height: 1200 });
    expect(f.renderer.render({ textRevision: 1 })).toBe(true);
    const frame = f.r.calls.filter(call => call.name === 'drawArraysInstanced');
    expect(frame.filter(call => call.args[4] === 'text')).toHaveLength(1);
    expect(f.renderer.stats.lastFrameDraws).toBeLessThanOrEqual(4);
    expect(f.renderer.stats.decodedTextInstances).toBeGreaterThan(60);
    f.renderer.dispose();
  });
  it('defers atlas growth and reset to frame boundaries and bumps its generation', () => {
    const fill = (maxTexture: number) => {
      const r = recordingGL(maxTexture), b = new ResourceLedger(), a = new GlyphAtlas(r.gl, b, rasterizer().createCanvas);
      let index = 0, capacityMiss = false;
      for (let frame = 0; frame < 10 && !capacityMiss; frame++) {
        a.beginFrame();
        for (let n = 0; n < 2000; n++) {
          const text = String.fromCodePoint(0xf0000 + index);
          if (!a.cell({ kind: 'mask', text, font, dpr: 1, width: 8 })) { capacityMiss = a.stats.uploads > 4000; break; }
          index++;
        }
        a.endFrame();
      }
      const oldGeneration = a.stats.generation;
      a.beginFrame();
      const stats = { generation: a.stats.generation, growths: a.stats.growths, resets: a.stats.resets, oldGeneration };
      a.dispose(); expect(b.usedBytes).toBe(0); return stats;
    };
    const grown = fill(2048); expect(grown.growths).toBe(1); expect(grown.generation).toBeGreaterThan(grown.oldGeneration);
    const reset = fill(1024); expect(reset.resets).toBeGreaterThanOrEqual(1); expect(reset.generation).toBeGreaterThan(reset.oldGeneration);
  });
  it('resets at the frame boundary when the ledger refuses atlas growth', () => {
    const r = recordingGL(2048), b = new ResourceLedger(5 * MiB), a = new GlyphAtlas(r.gl, b, rasterizer().createCanvas);
    let index = 0, capacityMiss = false;
    for (let frame = 0; frame < 10 && !capacityMiss; frame++) {
      a.beginFrame(4 * MiB);
      for (let n = 0; n < 2000; n++) {
        const text = String.fromCodePoint(0xf0000 + index);
        if (!a.cell({ kind: 'mask', text, font, dpr: 1, width: 8 })) { capacityMiss = true; break; }
        index++;
      }
      a.endFrame();
    }
    expect(capacityMiss).toBe(true);
    const oldGeneration = a.stats.generation;
    a.beginFrame();
    expect(a.stats.growths).toBe(0); expect(a.stats.resets).toBeGreaterThanOrEqual(1);
    expect(a.stats.generation).toBeGreaterThan(oldGeneration); expect(b.usedBytes).toBeLessThanOrEqual(5 * MiB);
    a.dispose(); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0);
  });
  it('rasterizes emoji as untinted color cells and Hebrew as a shaped run', () => {
    const f = fixture('😀\nאבג');
    expect(f.l.textCells(f.l.shape(0).runs[0]!).map(cell => cell.text)).toContain('😀');
    expect(f.renderer.render({ textRevision: 1 })).toBe(true);
    expect(f.raster.text.some(item => item.text === '😀')).toBe(true);
    expect(f.r.draws.some(draw => draw.layer === 'text' && (draw.flags & 2) !== 0)).toBe(true);
    expect(f.raster.text.some(item => item.text === 'אבג')).toBe(true);
    expect(f.l.shape(1).rtlUnsupported).toBe(true);
    f.renderer.dispose();
  });
  it('keeps grapheme clusters intact at the 256-cluster geometry boundary', () => {
    const family = '👨‍👩‍👧‍👦', source = `${'x'.repeat(255)}${family}z`, f = fixture(source);
    const chunks = f.l.geometrySegments(f.l.shape(0));
    expect(chunks).toHaveLength(2);
    expect(chunks[0]!.runs.map(run => run.text).join('')).toBe(`${'x'.repeat(255)}${family}`);
    expect(chunks[1]!.runs.map(run => run.text).join('')).toBe('z');
    f.renderer.dispose();
  });
  it('keeps 100 animation frames free of GPU queries, rasterization and cell uploads', () => {
    const f = fixture(); expect(f.renderer.render({ textRevision: 1, cursor: 0 })).toBe(true); const createCanvasCount = f.raster.canvases;
    const backgroundBuffer = f.r.calls.find(call => call.name === 'drawArraysInstanced' && call.args[4] === 'background')!.buffer;
    const overlayBuffer = f.r.calls.find(call => call.name === 'drawArraysInstanced' && call.args[4] === 'overlay')!.buffer;
    const textBuffer = f.r.calls.find(call => call.name === 'drawArraysInstanced' && call.args[4] === 'text')!.buffer;
    const textWrites = () => f.r.calls.filter(call => call.name === 'bufferSubData' && call.buffer === textBuffer).length;
    const initialWrites = textWrites();
    const initialOverlayWrites = f.r.calls.filter(call => call.name === 'bufferSubData' && call.buffer === overlayBuffer).length;
    const animationBackgroundWrites = () => f.r.calls.filter(call => call.name === 'bufferSubData' && call.buffer === backgroundBuffer);
    const backgroundWritesBefore = animationBackgroundWrites().length;
    const error = vi.spyOn(f.r.gl, 'getError'), parameter = vi.spyOn(f.r.gl, 'getParameter'), texture = vi.spyOn(f.r.gl, 'isTexture');
    error.mockClear(); parameter.mockClear(); texture.mockClear();
    const uploads = f.renderer.atlasStats.uploads, measures = f.raster.measures;
    for (let frame = 0; frame < 100; frame++) {
      expect(f.renderer.render({ textRevision: 1, animated: [{ kind: 'playing', from: 0, to: 3 }] })).toBe(true);
      expect(f.renderer.stats.lastFrameDraws).toBeLessThanOrEqual(4);
    }
    expect(error).not.toHaveBeenCalled(); expect(parameter).not.toHaveBeenCalled(); expect(texture).not.toHaveBeenCalled();
    expect(textWrites()).toBe(initialWrites); expect(f.renderer.stats.bufferUploads).toBe(textWrites());
    expect(f.r.calls.filter(call => call.name === 'bufferSubData' && call.buffer === overlayBuffer).length).toBe(initialOverlayWrites);
    expect(animationBackgroundWrites().length).toBeGreaterThan(backgroundWritesBefore);
    expect(animationBackgroundWrites().slice(backgroundWritesBefore).every(call => Number(call.args[1]) >= 32)).toBe(true);
    expect(f.renderer.atlasStats.uploads).toBe(uploads); expect(f.raster.canvases).toBe(createCanvasCount); expect(f.raster.measures).toBe(measures);
    f.renderer.setDocument('let Ω = 1'); expect(f.renderer.render({ textRevision: 2 })).toBe(true); expect(f.renderer.atlasStats.uploads).toBeGreaterThan(uploads);
    expect(textWrites()).toBe(initialWrites + 1); expect(f.renderer.stats.bufferUploads).toBe(textWrites());
    f.renderer.dispose();
  });
});
