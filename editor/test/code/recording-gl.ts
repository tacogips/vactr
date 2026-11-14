import { expect } from 'vitest';
import type { CanvasFactory } from 'emaki/engine';

export function width(text: string): number {
  let result = 0;
  for (const { segment } of new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(text)) result += /[日語本]/u.test(segment) ? 16 : /\p{Extended_Pictographic}/u.test(segment) ? 16 : 8;
  return result - (text.match(/ffi/g)?.length ?? 0) * 6;
}

export function rasterizer(): { createCanvas: CanvasFactory; text: { text: string; x: number; y: number; font: string; color: string }[]; crops: number[][]; drawImages: number; canvases: number; measures: number } {
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

export function recordingGL(maxTexture = 1024) {
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
