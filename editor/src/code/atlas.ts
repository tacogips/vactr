import { RESOURCE_LIMITS, ResourceLedger, type Reservation } from './resources';
import type { ShapedRun, LayoutFont } from './layout';

export type CanvasFactory = () => HTMLCanvasElement;
export interface RunStyle { from: number; to: number; color: string }
export interface TileRequest {
  run: ShapedRun; font: LayoutFont; dpr: number; x: number; width: number;
  color?: string; styles?: readonly RunStyle[];
}
export interface AtlasTile { texture: WebGLTexture; width: number; height: number; x: number; cssWidth: number }
interface Entry { tile: AtlasTile; reservation: Reservation; identity: string; metadata: Reservation }
function hash(text: string): number {
  let h = 2166136261;
  for (let i = 0; i < text.length; i++) h = Math.imul(h ^ text.charCodeAt(i), 16777619);
  return h >>> 0;
}

/** Bounded run tiles; rasterize the complete run translated into each cropped tile. */
export class GlyphAtlas {
  private entries = new Map<string, Entry>();
  private rasterCanvas: HTMLCanvasElement | null = null;
  private rasterContext: CanvasRenderingContext2D | null = null;
  private rasterReservation: Reservation | null = null;
  private runRasterCanvas: HTMLCanvasElement | null = null;
  private runRasterContext: CanvasRenderingContext2D | null = null;
  private runRasterReservation: Reservation | null = null;
  private runRasterIdentity = '';
  private disposed = false;
  private runIds = new WeakMap<ShapedRun, number>();
  private nextRunId = 1;
  private frameBudget = 1 << 20;
  private frameBytes = 0;
  readonly stats = { uploads: 0, hits: 0, evictions: 0 };
  readonly tilePixels: number;
  constructor(private gl: WebGL2RenderingContext, private ledger: ResourceLedger, private createCanvas: CanvasFactory = () => document.createElement('canvas')) {
    this.tilePixels = Math.min(1024, gl.getParameter(gl.MAX_TEXTURE_SIZE) as number);
    if (!Number.isInteger(this.tilePixels) || this.tilePixels < 1) throw new Error('Invalid texture limit');
  }
  get size(): number { return this.entries.size; }
  private raster(w: number, h: number, bytes: number): CanvasRenderingContext2D {
    if (!this.rasterCanvas) this.rasterCanvas = this.createCanvas();
    if (bytes > (this.rasterReservation?.bytes ?? 0)) {
      this.rasterCanvas.width = 0; this.rasterCanvas.height = 0;
      this.rasterReservation?.release(); this.rasterReservation = null;
      while (!this.rasterReservation) {
        this.rasterReservation = this.ledger.allocate('geometry', bytes);
        if (this.rasterReservation) break;
        const first = this.entries.entries().next().value;
        if (!first) throw new Error('Text atlas/staging budget exhausted');
        this.remove(first[0], first[1]);
      }
    }
    if (!this.rasterContext) {
      this.rasterContext = this.rasterCanvas.getContext('2d');
      if (!this.rasterContext) { this.releaseRaster(); throw new Error('Font rasterizer unavailable'); }
    }
    if (this.rasterCanvas.width !== w || this.rasterCanvas.height !== h) {
      this.rasterCanvas.width = w; this.rasterCanvas.height = h;
    }
    return this.rasterContext;
  }
  private releaseRaster(): void {
    if (this.rasterCanvas) { this.rasterCanvas.width = 0; this.rasterCanvas.height = 0; }
    this.rasterReservation?.release(); this.rasterReservation = null;
    this.rasterContext = null; this.rasterCanvas = null;
  }
  beginFrame(budgetBytes = 1 << 20): void {
    if (!Number.isSafeInteger(budgetBytes) || budgetBytes < 0) throw new RangeError('Invalid atlas frame budget');
    this.frameBudget = budgetBytes; this.frameBytes = 0;
  }
  endFrame(): void {
    if (this.rasterCanvas) { this.rasterCanvas.width = 0; this.rasterCanvas.height = 0; }
    this.rasterReservation?.release(); this.rasterReservation = null;
    this.releaseRunRaster();
  }
  private releaseRunRaster(): void {
    if (this.runRasterCanvas) { this.runRasterCanvas.width = 0; this.runRasterCanvas.height = 0; }
    this.runRasterReservation?.release(); this.runRasterReservation = null;
    this.runRasterContext = null; this.runRasterCanvas = null; this.runRasterIdentity = '';
  }
  supportsRunRaster(run: ShapedRun, font: LayoutFont, dpr: number): boolean {
    const width = Math.ceil(run.width * dpr), height = Math.ceil(font.lineHeight * dpr);
    const bytes = width * height * 4;
    return Number.isFinite(dpr) && dpr > 0 && width > this.tilePixels && width <= 16_384 && height >= 1 && bytes <= RESOURCE_LIMITS.geometry;
  }
  private prepareRunRaster(request: TileRequest, identity: string): HTMLCanvasElement | null {
    const { run, font, dpr } = request;
    const width = Math.ceil(run.width * dpr), height = Math.ceil(font.lineHeight * dpr);
    const bytes = width * height * 4;
    if (!this.supportsRunRaster(run, font, dpr)) return null;
    if (this.runRasterIdentity === identity && this.runRasterCanvas) return this.runRasterCanvas;
    this.releaseRunRaster();
    const reservation = this.ledger.allocate('geometry', bytes);
    if (!reservation) return null;
    const canvas = this.createCanvas();
    canvas.width = width; canvas.height = height;
    const ctx = canvas.getContext('2d');
    if (!ctx) { reservation.release(); canvas.width = 0; canvas.height = 0; return null; }
    try {
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0); ctx.clearRect(0, 0, run.width, font.lineHeight);
      ctx.font = font.font; ctx.textBaseline = 'alphabetic'; ctx.fillStyle = request.color ?? '#d8dee9';
      ctx.fillText(run.text, 0, font.baseline);
      for (const style of request.styles ?? []) {
        const left = ctx.measureText(run.text.slice(0, Math.max(0, style.from - run.from))).width;
        const right = ctx.measureText(run.text.slice(0, Math.max(0, style.to - run.from))).width;
        ctx.save(); ctx.beginPath(); ctx.rect(left, 0, right - left, font.lineHeight); ctx.clip();
        ctx.clearRect(left, 0, right - left, font.lineHeight); ctx.fillStyle = style.color;
        ctx.fillText(run.text, 0, font.baseline); ctx.restore();
      }
    } catch {
      reservation.release(); canvas.width = 0; canvas.height = 0; return null;
    }
    this.runRasterCanvas = canvas; this.runRasterContext = ctx;
    this.runRasterReservation = reservation; this.runRasterIdentity = identity;
    return canvas;
  }
  tile(request: TileRequest): AtlasTile | null {
    if (this.disposed) throw new Error('Atlas disposed');
    const { run, font, dpr, x, width } = request;
    const w = Math.ceil(width * dpr); const h = Math.ceil(font.lineHeight * dpr);
    if (![dpr, x, width].every(Number.isFinite) || dpr <= 0 || x < 0 || w < 1 || w > this.tilePixels || h < 1 || h > this.tilePixels) throw new RangeError('Invalid atlas tile');
    const styles = (request.styles ?? []).map(style => [style.from - run.from, style.to - run.from, style.color]);
    const identity = JSON.stringify([font, dpr, x, width, request.color, styles]);
    let id = this.runIds.get(run);
    if (id === undefined) { id = this.nextRunId++; this.runIds.set(run, id); }
    const key = `${id}:${hash(identity)}:${x}`;
    const hit = this.entries.get(key);
    if (hit && hit.identity === identity) {
      this.entries.delete(key); this.entries.set(key, hit); this.stats.hits++; return hit.tile;
    }
    if (hit) this.remove(key, hit); // Hash collision: equality above is authoritative.
    const bytes = w * h * 4;
    if (this.frameBytes + bytes > this.frameBudget) return null;
    const runIdentity = `${id}:${JSON.stringify([font, dpr, run.width, run.text, request.color, styles])}`;
    const runRaster = this.prepareRunRaster(request, runIdentity);
    // Keep one accounted staging canvas so raster setup does not allocate per tile.
    // Weak run identities keep historical source strings out of the atlas entirely.
    let reservation: Reservation | null = null;
    let metadata: Reservation | null = null;
    const ctx = this.raster(w, h, bytes);
    while (true) {
      reservation = this.ledger.allocate('atlas', bytes);
      metadata = reservation ? this.ledger.allocate('geometry', 128 + identity.length * 2) : null;
      if (reservation && metadata) break;
      reservation?.release(); metadata?.release();
      reservation = null; metadata = null;
      if (!this.entries.size) { this.releaseRaster(); throw new Error('Text atlas/staging budget exhausted'); }
      const first = this.entries.entries().next().value!; this.remove(first[0], first[1]);
    }
    let texture: WebGLTexture | null = null;
    try {
      const canvas = this.rasterCanvas;
      if (!canvas) throw new Error('Font rasterizer unavailable');
      if (runRaster && this.runRasterContext) {
        ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.clearRect(0, 0, w, h);
        ctx.drawImage(runRaster, Math.round(x * dpr), 0, w, h, 0, 0, w, h);
      } else {
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0); ctx.clearRect(0, 0, width, font.lineHeight);
        ctx.font = font.font; ctx.textBaseline = 'alphabetic'; ctx.fillStyle = request.color ?? '#d8dee9';
        ctx.fillText(run.text, -x, font.baseline);
        // Color masks reuse whole-run shaping; token boundaries never reshape substrings.
        for (const style of request.styles ?? []) {
          const left = ctx.measureText(run.text.slice(0, Math.max(0, style.from - run.from))).width - x;
          const right = ctx.measureText(run.text.slice(0, Math.max(0, style.to - run.from))).width - x;
          ctx.save(); ctx.beginPath(); ctx.rect(left, 0, right - left, font.lineHeight); ctx.clip();
          ctx.clearRect(left, 0, right - left, font.lineHeight);
          ctx.fillStyle = style.color; ctx.fillText(run.text, -x, font.baseline); ctx.restore();
        }
      }
      texture = this.gl.createTexture(); if (!texture) throw new Error('Text texture allocation failed');
      this.gl.bindTexture(this.gl.TEXTURE_2D, texture);
      this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_MIN_FILTER, this.gl.LINEAR);
      this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_MAG_FILTER, this.gl.LINEAR);
      this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_WRAP_S, this.gl.CLAMP_TO_EDGE);
      this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_WRAP_T, this.gl.CLAMP_TO_EDGE);
      this.gl.pixelStorei(this.gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, true);
      this.gl.texImage2D(this.gl.TEXTURE_2D, 0, this.gl.RGBA, this.gl.RGBA, this.gl.UNSIGNED_BYTE, canvas);
      if (this.gl.getError() !== this.gl.NO_ERROR) throw new Error('Text upload refused by GPU');
      const tile = { texture, width: w, height: h, x, cssWidth: width };
      this.entries.set(key, { tile, reservation, identity, metadata }); this.stats.uploads++; this.frameBytes += bytes;
      return tile;
    } catch (error) {
      if (texture) this.gl.deleteTexture(texture); reservation.release(); metadata.release(); this.releaseRaster(); throw error;
    }
  }
  private remove(key: string, entry: Entry): void {
    this.gl.deleteTexture(entry.tile.texture); entry.reservation.release(); entry.metadata.release(); this.entries.delete(key); this.stats.evictions++;
  }
  invalidate(): void { for (const [key, entry] of this.entries) this.remove(key, entry); }
  /** Lost-context objects have already been invalidated by WebGL. */
  contextLost(): void { for (const e of this.entries.values()) { e.reservation.release(); e.metadata.release(); } this.entries.clear(); this.releaseRaster(); this.releaseRunRaster(); }
  dispose(): void { if (this.disposed) return; this.invalidate(); this.releaseRaster(); this.releaseRunRaster(); this.disposed = true; }
}
