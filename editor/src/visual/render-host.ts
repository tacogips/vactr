// The WebGL2 RenderHost (design 9.2-9.4, 15.1.8). It consumes the `0x72`
// render records of the wasm session half:
//
// - `program`: the fragment source is compiled against a full-screen
//   triangle and linked. On success it replaces the output's program; a
//   compile or link FAILURE keeps the previous program drawing and reports
//   a `shader-compile` host diagnostic. The empty source (hush/stop)
//   releases the program and clears the output to black.
// - `uniforms`: per-frame values in the program's `uniform_names` order.
//
// Each output o0..o3 owns two framebuffer/texture pairs (ping-pong): a
// draw renders into the back buffer while `u_out<k>` samples output k's
// PREVIOUS frame, then the pair swaps. The host only receives render-safe
// data (plain strings and numbers, invariant 17.2) and evaluates nothing.

import type { OutputIndex, RenderRecord } from '../protocol/types';
import { uploadTextAsset, type CanvasFactory } from './text-asset';

export const OUTPUTS: readonly OutputIndex[] = [0, 1, 2, 3];

/** The full-screen triangle; no vertex attributes are needed. */
export const VERTEX_SOURCE = `#version 300 es
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
`;

export interface RenderSize {
  width: number;
  height: number;
}

export interface HostDiagnostic {
  /** `frame-loop` comes from the frame loop, not the host. */
  code: 'shader-compile' | 'text-asset' | 'frame-loop';
  out: OutputIndex;
  message: string;
}

interface Target {
  fb: WebGLFramebuffer;
  tex: WebGLTexture;
}

interface Sampler {
  loc: WebGLUniformLocation;
  /** `u_out<k>` samples output k; `u_text<id>` the text asset id. */
  kind: 'out' | 'text';
  index: number;
}

interface Program {
  program: WebGLProgram;
  time: WebGLUniformLocation | null;
  resolution: WebGLUniformLocation | null;
  named: (WebGLUniformLocation | null)[];
  samplers: Sampler[];
  textures: Map<number, WebGLTexture>;
}

interface Output {
  targets: [Target, Target];
  /** The target holding the latest frame. */
  cur: 0 | 1;
  program: Program | null;
  /** False after a failed or empty `program` record: later `uniforms` belong to it. */
  current: boolean;
  values: number[];
}

interface CompositeProgram {
  program: WebGLProgram;
  sampler: WebGLUniformLocation | null;
  resolution: WebGLUniformLocation | null;
  fragment: WebGLShader;
}

const COMPOSITE_FRAGMENT = `#version 300 es
precision highp float;
uniform sampler2D u_image;
uniform vec2 resolution;
out vec4 fragColor;
void main() {
  vec2 uv = gl_FragCoord.xy / resolution;
  fragColor = texture(u_image, uv);
}
`;

const SAMPLER_DECL = /uniform\s+sampler2D\s+(u_out|u_text)(\d+)\s*;/g;

export class GlRenderHost {
  private readonly gl: WebGL2RenderingContext;
  private readonly size: RenderSize;
  private readonly createCanvas: CanvasFactory;
  private readonly outputs: Output[];
  private readonly diagListeners: ((d: HostDiagnostic) => void)[] = [];
  private readonly programListeners: ((out: OutputIndex) => void)[] = [];
  private vertex: WebGLShader | null = null;
  private videoTexture: WebGLTexture | null = null;
  private videoSize: { width: number; height: number } | null = null;
  private videoSource: TexImageSource | null = null;
  private videoRevision = -1;
  private uploadedVideoRevision = -1;
  private composite: CompositeProgram | null = null;
  private disposed = false;

  constructor(gl: WebGL2RenderingContext, size: RenderSize, opts: { createCanvas?: CanvasFactory } = {}) {
    this.gl = gl;
    this.size = size;
    this.createCanvas = opts.createCanvas ?? (() => document.createElement('canvas'));
    this.outputs = OUTPUTS.map(() => ({
      targets: [this.target(), this.target()],
      cur: 0,
      program: null,
      current: true,
      values: [],
    }));
    for (const o of this.outputs) this.clear(o);
  }

