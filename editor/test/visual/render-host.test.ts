// Criterion 11 automated proxy (design 9.2-9.4, 15.1.8): program
// compile/link/draw, ping-pong alternation and feedback, failure recovery,
// the empty program, uniforms by name.

import { describe, expect, it } from 'vitest';

import type { RenderRecord } from '../../src/protocol/types';
import { GlRenderHost, type HostDiagnostic } from '../../src/visual/render-host';
import { RecordingGL, type FakeProgram } from '../support/gl';

/** The golden `osc 20 > rotate 0.5` source (src/tex/tests/goldens.rs). */
const OSC_ROTATE = `#version 300 es
precision highp float;
uniform float time;
uniform vec2 resolution;
out vec4 fragColor;
vec4 tex_osc(vec2 uv, float freq, float sync, float offset) {
  float r = sin((uv.x + offset) * freq + time * sync) * 0.5 + 0.5;
  float g = sin((uv.x + offset) * freq + time * sync + 2.094) * 0.5 + 0.5;
  float b = sin((uv.x + offset) * freq + time * sync + 4.189) * 0.5 + 0.5;
  return vec4(r, g, b, 1.0);
}

vec2 tex_rotate(vec2 uv, float angle, float speed) {
  float a = radians(angle) + time * speed;
  vec2 c = uv - 0.5;
  float s = sin(a);
  float cs = cos(a);
  return vec2(c.x * cs - c.y * s, c.x * s + c.y * cs) + 0.5;
}

void main() {
  vec2 st = gl_FragCoord.xy / resolution.xy;
  fragColor = tex_osc(tex_rotate(st, 0.5, 0.0), 20.0, 0.1, 0.0);
}
`;

const WITH_UNIFORMS = `#version 300 es
precision highp float;
uniform float time;
uniform vec2 resolution;
uniform float u0;
uniform float u1;
out vec4 fragColor;
void main() {
  fragColor = vec4(u0, u1, 0.0, 1.0);
}
`;

const FEEDBACK = `#version 300 es
precision highp float;
uniform float time;
uniform vec2 resolution;
uniform sampler2D u_out0;
out vec4 fragColor;
void main() {
  vec2 st = gl_FragCoord.xy / resolution.xy;
  fragColor = texture(u_out0, st) * 0.9;
}
`;

const program = (out: 0 | 1 | 2 | 3, source: string, uniform_names: string[] = []): RenderRecord => ({
  op: 'program',
  out,
  source,
  uniform_names,
  assets: [],
});

function setup(): { gl: RecordingGL; host: GlRenderHost; diags: HostDiagnostic[] } {
  const gl = new RecordingGL();
  const host = new GlRenderHost(gl.as(), { width: 640, height: 360 });
  const diags: HostDiagnostic[] = [];
  host.onDiagnostic((d) => diags.push(d));
  return { gl, host, diags };
}

