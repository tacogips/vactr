import { Text } from '@codemirror/state';
import type { CodeAnnotation, CodeRect } from '../app/apis';
import { GlyphAtlas, type CanvasFactory, type RunStyle } from './atlas';
import { TextLayout, type LayoutViewport, type ShapedLine, type ShapedRun } from './layout';
import { ResourceLedger, effectiveSize, RESOURCE_LIMITS, type Reservation } from './resources';
import type { PhaseTimer } from './frame';
import { FALLBACK_PALETTE, rgbaCss, type Palette } from './palette';

export interface GpuStatus {
  kind: 'ready' | 'degraded' | 'unavailable' | 'context-lost'; message: string;
  effectiveDpr: number; saveText(): string;
}
export interface RenderFeedback {
  annotations?: readonly CodeAnnotation[]; cursor?: number | null;
  handles?: readonly { pos: number; end?: boolean }[];
  textRevision?: number;
  annotationsRevision?: number;
  animated?: readonly CodeAnnotation[];
  cursorVisible?: boolean;
}
interface DrawCommand { rect: CodeRect; color: number[]; texture?: WebGLTexture; uv?: [number, number, number, number]; generation?: number; cursor?: boolean; clipped: boolean }
export interface RendererOptions {
  ledger?: ResourceLedger; gl?: WebGL2RenderingContext; createCanvas?: CanvasFactory;
  palette?: Palette;
  onStatus?: (status: GpuStatus) => void;
}
const VERTEX = `#version 300 es
in vec2 a_pos;
uniform vec4 u_rect;
uniform vec2 u_view;
out vec2 v_uv;
uniform vec4 u_uv;
void main() {
  v_uv = mix(u_uv.xy, u_uv.zw, a_pos);
  vec2 p = u_rect.xy + a_pos * u_rect.zw;
  gl_Position = vec4(p.x / u_view.x * 2.0 - 1.0, 1.0 - p.y / u_view.y * 2.0, 0., 1.);
}`;
const FRAGMENT = `#version 300 es
precision mediump float;
in vec2 v_uv;
uniform sampler2D u_texture;
uniform vec4 u_color;
out vec4 out_color;
void main() { out_color = texture(u_texture, v_uv) * u_color; }`;
export const GPU_TOKEN_COLORS: Readonly<Record<string, string>> = Object.freeze({
  'vact-tok-comment': '#7a7f87', 'vact-tok-directive': '#b07bd8', 'vact-tok-keyword': '#d08770',
  'vact-tok-number': '#88c0d0', 'vact-tok-string': '#a3be8c', 'vact-tok-path': '#a3be8c',
  'vact-tok-head': '#ebcb8b', 'vact-tok-bracket': '#8a8f98',
});