  /** Host diagnostics (`shader-compile`, `text-asset`). */
  onDiagnostic(cb: (d: HostDiagnostic) => void): () => void {
    return listen(this.diagListeners, cb);
  }

  /** A `program` record was accepted (compiled, or the empty program cleared). */
  onProgram(cb: (out: OutputIndex) => void): () => void {
    return listen(this.programListeners, cb);
  }

  /** Whether output `out` has a program drawing. */
  hasProgram(out: OutputIndex): boolean {
    return (this.outputs[out]?.program ?? null) !== null;
  }

  /** The texture holding output `out`'s latest frame. */
  texture(out: OutputIndex): WebGLTexture {
    const o = this.output(out);
    return o.targets[o.cur].tex;
  }

  /** Sets the current video frame; an unchanged revision is never uploaded again. */
  setVideoSource(source: TexImageSource | null, revision: number): void {
    if (this.disposed) return;
    if (this.videoRevision === revision && this.videoSource === source) return;
    this.videoSource = source;
    this.videoRevision = revision;
    if (source === null) this.uploadedVideoRevision = -1;
  }

  onRecord(rec: RenderRecord): void {
    if (this.disposed) return;
    const o = this.outputs[rec.out];
    if (!o) return;
    if (rec.op === 'uniforms') {
      if (o.current && o.program) o.values = rec.values.slice();
      return;
    }
    if (rec.source === '') {
      this.release(o);
      o.current = false;
      this.clear(o);
      this.emitProgram(rec.out);
      return;
    }
    const built = this.build(rec.out, rec.source, rec.uniform_names);
    if (!built) {
      o.current = false;
      return;
    }
    this.release(o);
    o.program = built;
    o.current = true;
    o.values = [];
    this.emitProgram(rec.out);
    // After the accept event, so a pane's banner keeps the asset failure.
    for (const a of rec.assets) {
      const r = uploadTextAsset(this.gl, this.createCanvas, a);
      if (r.ok) built.textures.set(a.id, r.texture);
      else this.diag(rec.out, 'text-asset', r.message);
    }
  }

  /** Draws every output that has a program, then swaps its pair. */
  draw(timeSec: number): void {
    if (this.disposed) return;
    const gl = this.gl;
    this.uploadVideo();
    // `u_out<k>` is output k's previous frame, whatever order outputs draw in.
    const prev = this.outputs.map((o) => o.targets[o.cur].tex);
    for (const o of this.outputs) {
      const p = o.program;
      if (!p) continue;
      const back = o.targets[o.cur === 0 ? 1 : 0];
      gl.bindFramebuffer(gl.FRAMEBUFFER, back.fb);
      gl.viewport(0, 0, this.size.width, this.size.height);
      gl.useProgram(p.program);
      if (p.time) gl.uniform1f(p.time, timeSec);
      if (p.resolution) gl.uniform2f(p.resolution, this.size.width, this.size.height);
      p.named.forEach((loc, i) => {
        const v = o.values[i];
        if (loc && typeof v === 'number' && Number.isFinite(v)) gl.uniform1f(loc, v);
      });
      p.samplers.forEach((s, unit) => {
        const tex = s.kind === 'out' ? prev[s.index] : p.textures.get(s.index);
        gl.activeTexture(gl.TEXTURE0 + unit);
        gl.bindTexture(gl.TEXTURE_2D, tex ?? null);
        gl.uniform1i(s.loc, unit);
      });
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      o.cur = o.cur === 0 ? 1 : 0;
    }
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  }