describe('GlRenderHost', () => {
  it('compiles, links and draws the osc/rotate program into o0', () => {
    const { gl, host, diags } = setup();
    host.onRecord(program(0, OSC_ROTATE));
    expect(diags).toEqual([]);
    expect(host.hasProgram(0)).toBe(true);
    const frag = gl.named('shaderSource').find((c) => c.args[1] === OSC_ROTATE);
    expect(frag).toBeDefined();
    const linked = gl.named('linkProgram');
    expect(linked).toHaveLength(1);
    const prog = linked[0]?.args[0] as FakeProgram;
    expect(prog.ok).toBe(true);

    const mark = gl.calls.length;
    host.draw(1.5);
    const after = gl.since(mark);
    const draws = after.filter((c) => c.fn === 'drawArrays');
    expect(draws).toHaveLength(1);
    expect(draws[0]?.args.slice(0, 3)).toEqual([gl.TRIANGLES, 0, 3]);
    expect(draws[0]?.args[4]).toBe(prog);
    expect(draws[0]?.args[3]).not.toBeNull();
    expect(after).toContainEqual({ fn: 'uniform1f', args: ['time', 1.5] });
    expect(after).toContainEqual({ fn: 'uniform2f', args: ['resolution', 640, 360] });
    // Outputs without a program do not draw.
    expect(host.hasProgram(1)).toBe(false);
  });

  it('alternates the ping-pong framebuffers and feeds back the previous frame', () => {
    const { gl, host } = setup();
    host.onRecord(program(0, FEEDBACK));
    const targets: unknown[] = [];
    const fedBack: unknown[] = [];
    const latest: unknown[] = [];
    for (let i = 0; i < 4; i++) {
      const mark = gl.calls.length;
      host.draw(i / 60);
      const calls = gl.since(mark);
      targets.push(calls.find((c) => c.fn === 'drawArrays')?.args[3]);
      const set = calls.findIndex((c) => c.fn === 'uniform1i' && c.args[0] === 'u_out0');
      expect(set).toBeGreaterThan(0);
      const bound = calls.slice(0, set).reverse().find((c) => c.fn === 'bindTexture');
      fedBack.push(bound?.args[1]);
      latest.push(host.texture(0));
    }
    const [a, b] = targets;
    expect(a).toBeDefined();
    expect(b).toBeDefined();
    expect(a).not.toBe(b);
    expect(targets).toEqual([a, b, a, b]);
    // Each frame renders into the other buffer; u_out0 samples the previous frame.
    expect(latest[0]).toBe(gl.textureOf(a));
    expect(latest[1]).toBe(gl.textureOf(b));
    expect(fedBack[1]).toBe(latest[0]);
    expect(fedBack[2]).toBe(latest[1]);
    expect(fedBack[3]).toBe(latest[2]);
    expect(fedBack[1]).not.toBe(gl.textureOf(targets[1]));
  });

  it('keeps the previous program drawing when a compile fails, with a shader-compile diagnostic', () => {
    const { gl, host, diags } = setup();
    host.onRecord(program(0, WITH_UNIFORMS, ['u0', 'u1']));
    host.onRecord({ op: 'uniforms', out: 0, values: [0.25, 3] });
    const good = gl.named('linkProgram')[0]?.args[0] as FakeProgram;

    gl.failNextCompile('ERROR: 0:7: syntax error');
    host.onRecord(program(0, 'broken source', ['u0']));
    expect(diags).toEqual([{ code: 'shader-compile', out: 0, message: 'ERROR: 0:7: syntax error' }]);
    expect(good.deleted).toBe(false);
    // Values for the failed program do not reach the previous one.
    host.onRecord({ op: 'uniforms', out: 0, values: [9] });

    const mark = gl.calls.length;
    host.draw(2);
    const after = gl.since(mark);
    expect(after.find((c) => c.fn === 'drawArrays')?.args[4]).toBe(good);
    expect(after).toContainEqual({ fn: 'uniform1f', args: ['u0', 0.25] });
    expect(after).not.toContainEqual({ fn: 'uniform1f', args: ['u0', 9] });
  });

  it('keeps the previous program when linking fails', () => {
    const { gl, host, diags } = setup();
    host.onRecord(program(1, OSC_ROTATE));
    const good = gl.named('linkProgram')[0]?.args[0] as FakeProgram;
    gl.failNextLink('link: varying mismatch');
    host.onRecord(program(1, WITH_UNIFORMS, ['u0', 'u1']));
    expect(diags).toEqual([{ code: 'shader-compile', out: 1, message: 'link: varying mismatch' }]);
    const mark = gl.calls.length;
    host.draw(0);
    expect(gl.since(mark).find((c) => c.fn === 'drawArrays')?.args[4]).toBe(good);
    // A later good program recovers.
    host.onRecord(program(1, WITH_UNIFORMS, ['u0', 'u1']));
    expect(good.deleted).toBe(true);
  });

  it('clears the output to black on the empty program', () => {
    const { gl, host } = setup();
    host.onRecord(program(2, OSC_ROTATE));
    host.draw(0);
    const prog = gl.named('linkProgram')[0]?.args[0] as FakeProgram;
    const accepted: number[] = [];
    host.onProgram((o) => accepted.push(o));
    const mark = gl.calls.length;
    host.onRecord(program(2, ''));
    expect(accepted).toEqual([2]);
    expect(prog.deleted).toBe(true);
    expect(host.hasProgram(2)).toBe(false);
    const after = gl.since(mark);
    expect(after).toContainEqual({ fn: 'clearColor', args: [0, 0, 0, 1] });
    const cleared = new Set(after.filter((c) => c.fn === 'clear').map((c) => c.args[1]));
    expect(cleared.size).toBe(2);
    const drawMark = gl.calls.length;
    host.draw(1);
    expect(gl.since(drawMark).filter((c) => c.fn === 'drawArrays')).toHaveLength(0);
  });

  it('sets uniforms values by name on the next draw', () => {
    const { gl, host } = setup();
    host.onRecord(program(3, WITH_UNIFORMS, ['u0', 'u1']));
    host.onRecord({ op: 'uniforms', out: 3, values: [0.5, 0.75] });
    let mark = gl.calls.length;
    host.draw(0);
    let after = gl.since(mark);
    expect(after).toContainEqual({ fn: 'uniform1f', args: ['u0', 0.5] });
    expect(after).toContainEqual({ fn: 'uniform1f', args: ['u1', 0.75] });
    host.onRecord({ op: 'uniforms', out: 3, values: [0.1, 0.2] });
    mark = gl.calls.length;
    host.draw(1 / 60);
    after = gl.since(mark);
    expect(after).toContainEqual({ fn: 'uniform1f', args: ['u0', 0.1] });
    expect(after).toContainEqual({ fn: 'uniform1f', args: ['u1', 0.2] });
  });

  it('draws nothing and deletes its resources after dispose', () => {
    const { gl, host } = setup();
    host.onRecord(program(0, OSC_ROTATE));
    host.dispose();
    const mark = gl.calls.length;
    host.draw(0);
    host.onRecord(program(0, OSC_ROTATE));
    expect(gl.since(mark)).toEqual([]);
    expect(gl.named('deleteFramebuffer')).toHaveLength(8);
  });
});