/** GPU presentation only. Mount/input owns editing, callbacks, scrolling and frame scheduling. */
export class CanvasRenderer {
  readonly ledger: ResourceLedger;
  readonly stats = { frames: 0, textBuilds: 0, bufferUploads: 0, drawCalls: 0 };
  private gl: WebGL2RenderingContext | null;
  private atlas: GlyphAtlas | null = null;
  private program: WebGLProgram | null = null;
  private buffer: WebGLBuffer | null = null;
  private vao: WebGLVertexArrayObject | null = null;
  private white: WebGLTexture | null = null;
  private allocations: Reservation[] = [];
  private target: Reservation | null = null;
  private maxTextureSize = 0;
  private palette: Palette;
  private locations: Record<string, WebGLUniformLocation | null> = {};
  private view: LayoutViewport = { width: 1, height: 1, scrollLeft: 0, scrollTop: 0 };
  private requestedDpr = 1;
  private labels = new Map<string, ShapedRun>();
  private beforeAnimation: DrawCommand[] | null = null;
  private afterAnimation: DrawCommand[] | null = null;
  private cacheKey = '';
  private textureGeneration = 0;
  private seenEvictions = 0;
  private collecting: DrawCommand[] | null = null;
  private drawWhileCollecting = false;
  private cursorVisibleWhileCollecting = true;
  private scissorOn = false;
  private pendingText = false;
  private visibleLines: ShapedLine[] = [];
  private pendingStartLine = 0;
  private currentLineIndex = 0;
  private scale = 1;
  private lost = false;
  private disposed = false;
  private statusValue: GpuStatus;
  private readonly createCanvas: CanvasFactory;
  private readonly loss = (event: Event): void => {
    event.preventDefault(); this.textureGeneration++; this.lost = true; this.clearDrawCache(); this.releaseGpu(true); this.report('context-lost', 'GPU context lost; editing and save remain available');
  };
  private readonly restore = (): void => {
    if (this.disposed) return;
    this.textureGeneration++;
    this.lost = false;
    try { this.initialize(); this.resize(this.requestedDpr); this.reportReady(); }
    catch (e) { this.fail(e); }
  };
  private readonly fontsChanged = (): void => {
    this.layout.setFont({ ...this.layout.font, generation: (this.layout.font.generation ?? 0) + 1 });
    this.textureGeneration++;
    this.atlas?.invalidate();
  };
  constructor(readonly canvas: HTMLCanvasElement, readonly layout: TextLayout, private options: RendererOptions = {}) {
    this.ledger = options.ledger ?? new ResourceLedger();
    this.palette = options.palette ?? FALLBACK_PALETTE;
    this.createCanvas = options.createCanvas ?? (() => document.createElement('canvas'));
    this.gl = options.gl ?? canvas.getContext('webgl2', { alpha: true, antialias: false, depth: false, stencil: false });
    this.statusValue = { kind: 'unavailable', message: 'GPU not initialized', effectiveDpr: 1, saveText: () => this.layout.document };
    canvas.addEventListener('webglcontextlost', this.loss); canvas.addEventListener('webglcontextrestored', this.restore);
    document.fonts?.addEventListener('loadingdone', this.fontsChanged);
    try { this.initialize(); this.reportReady(); } catch (e) { this.fail(e); }
  }
  get status(): GpuStatus { return this.statusValue; }
  get textPending(): boolean { return this.pendingText; }
  get atlasStats(): Readonly<{ uploads: number; hits: number; evictions: number }> { return this.atlas?.stats ?? { uploads: 0, hits: 0, evictions: 0 }; }
  setPhases(phases: PhaseTimer | null): void { this.layout.phases = phases; }
  setDocument(text: string): void { this.layout.setDocument(text); this.clearDrawCache(); }
  setText(doc: Text): void { this.layout.setText(doc); this.clearDrawCache(); }
  setPalette(palette: Palette): void { this.palette = palette; this.clearDrawCache(); }
  setViewport(view: LayoutViewport, dpr = 1): void {
    if (![view.width, view.height, view.scrollLeft, view.scrollTop, view.left ?? 0, view.top ?? 0, view.gutter ?? 48].every(Number.isFinite) || view.width <= 0 || view.height <= 0 || view.scrollLeft < 0 || view.scrollTop < 0 || dpr <= 0 || !Number.isFinite(dpr)) throw new RangeError('Invalid viewport');
    this.view = { ...view }; this.requestedDpr = dpr;
    if (!this.gl || this.lost || this.disposed || !this.program) return;
    try { this.resize(dpr); this.reportReady(); } catch (e) { this.fail(e); }
  }
  private initialize(): void {
    const gl = this.gl; if (!gl) throw new Error('WebGL2 unavailable');
    this.maxTextureSize = gl.getParameter(gl.MAX_TEXTURE_SIZE) as number;
    const geometry = this.ledger.allocate('geometry', 12 * 4);
    const white = this.ledger.allocate('atlas', 4);
    if (!geometry || !white) { geometry?.release(); white?.release(); throw new Error('GPU initialization budget exhausted'); }
    this.allocations.push(geometry, white);
    const shaders: WebGLShader[] = [];
    try {
      for (const [type, source] of [[gl.VERTEX_SHADER, VERTEX], [gl.FRAGMENT_SHADER, FRAGMENT]] as const) {
        const shader = gl.createShader(type); if (!shader) throw new Error('Shader allocation failed');
        shaders.push(shader); gl.shaderSource(shader, source); gl.compileShader(shader);
        if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(shader) ?? 'Shader compile failed');
      }
      this.program = gl.createProgram(); if (!this.program) throw new Error('Program allocation failed');
      for (const shader of shaders) gl.attachShader(this.program, shader);
      gl.linkProgram(this.program); if (!gl.getProgramParameter(this.program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(this.program) ?? 'Program link failed');
      this.buffer = gl.createBuffer(); this.vao = gl.createVertexArray(); this.white = gl.createTexture();
      if (!this.buffer || !this.vao || !this.white) throw new Error('GPU object allocation failed');
      gl.bindVertexArray(this.vao); gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 1]), gl.STATIC_DRAW); this.stats.bufferUploads++;
      const attribute = gl.getAttribLocation(this.program, 'a_pos'); gl.enableVertexAttribArray(attribute); gl.vertexAttribPointer(attribute, 2, gl.FLOAT, false, 0, 0);
      gl.bindTexture(gl.TEXTURE_2D, this.white);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array([255, 255, 255, 255]));
      for (const name of ['u_rect', 'u_view', 'u_color', 'u_texture', 'u_uv']) this.locations[name] = gl.getUniformLocation(this.program, name);
      if (gl.getError() !== gl.NO_ERROR) throw new Error('GPU initialization failed');
      this.atlas = new GlyphAtlas(gl, this.ledger, this.createCanvas);
    } finally { for (const shader of shaders) gl.deleteShader(shader); }
  }
  private resize(dpr: number): void {
    const gl = this.gl!;
    // Reserve old + new simultaneously. Reuse equal dimensions without a replacement.
    let size = effectiveSize(this.view.width, this.view.height, dpr, this.maxTextureSize, RESOURCE_LIMITS.canvasPixels / 2);
    if (this.target && this.canvas.width === size.width && this.canvas.height === size.height) {
      if (this.scale !== size.dpr) { this.textureGeneration++; this.atlas?.invalidate(); this.layout.invalidate(); }
      this.scale = size.dpr; return;
    }
    const freePixels = RESOURCE_LIMITS.canvasPixels - this.ledger.counters.pixels;
    const freeBytes = this.ledger.limitBytes - this.ledger.usedBytes;
    size = effectiveSize(this.view.width, this.view.height, dpr, this.maxTextureSize, Math.min(RESOURCE_LIMITS.canvasPixels / 2, freePixels, Math.floor(freeBytes / 4)));
    const allocation = this.ledger.allocate('canvas', size.width * size.height * 4, size);
    if (!allocation) throw new Error('Canvas replacement budget exhausted');
    try {
      this.canvas.width = size.width; this.canvas.height = size.height;
      if (gl.isContextLost() || gl.getError() !== gl.NO_ERROR) throw new Error('Canvas allocation failed');
    } catch (e) { allocation.release(); throw e; }
    this.target?.release(); this.target = allocation;
    if (this.scale !== size.dpr) { this.textureGeneration++; this.atlas?.invalidate(); this.layout.invalidate(); }
    this.scale = size.dpr;
  }
  render(feedback: RenderFeedback = {}): boolean {
    if (this.disposed || this.lost || !this.gl || !this.program || !this.atlas) return false;
    try {
      if (this.atlas.stats.evictions !== this.seenEvictions) { this.textureGeneration++; this.seenEvictions = this.atlas.stats.evictions; }
      if (!this.target) this.resize(this.requestedDpr);
      const gl = this.gl; this.atlas.beginFrame(); this.pendingText = false;
      gl.bindFramebuffer(gl.FRAMEBUFFER, null); gl.bindVertexArray(this.vao); gl.useProgram(this.program);
      gl.viewport(0, 0, this.canvas.width, this.canvas.height); this.unclip();
      gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
      gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT);
      gl.uniform2f(this.locations.u_view, this.view.width, this.view.height); gl.uniform1i(this.locations.u_texture, 0); gl.activeTexture(gl.TEXTURE0);
      this.quad({ left: 0, right: this.view.width, top: 0, bottom: this.view.height }, [0.04, 0.05, 0.07, 0.85]);
      const annotations = feedback.annotations ?? [];
      const staticAnnotations = annotations.filter(a => a.kind !== 'playing' && a.kind !== 'eval');
      const annotationKey = feedback.annotationsRevision === undefined ? staticAnnotations : feedback.annotationsRevision;
      const key = JSON.stringify([feedback.textRevision, this.view, this.scale, this.layout.font.generation, this.atlas.stats.evictions, annotationKey, feedback.cursor, feedback.handles]);
      const rebuild = feedback.textRevision === undefined || this.beforeAnimation === null || this.afterAnimation === null || this.cacheKey !== key;
      let unsupported = false;
      let before = this.beforeAnimation ?? [];
      let after = this.afterAnimation ?? [];
      if (rebuild) {
        before = []; after = [];
        this.collecting = before; this.clip(this.view.gutter ?? 48);
        for (const a of staticAnnotations) if (a.kind === 'selection') for (const r of this.layout.rangeRects(a, this.view)) this.quad(this.local(r), [0.12, 0.22, 0.32, 0.55]);
        this.collecting = null;
        for (const command of before) this.replay(command);
        this.visibleLines = this.layout.visible(this.view);
      } else {
        for (const command of before) this.replay(command);
      }
      this.clip(this.view.gutter ?? 48);
      for (const a of [...annotations, ...(feedback.animated ?? [])]) if (a.kind === 'playing' || a.kind === 'eval') {
        const color = a.kind === 'playing' ? [0.35, 0.29, 0.13, 0.5] : a.className?.includes('error') ? [0.4, 0.12, 0.16, 0.5] : [0.2, 0.32, 0.36, 0.5];
        for (const r of this.animatedRects(a)) this.quad(this.local(r), color);
      }
      if (rebuild) {
        this.collecting = after; this.drawWhileCollecting = true; this.cursorVisibleWhileCollecting = feedback.cursorVisible !== false;
        this.clip(this.view.gutter ?? 48);
        for (let offset = 0; offset < this.visibleLines.length; offset++) {
          const index = (this.pendingStartLine + offset) % this.visibleLines.length;
          const line = this.visibleLines[index]!; this.currentLineIndex = index;
          unsupported ||= line.rtlUnsupported; const y = line.number * this.layout.font.lineHeight - this.view.scrollTop;
          for (const run of line.runs) this.drawRun(run, (this.view.gutter ?? 48) + run.x - this.view.scrollLeft, y, staticAnnotations);
        }
        this.unclip();
        for (const line of this.visibleLines) this.drawRun(this.labelRun(String(line.number + 1), (this.view.gutter ?? 48) - 8), 4, line.number * this.layout.font.lineHeight - this.view.scrollTop, [], rgbaCss(this.palette.gutter), true);
        this.clip(this.view.gutter ?? 48);
        for (const a of staticAnnotations) {
          if (a.kind === 'diagnostic' || a.kind === 'composition') for (const r of this.layout.rangeRects(a, this.view)) this.quad(this.local({ ...r, top: r.bottom - 2 }), a.kind === 'diagnostic' ? this.palette.diagnostic : this.palette.composition);
          if (a.kind === 'call-head') for (const r of this.layout.rangeRects(a, this.view)) this.quad(this.local({ ...r, top: r.bottom - 1 }), this.palette.callHead);
        }
        if (feedback.cursor != null) { const r = this.layout.coordsAtPos(feedback.cursor, this.view); if (r) this.addCommand({ rect: this.local(r), color: this.palette.cursor, cursor: true }); }
        for (const a of staticAnnotations) {
          if (a.kind === 'binding' && a.label) {
            const r = this.layout.coordsAtPos(a.to, this.view); if (!r) continue;
            const x = r.left - (this.view.left ?? 0) + 3, y = r.top - (this.view.top ?? 0);
            const width = Math.min(256, Math.max(16, a.label.length * 8 + 8));
            this.quad({ left: x, right: x + width, top: y, bottom: y + this.layout.font.lineHeight }, this.palette.labelFill);
            this.drawRun(this.labelRun(a.label, width), x + 4, y, [], rgbaCss(this.palette.labelText));
          }
        }
        for (const handle of feedback.handles ?? []) {
          const r = this.layout.coordsAtPos(handle.pos, this.view); if (!r) continue;
          const x = r.left - (this.view.left ?? 0), y = (handle.end ? r.bottom : r.top) - (this.view.top ?? 0);
          this.quad({ left: x - 5, right: x + 5, top: y - 4, bottom: y + 6 }, this.palette.handle);
        }
      this.collecting = null; this.drawWhileCollecting = false; this.stats.textBuilds++;
        if (!this.pendingText) { this.beforeAnimation = before; this.afterAnimation = after; this.cacheKey = key; this.pendingStartLine = 0; }
        else { this.beforeAnimation = before; this.afterAnimation = after; this.cacheKey = ''; }
      } else {
        for (const command of after) if (!command.cursor || feedback.cursorVisible !== false) this.replay(command);
      }
      this.unclip();
      this.stats.frames++;
      if (unsupported) this.report('degraded', 'Mixed RTL hit testing is unsupported'); else this.reportReady();
      this.atlas.endFrame();
      return true;
    } catch (e) { this.atlas.endFrame(); this.fail(e); return false; }
  }
  private labelRun(text: string, width: number): ShapedRun {
    // Labels are bounded overlays, not document source. Keep stable identity across idle frames.
    if (text.length > 64) text = text.slice(0, 61) + '…';
    const key = `${width}:${text}`; const hit = this.labels.get(key);
    if (hit) { this.labels.delete(key); this.labels.set(key, hit); return hit; }
    if (this.labels.size >= 512) this.labels.delete(this.labels.keys().next().value!);
    const run = { text, from: 0, to: text.length, x: 0, width }; this.labels.set(key, run); return run;
  }
  private replay(command: DrawCommand): void {
    this.drawCommand(command);
  }
  private addCommand(command: Omit<DrawCommand, 'clipped'>): void {
    const recorded = { ...command, ...(command.texture ? { generation: this.textureGeneration } : {}), clipped: this.scissorOn };
    if (this.collecting) {
      this.collecting.push(recorded);
      if (this.drawWhileCollecting && (!recorded.cursor || this.cursorVisibleWhileCollecting)) this.drawCommand(recorded);
      return;
    }
    this.drawCommand(recorded);
  }
  private drawCommand(command: DrawCommand): void {
    if (command.texture && command.generation !== this.textureGeneration) return;
    const gl = this.gl!;
    if (command.clipped && !this.scissorOn) this.clip(this.view.gutter ?? 48);
    else if (!command.clipped && this.scissorOn) this.unclip();
    gl.bindTexture(gl.TEXTURE_2D, command.texture ?? this.white);
    gl.uniform4f(this.locations.u_rect, command.rect.left, command.rect.top, command.rect.right - command.rect.left, command.rect.bottom - command.rect.top);
    gl.uniform4f(this.locations.u_color, command.color[0]!, command.color[1]!, command.color[2]!, command.color[3]!);
    const uv = command.uv ?? [0, 0, 1, 1];
    gl.uniform4f(this.locations.u_uv, uv[0]!, uv[1]!, uv[2]!, uv[3]!);
    gl.drawArrays(gl.TRIANGLES, 0, 6); this.stats.drawCalls++;
  }
  private animatedRects(range: CodeAnnotation): CodeRect[] {
    if (this.visibleLines.length === 0) return [];
    const visibleFrom = this.visibleLines[0]!.from, visibleTo = this.visibleLines[this.visibleLines.length - 1]!.to;
    const from = range.from >= visibleFrom && range.from <= visibleTo ? this.layout.boundary(range.from) : range.from;
    const to = range.to >= visibleFrom && range.to <= visibleTo ? this.layout.boundary(range.to, 1) : range.to;
    const out: CodeRect[] = [];
    for (const line of this.visibleLines) {
      if (line.to < from || line.from > to) continue;
      const start = Math.max(from, line.from), end = Math.min(to, line.to);
      const left = Math.max((this.view.left ?? 0) + (this.view.gutter ?? 48), (this.view.left ?? 0) + (this.view.gutter ?? 48) + this.layout.advance(line, start) - this.view.scrollLeft);
      const endX = (this.view.left ?? 0) + (this.view.gutter ?? 48) + this.layout.advance(line, end) - this.view.scrollLeft + (to > line.to ? 8 : 0);
      const right = Math.min((this.view.left ?? 0) + this.view.width, endX);
      const top = (this.view.top ?? 0) + line.number * this.layout.font.lineHeight - this.view.scrollTop;
      if (right >= left) out.push({ left, right: Math.max(left + 1, right), top, bottom: top + this.layout.font.lineHeight });
    }
    return out;
  }
  private drawRun(run: ShapedRun, x: number, y: number, styles: readonly CodeAnnotation[], color?: string, gutter = false): void {
    const atlas = this.atlas!; const step = atlas.tilePixels / this.scale;
    const left = gutter ? 0 : this.view.gutter ?? 48; const right = gutter ? this.view.gutter ?? 48 : this.view.width;
    const first = Math.max(0, Math.floor((left - x) / step));
    const last = Math.min(Math.ceil(run.width / step), Math.ceil((right - x) / step));
    const fullRunRaster = atlas.supportsRunRaster(run, this.layout.font, this.scale);
    const runStyles: RunStyle[] = fullRunRaster
      ? styles.filter(a => a.kind === 'syntax' && a.to > run.from && a.from < run.to)
        .map(a => ({ from: Math.max(a.from, run.from), to: Math.min(a.to, run.to), color: rgbaCss(this.palette.token[a.className ?? ''] ?? this.palette.text) }))
      : [];
    // Draw during rebuild before a later miss can evict this tile from the atlas.
    for (let i = first; i < last; i++) {
      const offset = i * step; const width = Math.min(step, run.width - offset);
      if (width <= 0) continue;
      let tileStyles = runStyles;
      if (!fullRunRaster && styles.length) {
        const from = this.layout.offsetInRun(run, offset, -1);
        const to = this.layout.offsetInRun(run, offset + width, 1);
        tileStyles = styles.filter(a => a.kind === 'syntax' && a.to > from && a.from < to)
          .map(a => ({ from: Math.max(a.from, run.from), to: Math.min(a.to, run.to), color: rgbaCss(this.palette.token[a.className ?? ''] ?? this.palette.text) }));
      }
      const tile = atlas.tile({ run, font: this.layout.font, dpr: this.scale, x: offset, width, styles: tileStyles, color: color ?? rgbaCss(this.palette.text) });
      if (!tile) { if (!this.pendingText) this.pendingStartLine = (this.currentLineIndex + 1) % Math.max(1, this.visibleLines.length); this.pendingText = true; continue; }
      this.quad({ left: x + offset, right: x + offset + width, top: y, bottom: y + this.layout.font.lineHeight }, [1, 1, 1, 1], tile.texture,
        [tile.cell.u0, tile.cell.v0, tile.cell.u1, tile.cell.v1]);
    }
  }
  private local(r: CodeRect): CodeRect { return { left: r.left - (this.view.left ?? 0), right: r.right - (this.view.left ?? 0), top: r.top - (this.view.top ?? 0), bottom: r.bottom - (this.view.top ?? 0) }; }
  private clip(gutter: number): void {
    const gl = this.gl!; gl.enable(gl.SCISSOR_TEST);
    this.scissorOn = true;
    const left = Math.min(this.canvas.width, Math.max(0, Math.floor(gutter * this.scale)));
    gl.scissor(left, 0, this.canvas.width - left, this.canvas.height);
  }
  private unclip(): void {
    this.gl!.disable(this.gl!.SCISSOR_TEST);
    this.scissorOn = false;
  }
  private quad(r: CodeRect, color: number[], texture = this.white, uv?: [number, number, number, number]): void {
    if (r.right <= 0 || r.left >= this.view.width || r.bottom <= 0 || r.top >= this.view.height) return;
    this.addCommand({ rect: r, color, texture: texture ?? undefined, ...(uv ? { uv } : {}) });
  }
  private reportReady(): void { this.report(this.scale < this.requestedDpr ? 'degraded' : 'ready', this.scale < this.requestedDpr ? 'Effective DPR reduced to respect GPU budget' : 'GPU ready'); }
  private report(kind: GpuStatus['kind'], message: string): void {
    const changed = this.statusValue.kind !== kind || this.statusValue.message !== message || this.statusValue.effectiveDpr !== this.scale;
    this.statusValue = { kind, message, effectiveDpr: this.scale, saveText: () => this.layout.document };
    if (changed) this.options.onStatus?.(this.statusValue);
  }
  private fail(error: unknown): void { this.clearDrawCache(); this.releaseGpu(false); this.report('unavailable', error instanceof Error ? error.message : String(error)); }
  private clearDrawCache(): void { this.beforeAnimation = null; this.afterAnimation = null; this.cacheKey = ''; this.collecting = null; this.drawWhileCollecting = false; this.visibleLines = []; }
  private releaseGpu(lost: boolean): void {
    this.clearDrawCache();
    const gl = this.gl;
    if (lost) this.atlas?.contextLost(); else this.atlas?.dispose(); this.atlas = null;
    if (gl && !lost) {
      if (this.white) gl.deleteTexture(this.white);
      if (this.buffer) gl.deleteBuffer(this.buffer);
      if (this.vao) gl.deleteVertexArray(this.vao);
      if (this.program) gl.deleteProgram(this.program);
    }
    this.target?.release(); this.target = null;
    for (const allocation of this.allocations) allocation.release(); this.allocations = [];
    this.program = null; this.buffer = null; this.white = null; this.vao = null;
    this.canvas.width = 0; this.canvas.height = 0;
  }
  dispose(): void {
    if (this.disposed) return; this.disposed = true;
    this.textureGeneration++;
    this.canvas.removeEventListener('webglcontextlost', this.loss); this.canvas.removeEventListener('webglcontextrestored', this.restore);
    document.fonts?.removeEventListener('loadingdone', this.fontsChanged);
    this.releaseGpu(this.lost); this.labels.clear(); this.layout.invalidate();
  }
}