  /** Copies output `out`'s latest frame into the default framebuffer (for a pane blit). */
  present(out: OutputIndex): void {
    if (this.disposed) return;
    const gl = this.gl;
    const { width, height } = this.size;
    const outputTexture = this.output(out).targets[this.output(out).cur].tex;
    if (!this.videoTexture || this.uploadedVideoRevision < 0) {
      gl.bindFramebuffer(gl.READ_FRAMEBUFFER, this.output(out).targets[this.output(out).cur].fb);
      gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, null);
      gl.blitFramebuffer(0, 0, width, height, 0, 0, width, height, gl.COLOR_BUFFER_BIT, gl.NEAREST);
      gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
      return;
    }
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, width, height);
    const program = this.compositeProgram();
    if (!program) return;
    gl.disable(gl.BLEND);
    this.drawComposite(program, this.videoTexture);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    this.drawComposite(program, outputTexture);
    gl.disable(gl.BLEND);
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    const gl = this.gl;
    for (const o of this.outputs) {
      this.release(o);
      for (const t of o.targets) {
        gl.deleteFramebuffer(t.fb);
        gl.deleteTexture(t.tex);
      }
    }
    if (this.videoTexture) gl.deleteTexture(this.videoTexture);
    this.videoTexture = null;
    this.videoSource = null;
    if (this.composite) {
      gl.deleteProgram(this.composite.program);
      gl.deleteShader(this.composite.fragment);
      this.composite = null;
    }
    if (this.vertex) gl.deleteShader(this.vertex);
    this.vertex = null;
    this.diagListeners.length = 0;
    this.programListeners.length = 0;
  }

  // ------------------------------------------------------------ private

  private output(out: OutputIndex): Output {
    const o = this.outputs[out];
    if (!o) throw new Error(`no output o${out}`);
    return o;
  }

  private target(): Target {
    const gl = this.gl;
    const tex = gl.createTexture();
    const fb = gl.createFramebuffer();
    if (!tex || !fb) throw new Error('WebGL2 resources unavailable');
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, this.size.width, this.size.height, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.bindTexture(gl.TEXTURE_2D, null);
    return { fb, tex };
  }

  private uploadVideo(): void {
    const source = this.videoSource;
    if (!source || this.videoRevision === this.uploadedVideoRevision) return;
    const size = sourceSize(source);
    if (!size.width || !size.height) return;
    const gl = this.gl;
    if (!this.videoTexture) this.videoTexture = gl.createTexture();
    if (!this.videoTexture) return;
    gl.bindTexture(gl.TEXTURE_2D, this.videoTexture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    if (!this.videoSize || size.width !== this.videoSize.width || size.height !== this.videoSize.height) {
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, size.width, size.height, 0, gl.RGBA, gl.UNSIGNED_BYTE, source);
      this.videoSize = size;
    } else {
      gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, gl.RGBA, gl.UNSIGNED_BYTE, source);
    }
    gl.bindTexture(gl.TEXTURE_2D, null);
    this.uploadedVideoRevision = this.videoRevision;
  }

  private compositeProgram(): CompositeProgram | null {
    if (this.composite) return this.composite;
    const gl = this.gl;
    const vertex = this.vertexShader();
    if (!vertex) return null;
    const fragment = this.compile(gl.FRAGMENT_SHADER, COMPOSITE_FRAGMENT);
    if (!fragment.shader) return null;
    const program = gl.createProgram();
    if (!program) { gl.deleteShader(fragment.shader); return null; }
    gl.attachShader(program, vertex);
    gl.attachShader(program, fragment.shader);
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      gl.deleteProgram(program);
      gl.deleteShader(fragment.shader);
      return null;
    }
    this.composite = { program, fragment: fragment.shader,
      sampler: gl.getUniformLocation(program, 'u_image'),
      resolution: gl.getUniformLocation(program, 'resolution') };
    return this.composite;
  }

  private drawComposite(program: CompositeProgram, texture: WebGLTexture): void {
    const gl = this.gl;
    gl.useProgram(program.program);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, texture);
    if (program.sampler) gl.uniform1i(program.sampler, 0);
    if (program.resolution) gl.uniform2f(program.resolution, this.size.width, this.size.height);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }

  /** Clears both buffers of `o` to opaque black. */
  private clear(o: Output): void {
    const gl = this.gl;
    gl.clearColor(0, 0, 0, 1);
    for (const t of o.targets) {
      gl.bindFramebuffer(gl.FRAMEBUFFER, t.fb);
      gl.viewport(0, 0, this.size.width, this.size.height);
      gl.clear(gl.COLOR_BUFFER_BIT);
    }
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  }

  private release(o: Output): void {
    const p = o.program;
    if (!p) return;
    this.gl.deleteProgram(p.program);
    for (const t of p.textures.values()) this.gl.deleteTexture(t);
    o.program = null;
    o.values = [];
  }

  private vertexShader(): WebGLShader | null {
    if (!this.vertex) this.vertex = this.compile(this.gl.VERTEX_SHADER, VERTEX_SOURCE).shader;
    return this.vertex;
  }

  private compile(type: number, source: string): { shader: WebGLShader | null; log: string } {
    const gl = this.gl;
    const shader = gl.createShader(type);
    if (!shader) return { shader: null, log: 'createShader failed' };
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      const log = gl.getShaderInfoLog(shader) ?? '';
      gl.deleteShader(shader);
      return { shader: null, log };
    }
    return { shader, log: '' };
  }

  /** Compiles and links; null (after a diagnostic) on failure. */
  private build(
    out: OutputIndex,
    source: string,
    names: readonly string[],
  ): Program | null {
    const gl = this.gl;
    const vs = this.vertexShader();
    if (!vs) {
      this.diag(out, 'shader-compile', 'vertex shader failed to compile');
      return null;
    }
    const fs = this.compile(gl.FRAGMENT_SHADER, source);
    if (!fs.shader) {
      this.diag(out, 'shader-compile', fs.log || 'fragment shader failed to compile');
      return null;
    }
    const program = gl.createProgram();
    if (!program) {
      gl.deleteShader(fs.shader);
      this.diag(out, 'shader-compile', 'createProgram failed');
      return null;
    }
    gl.attachShader(program, vs);
    gl.attachShader(program, fs.shader);
    gl.linkProgram(program);
    gl.detachShader(program, fs.shader);
    gl.deleteShader(fs.shader);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      const log = gl.getProgramInfoLog(program) ?? '';
      gl.deleteProgram(program);
      this.diag(out, 'shader-compile', log || 'program failed to link');
      return null;
    }
    const samplers: Sampler[] = [];
    for (const m of source.matchAll(SAMPLER_DECL)) {
      const loc = gl.getUniformLocation(program, `${m[1]}${m[2]}`);
      if (loc) samplers.push({ loc, kind: m[1] === 'u_out' ? 'out' : 'text', index: Number(m[2]) });
    }
    return {
      program,
      time: gl.getUniformLocation(program, 'time'),
      resolution: gl.getUniformLocation(program, 'resolution'),
      named: names.map((n) => gl.getUniformLocation(program, n)),
      samplers,
      textures: new Map(),
    };
  }

  private diag(out: OutputIndex, code: HostDiagnostic['code'], message: string): void {
    for (const cb of [...this.diagListeners]) cb({ code, out, message });
  }

  private emitProgram(out: OutputIndex): void {
    for (const cb of [...this.programListeners]) cb(out);
  }
}

function sourceSize(source: TexImageSource): { width: number; height: number } {
  const s = source as unknown as { width?: number; height?: number; videoWidth?: number; videoHeight?: number; naturalWidth?: number; naturalHeight?: number };
  return { width: s.videoWidth || s.naturalWidth || s.width || 0, height: s.videoHeight || s.naturalHeight || s.height || 0 };
}

function listen<T>(list: T[], cb: T): () => void {
  list.push(cb);
  return () => {
    const i = list.indexOf(cb);
    if (i >= 0) list.splice(i, 1);
  };
}
