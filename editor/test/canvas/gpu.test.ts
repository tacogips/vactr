import { describe, expect, it } from 'vitest';
import { TextLayout, type LayoutFont } from '../../src/code/layout';
import { GlyphAtlas, type CanvasFactory } from '../../src/code/atlas';
import { CanvasRenderer } from '../../src/code/renderer';
import { MiB, ResourceLedger, RESOURCE_LIMITS, effectiveSize } from '../../src/code/resources';
import type { CodeAnnotation } from '../../src/app/apis';

const font: LayoutFont = { font: '16px test-mono', fallback: 'test-ja', lineHeight: 20, baseline: 16 };
function width(text: string): number {
  let result = 0;
  for (const { segment } of new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(text)) result += /[日語本]/u.test(segment) ? 16 : /\p{Extended_Pictographic}/u.test(segment) ? 16 : 8;
  return result - (text.match(/ffi/g)?.length ?? 0) * 6;
}
function layout(text = '', cacheBytes?: number): TextLayout {
  const l = new TextLayout({ font: '', measureText: text => ({ width: width(text) }) }, font, cacheBytes); l.setDocument(text); return l;
}
const view = { width: 320, height: 100, scrollLeft: 0, scrollTop: 0, left: 10, top: 20, gutter: 48 };
function rasterizer(): { createCanvas: CanvasFactory; text: { text: string; x: number; y: number; font: string; color: string }[]; crops: number[][] } {
  const text: { text: string; x: number; y: number; font: string; color: string }[] = []; const crops: number[][] = [];
  return { text, crops, createCanvas: () => {
    const canvas = document.createElement('canvas');
    const ctx = { font: '', fillStyle: '', textBaseline: '', setTransform() {}, clearRect() {}, save() {}, restore() {}, beginPath() {}, clip() {}, drawImage() {},
      rect(...args: number[]) { crops.push(args); },
      measureText(t: string) { return { width: width(t) }; },
      fillText(t: string, x: number, y: number) { text.push({ text: t, x, y, font: this.font, color: this.fillStyle }); },
    };
    Object.defineProperty(canvas, 'getContext', { value: () => ctx }); return canvas;
  } };
}
function recordingGL(maxTexture = 1024) {
  let next = 1; let error = 0; let failUpload = false;
  const live = new Map<object, string>(); const deleted: string[] = []; const calls: { name: string; args: unknown[] }[] = [];
  let rect: number[] = []; let color: number[] = []; let bound: object | null = null;
  const draws: { rect: number[]; color: number[]; texture: object | null }[] = [];
  const gl: Record<string, unknown> = {};
  ['MAX_TEXTURE_SIZE', 'TEXTURE_2D', 'TEXTURE_MIN_FILTER', 'TEXTURE_MAG_FILTER', 'LINEAR', 'NEAREST', 'TEXTURE_WRAP_S', 'TEXTURE_WRAP_T', 'CLAMP_TO_EDGE', 'UNPACK_PREMULTIPLY_ALPHA_WEBGL', 'RGBA', 'UNSIGNED_BYTE', 'VERTEX_SHADER', 'FRAGMENT_SHADER', 'COMPILE_STATUS', 'LINK_STATUS', 'ARRAY_BUFFER', 'STATIC_DRAW', 'FLOAT', 'FRAMEBUFFER', 'SCISSOR_TEST', 'BLEND', 'ONE', 'ONE_MINUS_SRC_ALPHA', 'COLOR_BUFFER_BIT', 'TEXTURE0', 'TRIANGLES'].forEach((name, i) => gl[name] = i + 1);
  gl.NO_ERROR = 0;
  for (const kind of ['Texture', 'Shader', 'Program', 'Buffer', 'VertexArray']) {
    gl[`create${kind}`] = () => { const object = { id: next++ }; live.set(object, kind); return object; };
    gl[`delete${kind}`] = (obj: object) => { expect(live.get(obj)).toBe(kind); live.delete(obj); deleted.push(kind); };
  }
  for (const name of ['texParameteri', 'pixelStorei', 'shaderSource', 'compileShader', 'attachShader', 'linkProgram', 'bindVertexArray', 'bindBuffer', 'bufferData', 'enableVertexAttribArray', 'vertexAttribPointer', 'bindFramebuffer', 'useProgram', 'viewport', 'disable', 'enable', 'blendFunc', 'clearColor', 'clear', 'uniform2f', 'uniform1i', 'activeTexture', 'scissor']) gl[name] = (...args: unknown[]) => calls.push({ name, args });
  gl.getParameter = () => maxTexture; gl.getShaderParameter = () => true; gl.getProgramParameter = () => true;
  gl.getShaderInfoLog = () => ''; gl.getProgramInfoLog = () => ''; gl.getAttribLocation = () => 0; gl.getUniformLocation = (_p: object, name: string) => ({ name });
  gl.isContextLost = () => false;
  gl.bindTexture = (_target: number, texture: object | null) => { bound = texture; };
  gl.uniform4f = (loc: { name: string }, ...args: number[]) => { if (loc.name === 'u_rect') rect = args; if (loc.name === 'u_color') color = args; };
  gl.drawArrays = () => draws.push({ rect: [...rect], color: [...color], texture: bound });
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
  it('uses whole-run shaping and measured Japanese/ligature caret advances', () => {
    const l = layout('ffi日本'); const line = l.shape(0);
    expect(line.runs.map(r => r.text)).toEqual(['ffi日本']); expect(line.width).toBe(50);
    expect(l.coordsAtPos(3, view)?.left).toBe(76); expect(l.coordsAtPos(4, view)?.left).toBe(92);
    expect(l.posAtCoords({ x: 94, y: 21 }, view)).toBe(4);
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
  it('crops the whole run and masks syntax without reshaping substrings', () => {
    const r = recordingGL(64); const raster = rasterizer(); const b = new ResourceLedger(); const a = new GlyphAtlas(r.gl, b, raster.createCanvas);
    const run = { text: 'ffi日本', from: 0, to: 5, x: 0, width: 50 };
    const tile = a.tile({ run, font, dpr: 1, x: 32, width: 18, styles: [{ from: 3, to: 5, color: '#ff0000' }] });
    expect(tile.width).toBe(18); expect(raster.text.map(t => t.text)).toEqual(['ffi日本', 'ffi日本']); expect(raster.text[0]!.x).toBe(-32);
    expect(raster.crops).toContainEqual([-14, 0, 32, 20]);
    a.dispose(); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0);
  });
  it('caches by text/font/fallback/DPR/style and deletes evicted tiles', () => {
    const r = recordingGL(); const b = new ResourceLedger(17000); const a = new GlyphAtlas(r.gl, b, rasterizer().createCanvas);
    const run = { text: 'abc', from: 0, to: 3, x: 0, width: 24 };
    const req = { run, font, dpr: 1, x: 0, width: 24 };
    const first = a.tile(req); expect(a.tile(req).texture).toBe(first.texture); expect(a.stats.uploads).toBe(1);
    a.tile({ ...req, font: { ...font, fallback: 'other' } });
    a.tile({ ...req, color: '#ff0000' }); a.tile({ ...req, dpr: 2 });
    expect(a.stats.evictions).toBeGreaterThan(0); expect(b.usedBytes).toBeLessThanOrEqual(17000);
    a.dispose(); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0);
  });
  it('rolls back reservations and textures on upload failure', () => {
    const r = recordingGL(); const b = new ResourceLedger(); const a = new GlyphAtlas(r.gl, b, rasterizer().createCanvas); r.failNextUpload();
    expect(() => a.tile({ run: { text: 'a', from: 0, to: 1, x: 0, width: 8 }, font, dpr: 1, x: 0, width: 8 })).toThrow('refused');
    expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0); expect(a.size).toBe(0);
  });
});

