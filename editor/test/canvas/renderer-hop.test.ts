import { describe, expect, it } from 'vitest';
import { CanvasRenderer } from '../../src/code/renderer';
import { TextLayout, type LayoutFont } from 'emaki/engine';
import { ResourceLedger } from 'emaki/engine';
import type { CanvasFactory } from 'emaki/engine';

const font: LayoutFont = { font: '16px test-mono', lineHeight: 20, baseline: 16 };
const view = { width: 320, height: 100, scrollLeft: 0, scrollTop: 0, left: 10, top: 20, gutter: 48 };

function fixture(text = 'abc') {
  let next = 0;
  const buffers: object[] = [];
  const uploads: object[] = [];
  const draws: Array<{ layer: string; commands: Array<{ rect: number[]; color: number[] }> }> = [];
  const instances = new Map<object, Uint8Array>();
  const constants = ['MAX_TEXTURE_SIZE', 'TEXTURE_2D', 'TEXTURE_MIN_FILTER', 'TEXTURE_MAG_FILTER', 'NEAREST', 'RGBA', 'RG', 'RG32F', 'FLOAT', 'UNSIGNED_SHORT', 'UNSIGNED_BYTE', 'VERTEX_SHADER', 'FRAGMENT_SHADER', 'COMPILE_STATUS', 'LINK_STATUS', 'ARRAY_BUFFER', 'STATIC_DRAW', 'DYNAMIC_DRAW', 'FRAMEBUFFER', 'BLEND', 'ONE', 'ONE_MINUS_SRC_ALPHA', 'COLOR_BUFFER_BIT', 'TEXTURE0', 'TEXTURE1', 'TRIANGLES'];
  const values = Object.fromEntries(constants.map((name, index) => [name, index + 1])) as Record<string, number>;
  const gl = new Proxy({ ...values, NO_ERROR: 0 } as Record<string, unknown>, {
    get(target, name: string) {
      if (name in target) return target[name];
      if (name === 'getParameter') return () => 1024;
      if (name === 'getShaderParameter' || name === 'getProgramParameter') return () => true;
      if (name === 'getShaderInfoLog' || name === 'getProgramInfoLog') return () => '';
      if (name === 'getAttribLocation') return () => 0;
      if (name === 'getUniformLocation') return (_program: object, uniform: string) => ({ name: uniform });
      if (name === 'getError') return () => 0;
      if (name === 'isContextLost') return () => false;
      if (name.startsWith('create')) return () => { const object = { id: ++next }; if (name === 'createBuffer') buffers.push(object); return object; };
      if (name === 'bufferSubData') return (_target: number, offset: number, data: Uint8Array) => {
        uploads.push(boundBuffer!);
        const previous = boundBuffer ? instances.get(boundBuffer) ?? new Uint8Array() : new Uint8Array();
        const bytes = new Uint8Array(Math.max(previous.length, offset + data.byteLength));
        bytes.set(previous); bytes.set(new Uint8Array(data.buffer, data.byteOffset, data.byteLength), offset);
        if (boundBuffer) instances.set(boundBuffer, bytes);
      };
      if (name === 'bindBuffer') return (_target: number, buffer: object) => { boundBuffer = buffer; };
      if (name === 'drawArraysInstanced') return (_mode: number, _first: number, _vertices: number, count: number) => {
        const layer = ['background', 'text', 'overlay', 'overlayText'][buffers.indexOf(boundBuffer!) - 1] ?? 'unknown';
        const bytes = boundBuffer ? instances.get(boundBuffer) ?? new Uint8Array() : new Uint8Array();
        const data = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
        const commands = Array.from({ length: count }, (_, index) => {
          const offset = index * 32;
          return { rect: [0, 4, 8, 12].map(at => data.getFloat32(offset + at, true)), color: [24, 25, 26, 27].map(at => data.getUint8(offset + at) / 255) };
        });
        draws.push({ layer, commands });
      };
      return () => undefined;
    },
  }) as unknown as WebGL2RenderingContext;
  let boundBuffer: object | null = null;
  const createCanvas: CanvasFactory = () => {
    const canvas = document.createElement('canvas');
    const context = { font: '', fillStyle: '', textBaseline: '', setTransform() {}, clearRect() {}, save() {}, restore() {}, beginPath() {}, clip() {}, drawImage() {}, rect() {}, getImageData: (_x: number, _y: number, width: number, height: number) => ({ data: new Uint8ClampedArray(width * height * 4) }), measureText: (value: string) => ({ width: value.length * 8 }), fillText() {} };
    Object.defineProperty(canvas, 'getContext', { value: () => context });
    return canvas;
  };
  const layout = new TextLayout({ font: '', measureText: value => ({ width: value.length * 8 }) }, font);
  layout.setDocument(text);
  const renderer = new CanvasRenderer(document.createElement('canvas'), layout, { gl, ledger: new ResourceLedger(), createCanvas });
  renderer.setViewport(view, 1);
  return { renderer, layout, uploads, draws, buffers };
}

function overlayUploadCount(f: ReturnType<typeof fixture>): number { return f.uploads.filter(buffer => buffer === f.buffers[3] || buffer === f.buffers[4]).length; }

describe('CanvasRenderer hop labels', () => {
  it('keeps commands identical when labels are absent or empty', () => {
    const without = fixture(), empty = fixture();
    expect(without.renderer.render()).toBe(true);
    expect(empty.renderer.render({ hopLabels: [], hopRevision: 1 })).toBe(true);
    expect(empty.draws).toEqual(without.draws);
    without.renderer.dispose(); empty.renderer.dispose();
  });

  it('draws a visible hop label in the overlay', () => {
    const f = fixture();
    expect(f.renderer.render({ hopLabels: [{ pos: 0, text: 'f' }], hopRevision: 1 })).toBe(true);
    expect(overlayUploadCount(f)).toBeGreaterThan(0);
    expect(f.draws.some(draw => draw.layer === 'overlay' && draw.commands.length > 0)).toBe(true);
    expect(f.draws.some(draw => draw.layer === 'overlayText' && draw.commands.length > 0)).toBe(true);
    f.renderer.dispose();
  });

  it('rebuilds labels when revision changes and reuses the same revision', () => {
    const f = fixture(); const labels = [{ pos: 0, text: 'f' }];
    expect(f.renderer.render({ hopLabels: labels, hopRevision: 1 })).toBe(true);
    const first = overlayUploadCount(f);
    expect(f.renderer.render({ hopLabels: labels, hopRevision: 1 })).toBe(true);
    expect(overlayUploadCount(f)).toBe(first);
    expect(f.renderer.render({ hopLabels: labels, hopRevision: 2 })).toBe(true);
    expect(overlayUploadCount(f)).toBeGreaterThan(first);
    f.renderer.dispose();
  });

  it('skips labels whose positions are outside the view', () => {
    const f = fixture();
    f.layout.coordsAtPos = () => null;
    expect(f.renderer.render()).toBe(true);
    const before = overlayUploadCount(f);
    expect(f.renderer.render({ hopLabels: [{ pos: 99, text: 'f' }], hopRevision: 1 })).toBe(true);
    expect(overlayUploadCount(f)).toBe(before);
    f.renderer.dispose();
  });
});
