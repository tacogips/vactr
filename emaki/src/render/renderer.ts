import { Text } from '@codemirror/state';
import type { Rect as CodeRect, TokenSpan } from '../types';
import type { GpuStatus, RenderInput } from './types';
import { tokenKey, writeSlots, styleHash, gutterIdentity, rebuildTextSegments, drawRun, indexTextDecorations, lineDecorations, fontVariantsForLine, type DecorationIndex, type TextDecoration, type TextPassContext, type DrawCommand } from './text-pass';
import { drawDecorations, type DecorationContext } from './decoration-pass';
import { GlyphAtlas, type CanvasFactory } from './atlas';
import { TextLayout, type LayoutViewport, type ShapedLine, type ShapedRun } from '../layout/layout';
import { ResourceLedger, effectiveSize, RESOURCE_LIMITS, type Reservation } from './resources';
import type { PhaseTimer } from './frame';
import { FALLBACK_PALETTE, type Palette } from './palette';
import { InstanceWriter, LayerBuffer, VERTEX_SOURCE, FRAGMENT_SOURCE, type GeometryLayer } from './geometry';
import { SegmentCache, MAX_TEXT_BLOCKS } from './segments';

export interface RendererOptions {
  ledger?: ResourceLedger; gl?: WebGL2RenderingContext; createCanvas?: CanvasFactory;
  palette?: Palette;
  onStatus?: (status: GpuStatus) => void;
}
const LAYERS: readonly GeometryLayer[] = ['background', 'text', 'overlay', 'overlayText'];
/** GPU presentation only. Mount/input owns editing, callbacks, scrolling and frame scheduling. */
export class CanvasRenderer {
  readonly ledger: ResourceLedger;
  readonly stats = { frames: 0, textBuilds: 0, gutterBuilds: 0, bufferUploads: 0, geometryBytes: 0, slotTableBytes: 0, compactions: 0, drawCalls: 0, lastFrameDraws: 0, decodedTextInstances: 0 };
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
  private textCommands: DrawCommand[] | null = null;
  private textLayerDirty = true;
  private cacheKey = '';
  private decorationIndex: DecorationIndex = indexTextDecorations([], []);
  private decorationIndexKey: string | undefined;
  private textureGeneration = 0;
  private seenEvictions = 0;
  private collecting: DrawCommand[] | null = null;
  private currentLayer: GeometryLayer = 'background';
  private currentSlot = 0;
  private writers = new Map<GeometryLayer, InstanceWriter>(LAYERS.map(layer => [layer, new InstanceWriter()]));
  private layerBuffers = new Map<GeometryLayer, LayerBuffer>();
  private backgroundKey = '';
  private backgroundOpacity = 0.85;
  private backgroundStaticCount = 0;
  private overlayKey = '';
  private overlayStaticCount = 0;
  private overlayTextStaticCount = 0;
  private readonly segments = new SegmentCache<DrawCommand[]>();
  private readonly pendingSegments = new Set<string>();
  private readonly gutterIdentities = new Map<number, readonly ShapedRun[]>();
  private readonly lineNumbersByRuns = new WeakMap<object, number>();
  private readonly shiftedGutterIdentities = new WeakMap<object, readonly ShapedRun[]>();
  private readonly lineSlots = new WeakMap<object, number>();
  private readonly slotOwners = new Map<number, object>();
  private readonly liveSlots = new Map<number, string>();
  private readonly freeSlots: number[] = [];
  private nextSlot = 1;
  private uploadedText = new Uint8Array();
  private textHighWaterBlocks = 0;
  private atlasGenerationSeen = -1;
  private cornerBuffer: WebGLBuffer | null = null;
  private slotTexture: WebGLTexture | null = null;
  private pendingText = false;
  private currentSegmentPending = false;
  private textRevisionCounter = 0;
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
  setDocument(text: string): void { this.layout.setDocument(text); this.textRevisionCounter++; this.clearFrameCache(); }
  setText(doc: Text): void { this.layout.setText(doc); this.textRevisionCounter++; this.clearFrameCache(); }
  setPalette(palette: Palette): void { this.palette = palette; this.clearDrawCache(); }
  setBackgroundOpacity(opacity: number): void {
    if (!Number.isFinite(opacity) || opacity < 0 || opacity > 1) throw new RangeError('Invalid editor background opacity');
    if (opacity === this.backgroundOpacity) return;
    this.backgroundOpacity = opacity;
    this.backgroundKey = '';
  }
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
    const slotBudget = this.ledger.allocate('geometry', this.maxTextureSize * 8);
    const white = this.ledger.allocate('atlas', 4);
    if (!geometry || !slotBudget || !white) { geometry?.release(); slotBudget?.release(); white?.release(); throw new Error('GPU initialization budget exhausted'); }
    this.allocations.push(geometry, slotBudget, white);
    const shaders: WebGLShader[] = [];
    try {
      for (const [type, source] of [[gl.VERTEX_SHADER, VERTEX_SOURCE], [gl.FRAGMENT_SHADER, FRAGMENT_SOURCE]] as const) {
        const shader = gl.createShader(type); if (!shader) throw new Error('Shader allocation failed');
        shaders.push(shader); gl.shaderSource(shader, source); gl.compileShader(shader);
        if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(shader) ?? 'Shader compile failed');
      }
      this.program = gl.createProgram(); if (!this.program) throw new Error('Program allocation failed');
      for (const shader of shaders) gl.attachShader(this.program, shader);
      gl.linkProgram(this.program); if (!gl.getProgramParameter(this.program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(this.program) ?? 'Program link failed');
      this.vao = gl.createVertexArray(); this.white = gl.createTexture(); this.cornerBuffer = gl.createBuffer(); this.slotTexture = gl.createTexture();
      if (!this.vao || !this.white || !this.cornerBuffer || !this.slotTexture) throw new Error('GPU object allocation failed');
      gl.bindVertexArray(this.vao);
      gl.bindBuffer(gl.ARRAY_BUFFER, this.cornerBuffer);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 1]), gl.STATIC_DRAW);
      const corner = gl.getAttribLocation(this.program, 'a_corner'); gl.enableVertexAttribArray(corner); gl.vertexAttribPointer(corner, 2, gl.FLOAT, false, 0, 0);
      for (const layer of LAYERS) {
        const buffer = gl.createBuffer(); if (!buffer) throw new Error('Layer buffer allocation failed');
        this.layerBuffers.set(layer, new LayerBuffer(gl, this.ledger, buffer, layer === 'text' ? MAX_TEXT_BLOCKS * 2048 : Number.MAX_SAFE_INTEGER));
      }
      const stride = 32;
      const attrs: Array<[string, number, number, number]> = [['a_rect', 4, gl.FLOAT, 0], ['a_uv', 4, gl.UNSIGNED_SHORT, 16], ['a_color', 4, gl.UNSIGNED_BYTE, 24]];
      for (const [name, size, type, offset] of attrs) {
        const loc = gl.getAttribLocation(this.program, name); gl.enableVertexAttribArray(loc);
        gl.vertexAttribPointer(loc, size, type, name !== 'a_rect', stride, offset); gl.vertexAttribDivisor(loc, 1);
      }
      const meta = gl.getAttribLocation(this.program, 'a_meta'); gl.enableVertexAttribArray(meta); gl.vertexAttribIPointer(meta, 2, gl.UNSIGNED_SHORT, stride, 28); gl.vertexAttribDivisor(meta, 1);
      gl.bindTexture(gl.TEXTURE_2D, this.slotTexture); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RG32F, this.maxTextureSize, 1, 0, gl.RG, gl.FLOAT, new Float32Array(this.maxTextureSize * 2));
      this.locations.u_slots = gl.getUniformLocation(this.program, 'u_slots'); this.locations.u_gutter = gl.getUniformLocation(this.program, 'u_gutter');
      gl.bindTexture(gl.TEXTURE_2D, this.white);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array([255, 255, 255, 255]));
      for (const name of ['u_view', 'u_device', 'u_texture']) this.locations[name] = gl.getUniformLocation(this.program, name);
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
  render(feedback: RenderInput = {}): boolean {
    if (this.disposed || this.lost || !this.gl || !this.program || !this.atlas) return false;
    try {
      if (!this.target) this.resize(this.requestedDpr);
      const gl = this.gl; this.atlas.beginFrame(); this.pendingText = false;
      if (this.atlas.stats.evictions !== this.seenEvictions) { this.textureGeneration++; this.seenEvictions = this.atlas.stats.evictions; }
      if (this.atlasGenerationSeen !== this.atlas.stats.generation) {
        this.atlasGenerationSeen = this.atlas.stats.generation;
        this.segments.clear(); this.pendingSegments.clear(); this.textHighWaterBlocks = 0; this.uploadedText = new Uint8Array(); this.textLayerDirty = true;
      }
      gl.bindFramebuffer(gl.FRAMEBUFFER, null); gl.bindVertexArray(this.vao); gl.useProgram(this.program);
      gl.viewport(0, 0, this.canvas.width, this.canvas.height);
      gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
      gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT);
      gl.uniform2f(this.locations.u_view, this.view.width, this.view.height); gl.uniform2f(this.locations.u_device, this.canvas.width / this.view.width, this.canvas.height / this.view.height); gl.uniform1i(this.locations.u_texture, 0); gl.uniform1i(this.locations.u_slots, 1); gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, this.atlas.texture);
      const tokens = feedback.tokens ?? [];
      if (this.decorationIndexKey !== feedback.decorationsKey || this.textCommands === null) {
        this.decorationIndex = indexTextDecorations(feedback.marks ?? [], feedback.lineFormats ?? []);
        this.decorationIndexKey = feedback.decorationsKey;
      }
      const viewportKey = `${this.view.width},${this.view.height},${this.view.scrollLeft},${this.view.scrollTop},${this.view.left ?? 0},${this.view.top ?? 0},${this.view.gutter ?? 48}|${this.scale}|${this.layout.font.generation ?? 0}|${this.atlas.stats.generation}|${this.layout.heightmap.revision}`;
      const key = `${feedback.textRevision ?? this.textRevisionCounter}|${viewportKey}|${tokenKey(tokens)}|${this.decorationIndex.key}`;
      const selectionKey = (feedback.selection ?? []).map(a => `${a.from}:${a.to}`).join(';');
      const backgroundKey = `${viewportKey}|${feedback.textRevision ?? this.textRevisionCounter}|${feedback.decorationsKey ?? ''}|${selectionKey}`;
      const handlesKey = (feedback.handles ?? []).map(handle => `${handle.pos}:${handle.end ? 1 : 0}`).join(',');
      const overlayKey = `${backgroundKey}|${feedback.cursor ?? ''}|${feedback.cursorVisible !== false}|${handlesKey}|${feedback.cursorShape ?? 'bar'}|${feedback.cursorEnd ?? ''}|${feedback.hopRevision ?? ''}`;
      const backgroundStaticDirty = this.backgroundKey !== backgroundKey;
      const overlayDirty = this.overlayKey !== overlayKey;
      for (const [layer, writer] of this.writers) {
        if (layer === 'text') continue;
        if (layer === 'background') {
          if (backgroundStaticDirty) writer.clear(); else writer.setCount(this.backgroundStaticCount);
        } else if (overlayDirty) writer.clear();
        else if (layer === 'overlay') writer.setCount(this.overlayStaticCount);
        else if (layer === 'overlayText') writer.setCount(this.overlayTextStaticCount);
      }
      const rebuildText = this.textCommands === null || this.cacheKey !== key;
      this.currentLayer = 'background';
      if (backgroundStaticDirty) this.quad({ left: 0, right: this.view.width, top: 0, bottom: this.view.height }, [...this.palette.background.slice(0, 3), this.backgroundOpacity]);
      if (rebuildText) {
        const lineHeight = this.layout.font.lineHeight;
        const overscan = Math.ceil(this.view.height / lineHeight) * lineHeight;
        const overscanTop = Math.max(0, this.view.scrollTop - overscan);
        const overscanBottom = this.view.scrollTop + this.view.height + overscan;
        const shapeView = { ...this.view, scrollTop: overscanTop, height: overscanBottom - overscanTop };
        this.visibleLines = this.layout.visible(shapeView);
        const variants = new Map(this.visibleLines.map(line => [line.number, fontVariantsForLine(this.layout, line, lineDecorations(this.decorationIndex, line))]));
        this.layout.setFontVariants(variants);
        this.visibleLines = this.layout.visible(shapeView);
        this.rebuildTextSegments(tokens);
        if (!this.pendingText) { this.cacheKey = key; this.pendingStartLine = 0; } else this.cacheKey = '';
      }
      drawDecorations.call(this as unknown as DecorationContext, feedback, backgroundStaticDirty, overlayDirty);
      if (backgroundStaticDirty) this.backgroundKey = backgroundKey;
      if (overlayDirty) this.overlayKey = overlayKey;
      this.collecting = null; this.flushLayers(backgroundStaticDirty, overlayDirty);
      this.stats.frames++;
      if (this.visibleLines.some(line => line.rtlUnsupported)) this.report('degraded', 'Mixed RTL hit testing is unsupported'); else this.reportReady();
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
  private addCommand(command: Omit<DrawCommand, 'layer' | 'slot'> & { flags?: number }): void {
    const recorded: DrawCommand = { ...command, flags: command.flags ?? 0, layer: this.currentLayer, slot: this.currentLayer === 'text' ? this.currentSlot : 0, ...(command.texture ? { generation: this.textureGeneration } : {}) };
    if (this.collecting) {
      this.collecting.push(recorded);
      return;
    }
    this.drawCommand(recorded);
  }
  private drawCommand(command: DrawCommand): void {
    if (command.texture && command.generation !== this.textureGeneration) return;
    const uv = command.uv ?? [0, 0, 2 / this.atlas!.dimensions[0], 2 / this.atlas!.dimensions[1]];
    this.writers.get(command.layer)!.push(command.rect, uv, command.color as [number, number, number, number], command.slot, command.flags);
  }
  private writeSlots(lines: readonly ShapedLine[]): void { writeSlots.call(this as unknown as TextPassContext, lines); }
  private styleHash(line: ShapedLine, styles: readonly TokenSpan[], from = line.from, to = line.to): string { return styleHash.call(this as unknown as TextPassContext, line, styles, from, to); }
  private gutterIdentity(line: number): readonly ShapedRun[] { return gutterIdentity.call(this as unknown as TextPassContext, line); }
  private rebuildTextSegments(styles: readonly TokenSpan[]): void { rebuildTextSegments.call(this as unknown as TextPassContext, styles); }
  private invalidateSegments(): void {
    this.segments.clear(); this.pendingSegments.clear(); this.liveSlots.clear();
    for (const [slot, owner] of this.slotOwners) {
      this.lineSlots.delete(owner); this.freeSlots.push(slot);
    }
    this.slotOwners.clear();
    this.uploadedText = new Uint8Array(); this.textHighWaterBlocks = 0;
  }
  private flushLayers(backgroundStaticDirty: boolean, overlayDirty: boolean): void {
    const gl = this.gl!;
    const drawStart = this.stats.drawCalls;
    gl.uniform1f(this.locations.u_gutter, (this.view.gutter ?? 48));
    gl.activeTexture(gl.TEXTURE1); gl.bindTexture(gl.TEXTURE_2D, this.slotTexture);
    gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, this.atlas!.texture);
    for (const layer of LAYERS) {
      const writer = this.writers.get(layer)!;
      if (!writer.count) {
        if (layer === 'text' && this.textLayerDirty) this.textLayerDirty = false;
        continue;
      }
      const buffer = this.layerBuffers.get(layer)!.buffer;
      if (layer === 'background') {
        if (backgroundStaticDirty) this.layerBuffers.get(layer)!.upload(writer.bytes());
        else if (writer.count > this.backgroundStaticCount) this.layerBuffers.get(layer)!.uploadRange(
          this.backgroundStaticCount * 32, writer.bytes().subarray(this.backgroundStaticCount * 32));
      } else if (layer === 'overlay' || layer === 'overlayText') {
        if (overlayDirty) this.layerBuffers.get(layer)!.upload(writer.bytes());
        else if (writer.count > (layer === 'overlay' ? this.overlayStaticCount : this.overlayTextStaticCount)) {
          const offset = (layer === 'overlay' ? this.overlayStaticCount : this.overlayTextStaticCount) * 32;
          this.layerBuffers.get(layer)!.uploadRange(offset, writer.bytes().subarray(offset));
        }
      } else if (this.textLayerDirty) {
        this.layerBuffers.get(layer)!.upload(writer.bytes());
      }
      if (layer === 'text' && this.textLayerDirty) {
        if (layer === 'text') {
          this.stats.bufferUploads++;
          this.textLayerDirty = false;
        }
      }
      gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
      const stride = 32;
      for (const [name, size, type, normalized, offset] of [['a_rect', 4, gl.FLOAT, false, 0], ['a_uv', 4, gl.UNSIGNED_SHORT, true, 16], ['a_color', 4, gl.UNSIGNED_BYTE, true, 24]] as const) {
        const loc = gl.getAttribLocation(this.program!, name); gl.vertexAttribPointer(loc, size, type, normalized, stride, offset);
      }
      const meta = gl.getAttribLocation(this.program!, 'a_meta'); gl.vertexAttribIPointer(meta, 2, gl.UNSIGNED_SHORT, stride, 28);
      gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, writer.count); this.stats.drawCalls++;
      if (layer === 'text') this.stats.decodedTextInstances = writer.count;
    }
    this.stats.lastFrameDraws = this.stats.drawCalls - drawStart;
  }
  private drawRun(run: ShapedRun, x: number, y: number, styles: readonly TokenSpan[], color?: string, gutter = false, decorations: readonly TextDecoration[] = []): void { drawRun.call(this as unknown as TextPassContext, run, x, y, styles, color, gutter, decorations); }
  private local(r: CodeRect): CodeRect { return { left: r.left - (this.view.left ?? 0), right: r.right - (this.view.left ?? 0), top: r.top - (this.view.top ?? 0), bottom: r.bottom - (this.view.top ?? 0) }; }
  private quad(r: CodeRect, color: number[], texture = this.white, uv?: [number, number, number, number], flags = 0): void {
    if (r.right <= 0 || r.left >= this.view.width || r.bottom <= 0 || r.top >= this.view.height) return;
    this.addCommand({ rect: r, color, texture: texture ?? undefined, ...(uv ? { uv } : {}), flags });
  }
  private reportReady(): void { this.report(this.scale < this.requestedDpr ? 'degraded' : 'ready', this.scale < this.requestedDpr ? 'Effective DPR reduced to respect GPU budget' : 'GPU ready'); }
  private report(kind: GpuStatus['kind'], message: string): void {
    const changed = this.statusValue.kind !== kind || this.statusValue.message !== message || this.statusValue.effectiveDpr !== this.scale;
    this.statusValue = { kind, message, effectiveDpr: this.scale, saveText: () => this.layout.document };
    if (changed) this.options.onStatus?.(this.statusValue);
  }
  private fail(error: unknown): void { this.clearDrawCache(); this.releaseGpu(false); this.report('unavailable', error instanceof Error ? error.message : String(error)); }
  private clearFrameCache(): void { this.textCommands = null; this.cacheKey = ''; this.collecting = null; this.visibleLines = []; }
  private clearDrawCache(): void {
    this.clearFrameCache(); this.invalidateSegments(); this.textLayerDirty = true;
    this.backgroundKey = ''; this.backgroundStaticCount = 0; this.overlayKey = '';
  }
  private releaseGpu(lost: boolean): void {
    this.clearDrawCache();
    const gl = this.gl;
    if (lost) this.atlas?.contextLost(); else this.atlas?.dispose(); this.atlas = null;
    for (const layer of this.layerBuffers.values()) {
      layer.dispose();
      if (gl && !lost) gl.deleteBuffer(layer.buffer);
    }
    this.layerBuffers.clear();
    if (gl && !lost) {
      if (this.white) gl.deleteTexture(this.white);
      if (this.cornerBuffer) gl.deleteBuffer(this.cornerBuffer);
      if (this.slotTexture) gl.deleteTexture(this.slotTexture);
      if (this.vao) gl.deleteVertexArray(this.vao);
      if (this.program) gl.deleteProgram(this.program);
    }
    this.target?.release(); this.target = null;
    for (const allocation of this.allocations) allocation.release(); this.allocations = [];
    this.program = null; this.buffer = null; this.white = null; this.vao = null; this.cornerBuffer = null; this.slotTexture = null;
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
