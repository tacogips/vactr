// `text "hello"`: the TextAsset is rasterized with canvas 2D, uploaded and
// bound to the declared `u_text<id>` sampler (design 9.2, 15.1.8).

import { afterEach, describe, expect, it } from 'vitest';

import { GlRenderHost, type HostDiagnostic } from '../../src/visual/render-host';
import { rasterizeText, TEXT_ASSET_FONT } from '../../src/visual/text-asset';
import { installCanvasFakes, type CanvasFakes } from '../support/canvas';
import { RecordingGL } from '../support/gl';

const TEXT_SOURCE = `#version 300 es
precision highp float;
uniform float time;
uniform vec2 resolution;
uniform sampler2D u_text0;
out vec4 fragColor;
void main() {
  vec2 st = gl_FragCoord.xy / resolution.xy;
  fragColor = texture(u_text0, st);
}
`;

let fakes: CanvasFakes | null = null;

afterEach(() => {
  fakes?.restore();
  fakes = null;
});

describe('text assets', () => {
  it('rasterizes the text white on transparent with the fixed font', () => {
    fakes = installCanvasFakes();
    const canvas = rasterizeText(() => document.createElement('canvas'), 'hello');
    expect(canvas).not.toBeNull();
    const ctx = fakes.ctx(canvas as HTMLCanvasElement);
    const text = ctx.named('fillText');
    expect(text).toHaveLength(1);
    expect(text[0]?.args[0]).toBe('hello');
    expect(text[0]?.args).toContain(TEXT_ASSET_FONT);
    expect(text[0]?.args).toContain('#ffffff');
    expect(ctx.named('clearRect')).toHaveLength(1);
  });

  it('uploads the rasterized text and binds it at u_text<id>', () => {
    fakes = installCanvasFakes();
    const gl = new RecordingGL();
    const host = new GlRenderHost(gl.as(), { width: 640, height: 360 });
    const diags: HostDiagnostic[] = [];
    host.onDiagnostic((d) => diags.push(d));
    host.onRecord({
      op: 'program',
      out: 1,
      source: TEXT_SOURCE,
      uniform_names: [],
      assets: [{ id: 0, text: 'hello' }],
    });
    expect(diags).toEqual([]);
    const canvas = fakes.canvases2d[0];
    expect(canvas).toBeDefined();
    expect(fakes.ctx(canvas as HTMLCanvasElement).named('fillText')[0]?.args[0]).toBe('hello');

    const upload = gl.named('texImage2D').find((c) => c.args[5] === canvas);
    expect(upload).toBeDefined();
    const uploadAt = gl.calls.indexOf(upload as (typeof gl.calls)[number]);
    const texture = gl.calls
      .slice(0, uploadAt)
      .reverse()
      .find((c) => c.fn === 'bindTexture')?.args[1];
    expect(texture).toBeTruthy();

    const mark = gl.calls.length;
    host.draw(0);
    const after = gl.since(mark);
    const set = after.findIndex((c) => c.fn === 'uniform1i' && c.args[0] === 'u_text0');
    expect(set).toBeGreaterThan(0);
    const bound = after.slice(0, set).reverse().find((c) => c.fn === 'bindTexture');
    expect(bound?.args[1]).toBe(texture);
  });

  it('reports a host diagnostic when there is no 2D context', () => {
    fakes = installCanvasFakes({ no2d: true });
    const gl = new RecordingGL();
    const host = new GlRenderHost(gl.as(), { width: 64, height: 64 });
    const diags: HostDiagnostic[] = [];
    host.onDiagnostic((d) => diags.push(d));
    host.onRecord({ op: 'program', out: 2, source: TEXT_SOURCE, uniform_names: [], assets: [{ id: 0, text: 'x' }] });
    expect(diags).toEqual([{ code: 'text-asset', out: 2, message: '2D canvas not available for text asset 0' }]);
    // The program still draws; the sampler reads an unbound texture.
    expect(host.hasProgram(2)).toBe(true);
  });
});
