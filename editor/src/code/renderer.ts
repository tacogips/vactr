import { Text } from '@codemirror/state';
import type { CodeAnnotation, CodeRect } from '../app/apis';
import { GlyphAtlas, type CanvasFactory } from './atlas';
import { TextLayout, type LayoutViewport, type ShapedLine, type ShapedRun } from './layout';
import { ResourceLedger, effectiveSize, RESOURCE_LIMITS, type Reservation } from './resources';
import type { PhaseTimer } from './frame';
import { FALLBACK_PALETTE, rgbaCss, type Palette } from './palette';
import { CLIP_GUTTER, UNTINTED, InstanceWriter, LayerBuffer, VERTEX_SOURCE, FRAGMENT_SOURCE, type GeometryLayer } from './geometry';
import { SegmentCache, MAX_TEXT_BLOCKS, type SegmentKey } from './segments';

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
interface DrawCommand { rect: CodeRect; color: number[]; texture?: WebGLTexture; uv?: [number, number, number, number]; generation?: number; cursor?: boolean; flags?: number; layer: GeometryLayer; slot: number }
export interface RendererOptions {
  ledger?: ResourceLedger; gl?: WebGL2RenderingContext; createCanvas?: CanvasFactory;
  palette?: Palette;
  onStatus?: (status: GpuStatus) => void;
}
const LAYERS: readonly GeometryLayer[] = ['background', 'text', 'overlay', 'overlayText'];
export const GPU_TOKEN_COLORS: Readonly<Record<string, string>> = Object.freeze({
  'vact-tok-comment': '#7a7f87', 'vact-tok-directive': '#b07bd8', 'vact-tok-keyword': '#d08770',
  'vact-tok-number': '#88c0d0', 'vact-tok-string': '#a3be8c', 'vact-tok-path': '#a3be8c',
  'vact-tok-head': '#ebcb8b', 'vact-tok-bracket': '#8a8f98',
});

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
  private textureGeneration = 0;
  private seenEvictions = 0;
  private collecting: DrawCommand[] | null = null;
  private currentLayer: GeometryLayer = 'background';
  private currentSlot = 0;
  private writers = new Map<GeometryLayer, InstanceWriter>(LAYERS.map(layer => [layer, new InstanceWriter()]));
  private layerBuffers = new Map<GeometryLayer, LayerBuffer>();
  private backgroundKey = '';
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
      for (const name of ['u_view', 'u_texture']) this.locations[name] = gl.getUniformLocation(this.program, name);
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
      gl.uniform2f(this.locations.u_view, this.view.width, this.view.height); gl.uniform1i(this.locations.u_texture, 0); gl.uniform1i(this.locations.u_slots, 1); gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, this.atlas.texture);
      const annotations = feedback.annotations ?? [];
      const staticAnnotations = annotations.filter(a => a.kind !== 'playing' && a.kind !== 'eval');
      const syntaxKey = staticAnnotations.filter(a => a.kind === 'syntax').map(a => `${a.from}:${a.to}:${a.className ?? ''}`).join(';');
      const key = `${feedback.textRevision ?? this.textRevisionCounter}|${this.view.width},${this.view.height},${this.view.scrollLeft},${this.view.scrollTop},${this.view.left ?? 0},${this.view.top ?? 0},${this.view.gutter ?? 48}|${this.scale}|${this.layout.font.generation ?? 0}|${this.atlas.stats.generation}|${syntaxKey}`;
      const viewportKey = `${this.view.width},${this.view.height},${this.view.scrollLeft},${this.view.scrollTop},${this.view.left ?? 0},${this.view.top ?? 0},${this.view.gutter ?? 48}|${this.scale}|${this.layout.font.generation ?? 0}|${this.atlas.stats.generation}`;
      const selectionKey = staticAnnotations.filter(a => a.kind === 'selection').map(a => `${a.from}:${a.to}`).join(';');
      const backgroundKey = `${viewportKey}|${feedback.textRevision ?? this.textRevisionCounter}|${feedback.annotationsRevision ?? 0}|${selectionKey}`;
      const handlesKey = (feedback.handles ?? []).map(handle => `${handle.pos}:${handle.end ? 1 : 0}`).join(',');
      const overlayKey = `${backgroundKey}|${feedback.annotationsRevision ?? 0}|${feedback.cursor ?? ''}|${feedback.cursorVisible !== false}|${handlesKey}`;
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
      if (backgroundStaticDirty) this.quad({ left: 0, right: this.view.width, top: 0, bottom: this.view.height }, [0.04, 0.05, 0.07, 0.85]);
      if (rebuildText) {
        const lineHeight = this.layout.font.lineHeight;
        const overscan = Math.ceil(this.view.height / lineHeight) * lineHeight;
        const overscanTop = Math.max(0, this.view.scrollTop - overscan);
        const overscanBottom = this.view.scrollTop + this.view.height + overscan;
        this.visibleLines = this.layout.visible({ ...this.view, scrollTop: overscanTop, height: overscanBottom - overscanTop });
        this.rebuildTextSegments(staticAnnotations);
        if (!this.pendingText) { this.cacheKey = key; this.pendingStartLine = 0; }
        else this.cacheKey = '';
      }
      this.currentLayer = 'background';
      if (backgroundStaticDirty) {
        for (const a of staticAnnotations) if (a.kind === 'selection') for (const r of this.layout.rangeRects(a, this.view)) this.quad(this.local(r), [0.12, 0.22, 0.32, 0.55]);
        this.backgroundStaticCount = this.writers.get('background')!.count;
        this.backgroundKey = backgroundKey;
      }
      for (const a of [...annotations, ...(feedback.animated ?? [])]) if (a.kind === 'playing' || a.kind === 'eval') {
        const color = a.kind === 'playing' ? [0.35, 0.29, 0.13, 0.5] : a.className?.includes('error') ? [0.4, 0.12, 0.16, 0.5] : [0.2, 0.32, 0.36, 0.5];
        for (const r of this.animatedRects(a)) this.quad(this.local(r), color);
      }
      this.currentLayer = 'overlay';
      if (overlayDirty) for (const a of staticAnnotations) {
        if (a.kind === 'diagnostic' || a.kind === 'composition') for (const r of this.layout.rangeRects(a, this.view)) this.quad(this.local({ ...r, top: r.bottom - 2 }), a.kind === 'diagnostic' ? this.palette.diagnostic : this.palette.composition);
        if (a.kind === 'call-head') for (const r of this.layout.rangeRects(a, this.view)) this.quad(this.local({ ...r, top: r.bottom - 1 }), this.palette.callHead);
      }
      if (overlayDirty && feedback.cursorVisible !== false && feedback.cursor != null) {
        const r = this.layout.coordsAtPos(feedback.cursor, this.view); if (r) this.quad(this.local(r), this.palette.cursor);
      }
      if (overlayDirty) for (const a of staticAnnotations) {
        if (a.kind === 'binding' && a.label) {
          const r = this.layout.coordsAtPos(a.to, this.view); if (!r) continue;
          const x = r.left - (this.view.left ?? 0) + 3, y = r.top - (this.view.top ?? 0);
          const width = Math.min(256, Math.max(16, a.label.length * 8 + 8));
          this.quad({ left: x, right: x + width, top: y, bottom: y + this.layout.font.lineHeight }, this.palette.labelFill);
          this.currentLayer = 'overlayText'; this.drawRun(this.labelRun(a.label, width), x + 4, y, [], rgbaCss(this.palette.labelText)); this.currentLayer = 'overlay';
        }
      }
      this.currentLayer = 'overlay';
      if (overlayDirty) for (const handle of feedback.handles ?? []) {
        const r = this.layout.coordsAtPos(handle.pos, this.view); if (!r) continue;
        const x = r.left - (this.view.left ?? 0), y = (handle.end ? r.bottom : r.top) - (this.view.top ?? 0);
        this.quad({ left: x - 5, right: x + 5, top: y - 4, bottom: y + 6 }, this.palette.handle);
      }
      if (overlayDirty) {
        this.overlayStaticCount = this.writers.get('overlay')!.count;
        this.overlayTextStaticCount = this.writers.get('overlayText')!.count;
      }
      const momentary = (feedback.animated ?? []).filter(a => a.kind === 'momentary' && a.label);
      if (momentary.length) {
        this.currentLayer = 'overlay';
        for (const a of momentary) {
          const r = this.layout.coordsAtPos(a.to, this.view); if (!r) continue;
          const x = r.left - (this.view.left ?? 0) + 3, y = r.top - (this.view.top ?? 0);
          const width = Math.min(256, Math.max(16, a.label!.length * 8 + 8));
          this.quad({ left: x, right: x + width, top: y, bottom: y + this.layout.font.lineHeight }, this.palette.labelFill);
          this.currentLayer = 'overlayText'; this.drawRun(this.labelRun(a.label!, width), x + 4, y, [], rgbaCss(this.palette.labelText)); this.currentLayer = 'overlay';
        }
      }
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
    const recorded: DrawCommand = { ...command, flags: command.flags ?? 0, layer: this.currentLayer, slot: this.currentSlot, ...(command.texture ? { generation: this.textureGeneration } : {}) };
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
  private writeSlots(lines: readonly ShapedLine[]): void {
    if (lines.length + 1 > this.maxTextureSize) throw new Error('Visible line slot table exceeds texture limit');
    const active = new Set<number>(), changed = new Map<number, [number, number]>();
    for (const line of lines) {
      const identity = line.runs as object;
      let slot = this.lineSlots.get(identity);
      if (slot !== undefined && this.slotOwners.get(slot) !== identity) {
        this.lineSlots.delete(identity); slot = undefined;
      }
      if (slot === undefined) {
        while (slot === undefined) {
          const available = this.freeSlots.pop();
          if (available === undefined) slot = this.nextSlot++;
          else if (!this.slotOwners.has(available)) slot = available;
        }
        this.lineSlots.set(identity, slot); this.slotOwners.set(slot, identity);
      }
      active.add(slot);
      if (slot >= Math.min(1024, this.maxTextureSize)) { this.pendingText = true; continue; }
      const x = -this.view.scrollLeft, y = line.number * this.layout.font.lineHeight - this.view.scrollTop;
      const position = `${x}:${y}`;
      if (this.liveSlots.get(slot) !== position) {
        changed.set(slot, [x, y]);
        this.liveSlots.set(slot, position);
      }
    }
    for (const slot of this.slotOwners.keys()) if (!active.has(slot)) {
      if (this.liveSlots.has(slot)) changed.set(slot, [0, 0]);
      this.liveSlots.delete(slot);
      const owner = this.slotOwners.get(slot);
      if (owner) this.lineSlots.delete(owner);
      this.slotOwners.delete(slot); this.freeSlots.push(slot);
    }
    const slots = [...changed.keys()].sort((a, b) => a - b);
    const gl = this.gl!; gl.activeTexture(gl.TEXTURE1); gl.bindTexture(gl.TEXTURE_2D, this.slotTexture);
    for (let at = 0; at < slots.length;) {
      const start = slots[at]!; let end = at + 1;
      while (end < slots.length && slots[end] === slots[end - 1]! + 1) end++;
      const values = new Float32Array((end - at) * 2);
      for (let index = at; index < end; index++) values.set(changed.get(slots[index]!)!, (index - at) * 2);
      gl.texSubImage2D(gl.TEXTURE_2D, 0, start, 0, end - at, 1, gl.RG, gl.FLOAT, values);
      this.stats.slotTableBytes += values.byteLength; at = end;
    }
  }
  private styleHash(line: ShapedLine, styles: readonly CodeAnnotation[], from = line.from, to = line.to): string {
    return styles.filter(a => a.kind === 'syntax' && a.from < to && a.to > from)
      .map(a => `${Math.max(0, a.from - line.from)}:${Math.min(line.to, a.to) - line.from}:${a.className ?? ''}`).join(';');
  }
  private gutterIdentity(lineNumber: number): readonly ShapedRun[] {
    let identity = this.gutterIdentities.get(lineNumber);
    if (!identity) { identity = []; this.gutterIdentities.set(lineNumber, identity); }
    return identity;
  }
  private rebuildTextSegments(styles: readonly CodeAnnotation[]): void {
    const lineHeight = this.layout.font.lineHeight;
    const visibleFirst = Math.floor(this.view.scrollTop / lineHeight);
    const visibleLast = Math.ceil((this.view.scrollTop + this.view.height) / lineHeight);
    const rows = this.visibleLines.map(line => {
      const runs = line.runs, identity = runs as object;
      const previousNumber = this.lineNumbersByRuns.get(identity);
      this.lineNumbersByRuns.set(identity, line.number);
      const chunks = this.layout.geometrySegments(line).filter(chunk => {
        const first = chunk.runs[0], last = chunk.runs[chunk.runs.length - 1];
        const from = first?.x ?? 0, to = last ? last.x + last.width : 0;
        return to >= Math.max(0, this.view.scrollLeft - this.view.width) && from <= this.view.scrollLeft + this.view.width * 2;
      });
      const textKeys = chunks.map(chunk => {
        const from = chunk.runs[0]?.from ?? line.from, last = chunk.runs[chunk.runs.length - 1];
        const to = last?.to ?? line.to;
        return { chunk, key: { runs, chunk: chunk.chunk, fontGeneration: this.layout.font.generation ?? 0,
          dpr: this.scale, styleHash: this.styleHash(line, styles, from, to), atlasGeneration: this.textureGeneration } as SegmentKey };
      });
      let shiftedGutter = this.shiftedGutterIdentities.get(identity);
      if (!shiftedGutter && previousNumber !== undefined && previousNumber !== line.number) {
        shiftedGutter = runs; this.shiftedGutterIdentities.set(identity, shiftedGutter);
      }
      const gutterRuns = shiftedGutter ?? this.gutterIdentity(line.number);
      const gutterKey: SegmentKey = { runs: gutterRuns, chunk: -1, fontGeneration: this.layout.font.generation ?? 0,
        dpr: this.scale, styleHash: `gutter:${line.number + 1}`, atlasGeneration: this.textureGeneration };
      return { line, identity, textKeys, gutterKey, canBuild: line.number >= visibleFirst && line.number < visibleLast };
    });
    const visibleNumbers = new Set(rows.map(row => row.line.number));
    for (const number of this.gutterIdentities.keys()) if (!visibleNumbers.has(number)) this.gutterIdentities.delete(number);
    const keep = new Set<string>();
    for (const row of rows) { for (const { key } of row.textKeys) keep.add(this.segments.key(key)); keep.add(this.segments.key(row.gutterKey)); }
    this.writeSlots(this.visibleLines);
    const removed = this.segments.deleteOutside(keep);
    if (removed.length) this.textLayerDirty = true;
    const used = this.segments.usedBlocks;
    if (used > 0 && this.segments.highWaterBlocks > 2 * used) {
      this.segments.compact(); this.stats.compactions++; this.uploadedText = new Uint8Array();
    }
    const writer = this.writers.get('text')!; writer.clear();
    for (let offset = 0; offset < rows.length; offset++) {
      const index = (this.pendingStartLine + offset) % Math.max(1, rows.length), { line, identity, textKeys, gutterKey, canBuild } = rows[index]!;
      const gutterId = this.segments.key(gutterKey);
      if (this.pendingSegments.delete(gutterId)) this.segments.delete(gutterKey);
      let gutter = this.segments.get(gutterKey);
      if (!gutter && canBuild) {
        this.currentSlot = this.lineSlots.get(identity)!; this.currentLayer = 'text'; this.collecting = []; this.currentSegmentPending = false;
        this.drawRun(this.labelRun(String(line.number + 1), (this.view.gutter ?? 48) - 8), 4, 0, [], rgbaCss(this.palette.gutter), true);
        gutter = this.segments.set(gutterKey, this.collecting, 0) ?? undefined;
        if (!gutter) this.pendingText = true;
        else { this.stats.gutterBuilds++; if (this.currentSegmentPending) this.pendingSegments.add(gutterId); }
      }
      let gutterPacked = false;
      for (let chunkIndex = 0; chunkIndex < textKeys.length; chunkIndex++) {
        const { chunk, key: textKey } = textKeys[chunkIndex]!;
        const textId = this.segments.key(textKey);
        if (this.pendingSegments.delete(textId)) this.segments.delete(textKey);
        let segment = this.segments.get(textKey);
        if (!segment && canBuild) {
          this.currentLineIndex = index; this.currentSlot = this.lineSlots.get(identity)!;
          this.currentLayer = 'text'; this.collecting = []; this.currentSegmentPending = false;
          const oldWidth = this.view.width;
          this.view.width = Math.max(oldWidth, (this.view.gutter ?? 48) + line.width + 8);
          const from = chunk.runs[0]?.from ?? line.from, last = chunk.runs[chunk.runs.length - 1];
          const to = last?.to ?? line.to;
          const chunkStyles = styles.filter(style => style.kind !== 'syntax' || (style.from < to && style.to > from));
          for (const run of chunk.runs) this.drawRun(run, (this.view.gutter ?? 48) + run.x, 0, chunkStyles);
          this.view.width = oldWidth;
          const built = this.segments.set(textKey, this.collecting, 0);
          this.collecting = null;
          if (!built) this.pendingText = true;
          segment = built ?? undefined;
          if (built) {
            this.stats.textBuilds++;
            if (this.currentSegmentPending) this.pendingSegments.add(textId);
          }
        }
        if (!segment) continue;
        const commands = chunkIndex === 0 ? [...segment.value, ...(gutter?.value ?? [])] : segment.value;
        if (!this.segments.ensureBlocks(textKey, Math.max(1, Math.ceil(commands.length / 64)))) { this.pendingText = true; continue; }
        let wroteAllCommands = true;
        for (let commandIndex = 0; commandIndex < commands.length; commandIndex++) {
          const command = commands[commandIndex]!;
          const block = segment.blocks[Math.floor(commandIndex / 64)];
          if (block === undefined) { this.pendingText = true; wroteAllCommands = false; break; }
          const gutterCommand = chunkIndex === 0 && commandIndex >= segment.value.length;
          const rect = gutterCommand
            ? { ...command.rect, left: command.rect.left + this.view.scrollLeft, right: command.rect.right + this.view.scrollLeft }
            : command.rect;
          writer.pushAt(block * 64 + commandIndex % 64, rect,
            command.uv ?? [0, 0, 2 / this.atlas!.dimensions[0], 2 / this.atlas!.dimensions[1]],
            command.color as [number, number, number, number], this.lineSlots.get(identity)!, command.flags);
        }
        if (chunkIndex === 0 && gutter && wroteAllCommands) gutterPacked = true;
      }
      if (gutter && gutter.value.length > 0 && !gutterPacked) {
        if (!this.segments.ensureBlocks(gutterKey, Math.max(1, Math.ceil(gutter.value.length / 64)))) this.pendingText = true;
        else for (let commandIndex = 0; commandIndex < gutter.value.length; commandIndex++) {
          const command = gutter.value[commandIndex]!;
          const block = gutter.blocks[Math.floor(commandIndex / 64)];
          if (block === undefined) { this.pendingText = true; break; }
          const rect = { ...command.rect, left: command.rect.left + this.view.scrollLeft, right: command.rect.right + this.view.scrollLeft };
          writer.pushAt(block * 64 + commandIndex % 64, rect,
            command.uv ?? [0, 0, 2 / this.atlas!.dimensions[0], 2 / this.atlas!.dimensions[1]],
            command.color as [number, number, number, number], this.lineSlots.get(identity)!, command.flags);
        }
      }
      this.collecting = null;
    }
    const blocks = Math.min(MAX_TEXT_BLOCKS, this.segments.highWaterBlocks);
    writer.setCount(blocks * 64); this.textHighWaterBlocks = blocks;
    const next = writer.bytes();
    const previous = this.uploadedText;
    const layer = this.layerBuffers.get('text')!;
    const totalBlocks = Math.max(Math.ceil(previous.length / 2048), Math.ceil(next.length / 2048));
    let block = 0;
    while (block < totalBlocks) {
      const offset = block * 2048;
      const a = previous.subarray(offset, offset + 2048), b = next.subarray(offset, offset + 2048);
      if (a.length === b.length && a.every((value, i) => value === b[i])) { block++; continue; }
      const start = block++;
      while (block < totalBlocks) {
        const nextOffset = block * 2048, before = previous.subarray(nextOffset, nextOffset + 2048), after = next.subarray(nextOffset, nextOffset + 2048);
        if (before.length === after.length && before.every((value, i) => value === after[i])) break;
        block++;
      }
      const end = block * 2048, data = new Uint8Array(end - start * 2048);
      data.set(next.subarray(start * 2048, end));
      layer.uploadRange(start * 2048, data); this.stats.bufferUploads++; this.stats.geometryBytes += data.byteLength; this.textLayerDirty = true;
    }
    this.uploadedText = next.slice();
    this.textCommands = [];
    this.stats.decodedTextInstances = writer.count;
    if (!this.pendingText) this.textLayerDirty = false;
  }
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
    const atlas = this.atlas!; const pad = Math.ceil(2 * this.scale); const step = (atlas.maxCellWidth - 2 * pad) / this.scale;
    const left = gutter ? 0 : this.view.gutter ?? 48; const right = gutter ? this.view.gutter ?? 48 : this.view.width;
    if (!this.layout.isRunLine(run)) {
      for (const cluster of this.layout.textCells(run, Math.max(0, left - x - 8), right - x + 8)) {
        const glyphX = x + cluster.x;
        if (glyphX + cluster.width < left || glyphX > right) continue;
        const from = run.from + cluster.from, to = run.from + cluster.to;
        const style = styles.find(a => a.kind === 'syntax' && a.from < to && a.to > from);
        const tint = color ?? (style ? rgbaCss(this.palette.token[style.className ?? ''] ?? this.palette.text) : rgbaCss(this.palette.text));
        const emoji = /\p{Extended_Pictographic}/u.test(cluster.text);
        const cell = atlas.cell({ kind: emoji ? 'color' : 'mask', text: cluster.text, font: this.layout.font, dpr: this.scale, width: cluster.width });
        if (!cell) {
          if (!this.pendingText) this.pendingStartLine = (this.currentLineIndex + 1) % Math.max(1, this.visibleLines.length);
          this.pendingText = true; this.currentSegmentPending = true; continue;
        }
        const flags = (gutter ? 0 : CLIP_GUTTER) | (emoji ? UNTINTED : 0);
        const cssPad = cell.pad / this.scale;
        this.quad({ left: glyphX - cssPad, right: glyphX + cluster.width + cssPad, top: y, bottom: y + this.layout.font.lineHeight }, parseCssColor(tint), atlas.texture,
          [cell.u0, cell.v0, cell.u1, cell.v1], flags);
      }
      return;
    }
    const runStyles = color ? [] : styles.filter(a => a.kind === 'syntax' && a.from < run.to && a.to > run.from);
    const boundaries = [0, run.width];
    for (const style of runStyles) {
      boundaries.push(this.layout.runOffset(run, Math.max(0, style.from - run.from)));
      boundaries.push(this.layout.runOffset(run, Math.min(run.text.length, style.to - run.from)));
    }
    boundaries.sort((a, b) => a - b);
    const edges = boundaries.filter((value, index) => index === 0 || value > boundaries[index - 1]!);
    for (let segment = 0; segment < edges.length - 1; segment++) {
      const segmentFrom = edges[segment]!, segmentTo = edges[segment + 1]!;
      if (segmentTo <= segmentFrom) continue;
      const middle = (segmentFrom + segmentTo) / 2;
      const at = this.layout.offsetInRun(run, middle, 1);
      const style = runStyles.find(a => a.from <= at && a.to > at);
      const tint = color ?? (style ? rgbaCss(this.palette.token[style.className ?? ''] ?? this.palette.text) : rgbaCss(this.palette.text));
      let offset = Math.max(segmentFrom, left - x); const visibleTo = Math.min(segmentTo, right - x);
      while (offset < visibleTo) {
        const width = Math.min(step, segmentTo - offset, visibleTo - offset);
        if (width <= 0) break;
        const cell = atlas.cell({ kind: 'run', text: run.text, run, piece: { from: offset, to: offset + width }, chunkX: offset, font: this.layout.font, dpr: this.scale, width });
        if (!cell) { if (!this.pendingText) this.pendingStartLine = (this.currentLineIndex + 1) % Math.max(1, this.visibleLines.length); this.pendingText = true; this.currentSegmentPending = true; offset += width; continue; }
        const cssPad = cell.pad / this.scale, flags = gutter ? 0 : CLIP_GUTTER;
        this.quad({ left: x + offset - cssPad, right: x + offset + width + cssPad, top: y, bottom: y + this.layout.font.lineHeight }, parseCssColor(tint), atlas.texture,
          [cell.u0, cell.v0, cell.u1, cell.v1], flags);
        offset += width;
      }
    }
  }
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

function parseCssColor(value: string): number[] {
  const hex = /^#([\da-f]{6})$/i.exec(value);
  if (hex) return [0, 2, 4].map((index) => Number.parseInt(hex[1]!.slice(index, index + 2), 16) / 255).concat(1);
  const rgba = /^rgba?\(([^)]+)\)$/.exec(value);
  if (rgba) {
    const parts = rgba[1]!.split(',').map((part) => Number(part.trim()));
    if (parts.length >= 3 && parts.slice(0, 3).every(Number.isFinite)) return [parts[0]! / 255, parts[1]! / 255, parts[2]! / 255, parts[3] ?? 1];
  }
  return [1, 1, 1, 1];
}
