// RecordingGL: a WebGL2 fake for the render-host tests. Every call is
// recorded in order; shaders and programs are plain objects. The uniform
// locations a program exposes are the `uniform` declarations of its
// attached fragment source, so `getUniformLocation` is null for a name the
// source does not declare. `failNextCompile(log)` / `failNextLink(log)`
// make the next fragment compile / link fail with that info log.

export interface GlCall {
  fn: string;
  args: unknown[];
}

export interface FakeShader {
  kind: 'shader';
  id: number;
  type: number;
  source: string;
  ok: boolean;
  log: string;
}

export interface FakeProgram {
  kind: 'program';
  id: number;
  shaders: FakeShader[];
  ok: boolean;
  log: string;
  deleted: boolean;
}

export interface FakeLocation {
  kind: 'location';
  program: number;
  name: string;
}

export interface FakeObject {
  kind: 'texture' | 'framebuffer';
  id: number;
}

let nextId = 1;

export class RecordingGL {
  readonly VERTEX_SHADER = 0x8b31;
  readonly FRAGMENT_SHADER = 0x8b30;
  readonly COMPILE_STATUS = 0x8b81;
  readonly LINK_STATUS = 0x8b82;
  readonly TEXTURE_2D = 0x0de1;
  readonly TEXTURE0 = 0x84c0;
  readonly RGBA = 0x1908;
  readonly UNSIGNED_BYTE = 0x1401;
  readonly UNSIGNED_SHORT = 0x1403;
  readonly FLOAT = 0x1406;
  readonly RG = 0x8227;
  readonly RG32F = 0x8230;
  readonly DYNAMIC_DRAW = 0x88e8;
  readonly ARRAY_BUFFER = 0x8892;
  readonly MAX_TEXTURE_SIZE = 0x0d33;
  readonly UNPACK_SKIP_PIXELS = 0x0cf4;
  readonly UNPACK_SKIP_ROWS = 0x0cf3;
  readonly UNPACK_ROW_LENGTH = 0x0cf2;
  readonly TEXTURE1 = 0x84c1;
  readonly TEXTURE_MIN_FILTER = 0x2801;
  readonly TEXTURE_MAG_FILTER = 0x2800;
  readonly TEXTURE_WRAP_S = 0x2802;
  readonly TEXTURE_WRAP_T = 0x2803;
  readonly LINEAR = 0x2601;
  readonly NEAREST = 0x2600;
  readonly CLAMP_TO_EDGE = 0x812f;
  readonly FRAMEBUFFER = 0x8d40;
  readonly READ_FRAMEBUFFER = 0x8ca8;
  readonly DRAW_FRAMEBUFFER = 0x8ca9;
  readonly COLOR_ATTACHMENT0 = 0x8ce0;
  readonly COLOR_BUFFER_BIT = 0x4000;
  readonly TRIANGLES = 0x0004;
  readonly UNPACK_FLIP_Y_WEBGL = 0x9240;

  readonly calls: GlCall[] = [];
  readonly canvas = { width: 640, height: 360 };
  private compileFailure: string | null = null;
  private linkFailure: string | null = null;
  /** The framebuffer bound to FRAMEBUFFER (null = default). */
  boundFramebuffer: FakeObject | null = null;
  current: FakeProgram | null = null;

  failNextCompile(log: string): void {
    this.compileFailure = log;
  }

  failNextLink(log: string): void {
    this.linkFailure = log;
  }

  /** The recorded calls named `fn`, in order. */
  named(fn: string): GlCall[] {
    return this.calls.filter((c) => c.fn === fn);
  }

  /** The recorded calls from index `from` on. */
  since(from: number): GlCall[] {
    return this.calls.slice(from);
  }

  as(): WebGL2RenderingContext {
    return this as unknown as WebGL2RenderingContext;
  }

  private rec(fn: string, args: unknown[]): void {
    this.calls.push({ fn, args });
  }

  // ------------------------------------------------------------ shaders

  createShader(type: number): FakeShader {
    const s: FakeShader = { kind: 'shader', id: nextId++, type, source: '', ok: false, log: '' };
    this.rec('createShader', [type]);
    return s;
  }

  shaderSource(s: FakeShader, source: string): void {
    s.source = source;
    this.rec('shaderSource', [s, source]);
  }

  compileShader(s: FakeShader): void {
    this.rec('compileShader', [s]);
    if (s.type === this.FRAGMENT_SHADER && this.compileFailure !== null) {
      s.ok = false;
      s.log = this.compileFailure;
      this.compileFailure = null;
      return;
    }
    s.ok = true;
  }

  getShaderParameter(s: FakeShader, pname: number): boolean {
    return pname === this.COMPILE_STATUS ? s.ok : false;
  }

  getShaderInfoLog(s: FakeShader): string {
    return s.log;
  }

  deleteShader(s: FakeShader | null): void {
    this.rec('deleteShader', [s]);
  }

  // ----------------------------------------------------------- programs

  createProgram(): FakeProgram {
    const p: FakeProgram = { kind: 'program', id: nextId++, shaders: [], ok: false, log: '', deleted: false };
    this.rec('createProgram', []);
    return p;
  }