describe('GPU code compositor', () => {
  function fixture(text = 'let x = 1\n日本') {
    const r = recordingGL(); const raster = rasterizer(); const b = new ResourceLedger(); const c = document.createElement('canvas'); const l = layout(text);
    const renderer = new CanvasRenderer(c, l, { gl: r.gl, ledger: b, createCanvas: raster.createCanvas }); renderer.setViewport(view, 1);
    return { r, raster, b, c, l, renderer };
  }
  it('draws all required feedback in order on GPU and clips source against gutter', () => {
    const f = fixture(); const source = document.createElement('canvas'); source.width = 100; source.height = 100; f.renderer.setBackground(source, 1);
    const annotations: CodeAnnotation[] = [
      { kind: 'selection', from: 0, to: 3 }, { kind: 'playing', from: 4, to: 5 }, { kind: 'eval', from: 0, to: 9 },
      { kind: 'syntax', from: 0, to: 3, className: 'vact-tok-head' }, { kind: 'diagnostic', from: 4, to: 5 },
      { kind: 'composition', from: 6, to: 9 }, { kind: 'binding', from: 4, to: 5, label: 'CC 1' },
    ];
    expect(f.renderer.render({ annotations, cursor: 5, handles: [{ pos: 0 }, { pos: 9, end: true }] })).toBe(true);
    const colors = f.r.draws.map(d => d.color);
    expect(colors.slice(0, 5)).toEqual([[1, 1, 1, 1], [0.04, 0.05, 0.07, 0.85], [0.12, 0.22, 0.32, 0.55], [0.35, 0.29, 0.13, 0.5], [0.2, 0.32, 0.36, 0.5]]);
    const diagnostic = colors.findIndex(c => c[0] === 0.75); const cursor = colors.findIndex(c => c[0] === 0.9); const badge = colors.findIndex(c => c[0] === 0.15);
    expect(diagnostic).toBeGreaterThan(4); expect(cursor).toBeGreaterThan(diagnostic); expect(badge).toBeGreaterThan(cursor);
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
  it('copies changed background once per frame into a reusable texture', () => {
    const f = fixture(); const source = document.createElement('canvas'); source.width = 100; source.height = 100;
    f.renderer.setBackground(source, 1); f.renderer.render(); const textures = [...f.r.live.values()].filter(v => v === 'Texture').length;
    f.renderer.render(); expect(f.renderer.stats.backgroundUploads).toBe(1);
    f.renderer.setBackground(source, 2); f.renderer.render(); expect(f.renderer.stats.backgroundUploads).toBe(2);
    expect([...f.r.live.values()].filter(v => v === 'Texture')).toHaveLength(textures);
    expect(f.r.calls.filter(c => c.name === 'texSubImage2D')).toHaveLength(1);
    const replacement = document.createElement('canvas'); replacement.width = 100; replacement.height = 100;
    f.renderer.setBackground(replacement, 2); f.renderer.render(); expect(f.renderer.stats.backgroundUploads).toBe(3);
    f.renderer.setBackground(null); f.renderer.dispose(); expect(f.b.usedBytes).toBe(0);
  });
  it('restores CPU source after context loss with fresh resources and empty targets', () => {
    const f = fixture(); f.renderer.render(); f.r.lose();
    const lost = new Event('webglcontextlost', { cancelable: true }); f.c.dispatchEvent(lost);
    expect(lost.defaultPrevented).toBe(true); expect(f.b.usedBytes).toBe(0); expect(f.renderer.render()).toBe(false); expect(f.renderer.status.saveText()).toBe('let x = 1\n日本');
    f.renderer.setDocument('retained edit'); f.c.dispatchEvent(new Event('webglcontextrestored')); expect(f.renderer.render()).toBe(true); expect(f.renderer.status.saveText()).toBe('retained edit');
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
  it('large backgrounds downsample within staging and aggregate pixel caps', () => {
    const f = fixture(); const source = document.createElement('canvas'); source.width = 3840; source.height = 2160;
    f.renderer.setBackground(source, 1); expect(f.renderer.render()).toBe(true);
    expect(f.b.counters.byKind.geometry).toBeLessThanOrEqual(RESOURCE_LIMITS.geometry); expect(f.b.counters.pixels).toBeLessThanOrEqual(4_000_000);
    const uploads = f.r.calls.filter(c => c.name === 'texImage2D');
    const copied = uploads.find(c => c.args.length === 6 && (c.args[5] as HTMLCanvasElement).width > 0);
    // Copy staging canvases are explicitly cleared after upload; retained targets stay accounted.
    expect(copied).toBeUndefined(); expect(f.renderer.stats.backgroundUploads).toBe(1); f.renderer.dispose(); expect(f.b.usedBytes).toBe(0);
  });
  it('reuses a near-staging-cap backdrop after text metadata has grown', () => {
    const r = recordingGL(4096); const raster = rasterizer(); const b = new ResourceLedger(); const c = document.createElement('canvas');
    const text = Array.from({ length: 40 }, (_, i) => `line ${i}`).join('\n');
    const renderer = new CanvasRenderer(c, layout(text), { gl: r.gl, ledger: b, createCanvas: raster.createCanvas });
    renderer.setViewport({ width: 640, height: 800, scrollLeft: 0, scrollTop: 0 });
    const source = document.createElement('canvas'); source.width = 3840; source.height = 2160;
    renderer.setBackground(source, 1); expect(renderer.render()).toBe(true);
    renderer.setBackground(source, 2); expect(renderer.render()).toBe(true); expect(renderer.stats.backgroundUploads).toBe(2);
    expect(b.counters.byKind.geometry).toBeLessThanOrEqual(RESOURCE_LIMITS.geometry); renderer.dispose(); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0);
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
    expect(renderer.render()).toBe(true); expect(renderer.atlasStats.evictions).toBeGreaterThan(0); expect(raster.text.filter(t => t.text.includes(':'))).toHaveLength(50);
    expect(b.usedBytes).toBeLessThanOrEqual(2_100_000); renderer.dispose(); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0);
  });
  it('releases initialization resources when shader or object setup fails', () => {
    const r = recordingGL(); Object.assign(r.gl, { getShaderParameter: () => false }); const b = new ResourceLedger();
    const renderer = new CanvasRenderer(document.createElement('canvas'), layout('safe'), { gl: r.gl, ledger: b });
    expect(renderer.status.kind).toBe('unavailable'); expect(renderer.status.saveText()).toBe('safe'); expect(b.usedBytes).toBe(0); expect(r.live.size).toBe(0); renderer.dispose();
  });
});
