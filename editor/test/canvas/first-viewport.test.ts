// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import type { EditorDeps } from '../../src/app/deps';
import { buildLayout } from '../../src/app/layout';
import { mount } from '../../src/code/mount';
import { CanvasRenderer } from '../../src/code/renderer';
import { effectiveSize, RESOURCE_LIMITS } from '../../src/code/resources';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { MemoryFiles } from '../../src/platform/files';
import { MockClock } from '../support/clock';
import { installCanvasFakes, type CanvasFakes } from '../support/canvas';
import { RecordingTransport } from '../support/recording';

interface RafHost {
  requestAnimationFrame(cb: FrameRequestCallback): number;
  cancelAnimationFrame(id: number): void;
  devicePixelRatio: number;
  innerHeight: number;
  callbacks: Map<number, FrameRequestCallback>;
  step(time: number): void;
}

function rafHost(): RafHost {
  let next = 1;
  const callbacks = new Map<number, FrameRequestCallback>();
  return { callbacks, devicePixelRatio: 2, innerHeight: 900,
    requestAnimationFrame(cb) { const id = next++; callbacks.set(id, cb); return id; },
    cancelAnimationFrame(id) { callbacks.delete(id); },
    step(time) { const row = callbacks.entries().next().value as [number, FrameRequestCallback] | undefined; if (!row) return; callbacks.delete(row[0]); row[1](time); },
  };
}

function rendererGL(): WebGL2RenderingContext {
  let next = 1;
  const gl: Record<string, unknown> = {};
  const constants = ['MAX_TEXTURE_SIZE', 'TEXTURE_2D', 'TEXTURE_MIN_FILTER', 'TEXTURE_MAG_FILTER', 'LINEAR', 'NEAREST',
    'TEXTURE_WRAP_S', 'TEXTURE_WRAP_T', 'CLAMP_TO_EDGE', 'UNPACK_PREMULTIPLY_ALPHA_WEBGL', 'RGBA', 'UNSIGNED_BYTE',
    'VERTEX_SHADER', 'FRAGMENT_SHADER', 'COMPILE_STATUS', 'LINK_STATUS', 'ARRAY_BUFFER', 'STATIC_DRAW', 'FLOAT',
    'FRAMEBUFFER', 'SCISSOR_TEST', 'BLEND', 'ONE', 'ONE_MINUS_SRC_ALPHA', 'COLOR_BUFFER_BIT', 'TEXTURE0', 'TRIANGLES'];
  constants.forEach((name, index) => { gl[name] = index + 1; }); gl.NO_ERROR = 0;
  for (const kind of ['Texture', 'Shader', 'Program', 'Buffer', 'VertexArray']) {
    gl[`create${kind}`] = () => ({ id: next++ }); gl[`delete${kind}`] = () => {};
  }
  for (const name of ['texParameteri', 'pixelStorei', 'shaderSource', 'compileShader', 'attachShader', 'linkProgram',
    'bindVertexArray', 'bindBuffer', 'bufferData', 'enableVertexAttribArray', 'vertexAttribPointer', 'bindFramebuffer',
    'useProgram', 'viewport', 'disable', 'enable', 'blendFunc', 'clearColor', 'clear', 'uniform2f', 'uniform1i',
    'activeTexture', 'scissor', 'drawArrays', 'bindTexture', 'texImage2D', 'texSubImage2D', 'uniform4f']) gl[name] = () => {};
  gl.getParameter = () => 4096; gl.getShaderParameter = () => true; gl.getProgramParameter = () => true;
  gl.getShaderInfoLog = () => ''; gl.getProgramInfoLog = () => ''; gl.getAttribLocation = () => 0;
  gl.getUniformLocation = (_program: unknown, name: string) => ({ name }); gl.getError = () => gl.NO_ERROR;
  gl.isContextLost = () => false; gl.isTexture = () => true;
  return gl as unknown as WebGL2RenderingContext;
}

interface Rig { root: HTMLElement; deps: EditorDeps; mounted: ReturnType<typeof mount>; transport: RecordingTransport; fakes: CanvasFakes; host: RafHost }
const rigs: Rig[] = [];

function setup(): Rig {
  const gl = rendererGL();
  const fakes = installCanvasFakes({ webgl2: () => gl });
  const context = fakes.ctx(document.createElement('canvas')) as unknown as Record<string, unknown>;
  Object.assign(Object.getPrototypeOf(context) as object, {
    measureText: (text: string) => ({ width: text.length * 8 }),
    setTransform() {}, save() {}, restore() {}, rect() {}, clip() {},
  });
  const root = document.createElement('div'); document.body.append(root);
  buildLayout(root); const store = new Store(); const transport = new RecordingTransport();
  const deps: EditorDeps = { client: new Client(transport, { store }), store, clock: new MockClock(), tier: 'browser', files: new MemoryFiles() };
  const host = rafHost();
  const rect = () => ({ width: 767, height: 858, x: 0, y: 0, left: 0, top: 0, right: 767, bottom: 858, toJSON: () => ({}) });
  const originalAppend = HTMLElement.prototype.appendChild;
  HTMLElement.prototype.appendChild = function <T extends Node>(node: T): T {
    const appended = originalAppend.call(this, node) as T;
    if (node instanceof HTMLElement && node.classList.contains('vact-code')) node.getBoundingClientRect = rect;
    return appended;
  };
  let mounted: ReturnType<typeof mount>;
  try { mounted = mount(root, deps, { frameHost: host, gl }); }
  finally { HTMLElement.prototype.appendChild = originalAppend; }
  const rig = { root, deps, mounted, transport, fakes, host };
  rigs.push(rig); return rig;
}

afterEach(() => {
  for (const rig of rigs.splice(0)) { rig.mounted.dispose(); rig.deps.client.close(); rig.deps.store.dispose(); rig.root.remove(); rig.fakes.restore(); }
  document.body.replaceChildren();
});

describe('first canvas viewport', () => {
  it('sizes the mounted canvas from the first visible viewport tick', () => {
    const setViewport = vi.spyOn(CanvasRenderer.prototype, 'setViewport');
    const rig = setup();
    rig.host.step(16);
    expect(setViewport).toHaveBeenCalledWith(expect.objectContaining({ width: 767, height: 858 }), 2);
    const canvas = rig.root.querySelector<HTMLCanvasElement>('canvas.vact-code-canvas')!;
    const expected = effectiveSize(767, 858, 2, 4096, RESOURCE_LIMITS.canvasPixels / 2);
    expect(canvas.width).toBe(expected.width);
    expect(canvas.height).toBe(expected.height);
    expect(canvas.width).toBeGreaterThan(1);
    expect(canvas.height).toBeGreaterThan(1);
  });
});