  attachShader(p: FakeProgram, s: FakeShader): void {
    p.shaders.push(s);
    this.rec('attachShader', [p, s]);
  }

  detachShader(p: FakeProgram, s: FakeShader): void {
    this.rec('detachShader', [p, s]);
  }

  linkProgram(p: FakeProgram): void {
    this.rec('linkProgram', [p]);
    if (this.linkFailure !== null) {
      p.ok = false;
      p.log = this.linkFailure;
      this.linkFailure = null;
      return;
    }
    p.ok = p.shaders.every((s) => s.ok);
  }

  getProgramParameter(p: FakeProgram, pname: number): boolean {
    return pname === this.LINK_STATUS ? p.ok : false;
  }

  getProgramInfoLog(p: FakeProgram): string {
    return p.log;
  }

  deleteProgram(p: FakeProgram | null): void {
    if (p) p.deleted = true;
    this.rec('deleteProgram', [p]);
  }

  useProgram(p: FakeProgram | null): void {
    this.current = p;
    this.rec('useProgram', [p]);
  }

  getUniformLocation(p: FakeProgram, name: string): FakeLocation | null {
    const frag = p.shaders.find((s) => s.type === this.FRAGMENT_SHADER);
    const decl = new RegExp(`uniform\\s+\\w+\\s+${name}\\s*;`);
    if (!frag || !decl.test(frag.source)) return null;
    return { kind: 'location', program: p.id, name };
  }

  uniform1f(loc: FakeLocation, v: number): void {
    this.rec('uniform1f', [loc.name, v]);
  }

  uniform2f(loc: FakeLocation, x: number, y: number): void {
    this.rec('uniform2f', [loc.name, x, y]);
  }

  uniform1i(loc: FakeLocation, v: number): void {
    this.rec('uniform1i', [loc.name, v]);
  }

  // ------------------------------------------------- textures and buffers

  createTexture(): FakeObject {
    const t: FakeObject = { kind: 'texture', id: nextId++ };
    this.rec('createTexture', [t]);
    return t;
  }

  deleteTexture(t: FakeObject | null): void {
    this.rec('deleteTexture', [t]);
  }

  bindTexture(target: number, t: FakeObject | null): void {
    this.rec('bindTexture', [target, t]);
  }

  activeTexture(unit: number): void {
    this.rec('activeTexture', [unit]);
  }

  texImage2D(...args: unknown[]): void {
    this.rec('texImage2D', args);
  }

  texParameteri(...args: unknown[]): void {
    this.rec('texParameteri', args);
  }

  pixelStorei(pname: number, v: unknown): void {
    this.rec('pixelStorei', [pname, v]);
  }

  createFramebuffer(): FakeObject {
    const f: FakeObject = { kind: 'framebuffer', id: nextId++ };
    this.rec('createFramebuffer', [f]);
    return f;
  }

  deleteFramebuffer(f: FakeObject | null): void {
    this.rec('deleteFramebuffer', [f]);
  }

  bindFramebuffer(target: number, f: FakeObject | null): void {
    if (target === this.FRAMEBUFFER) this.boundFramebuffer = f;
    this.rec('bindFramebuffer', [target, f]);
  }

  /** Recorded as `[...args, bound framebuffer]`. */
  framebufferTexture2D(...args: unknown[]): void {
    this.rec('framebufferTexture2D', [...args, this.boundFramebuffer]);
  }

  /** The texture attached to framebuffer `fb`. */
  textureOf(fb: unknown): unknown {
    const c = this.named('framebufferTexture2D').find((x) => x.args[5] === fb);
    return c?.args[3];
  }

  blitFramebuffer(...args: unknown[]): void {
    this.rec('blitFramebuffer', args);
  }

  // ------------------------------------------------------------ drawing

  viewport(...args: number[]): void {
    this.rec('viewport', args);
  }

  clearColor(...args: number[]): void {
    this.rec('clearColor', args);
  }

  clear(mask: number): void {
    this.rec('clear', [mask, this.boundFramebuffer]);
  }

  drawArrays(mode: number, first: number, count: number): void {
    this.rec('drawArrays', [mode, first, count, this.boundFramebuffer, this.current]);
  }

  drawArraysInstanced(mode: number, first: number, count: number, instances: number): void {
    this.rec('drawArraysInstanced', [mode, first, count, instances, this.boundFramebuffer, this.current]);
  }

  bufferData(...args: unknown[]): void { this.rec('bufferData', args); }
  bufferSubData(...args: unknown[]): void { this.rec('bufferSubData', args); }
  vertexAttribPointer(...args: unknown[]): void { this.rec('vertexAttribPointer', args); }
  vertexAttribIPointer(...args: unknown[]): void { this.rec('vertexAttribIPointer', args); }
  vertexAttribDivisor(...args: unknown[]): void { this.rec('vertexAttribDivisor', args); }
  enableVertexAttribArray(...args: unknown[]): void { this.rec('enableVertexAttribArray', args); }
}
