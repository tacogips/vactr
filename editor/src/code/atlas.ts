import { ResourceLedger, type Reservation } from './resources';
import type { ShapedRun, LayoutFont } from './layout';

export type CanvasFactory = () => HTMLCanvasElement;
export interface RunStyle { from: number; to: number; color: string }
export interface CellRequest {
  kind: 'mask' | 'color' | 'run'; text: string; font: LayoutFont; dpr: number; width: number;
  run?: ShapedRun; piece?: { from: number; to: number }; chunkX?: number; color?: string;
}
export interface AtlasCell { u0: number; v0: number; u1: number; v1: number; width: number; height: number; pad: number; generation: number }
interface Entry { cell: AtlasCell; reservation: Reservation; key: string }
const PAD_DPR = 2;

/** Append-only, single-texture glyph cells. Cells remain valid until generation changes. */
export class GlyphAtlas {
  private cells = new Map<string, Entry>();
  private textureValue: WebGLTexture;
  private canvas: HTMLCanvasElement;
  private context: CanvasRenderingContext2D;
  private canvasReservation: Reservation;
  private textureReservation: Reservation;
  private width: number;
  private height: number;
  private x = 2;
  private y = 0;
  private shelfHeight = 2;
  private frameBudget = 1 << 20;
  private frameBytes = 0;
  private disposed = false;
  readonly stats = { uploads: 0, newCells: 0, hits: 0, resets: 0, growths: 0, generation: 1, evictions: 0 };
  constructor(private gl: WebGL2RenderingContext, private ledger: ResourceLedger, private createCanvas: CanvasFactory = () => document.createElement('canvas')) {
    const max = gl.getParameter(gl.MAX_TEXTURE_SIZE) as number;
    if (!Number.isInteger(max) || max < 1) throw new Error('Invalid texture limit');
    this.width = Math.min(1024, max); this.height = Math.min(1024, max);
    const bytes = this.width * this.height * 4;
    const textureReservation = ledger.allocate('atlas', bytes);
    const canvasReservation = ledger.allocate('geometry', this.width * 4);
    if (!textureReservation || !canvasReservation) { textureReservation?.release(); canvasReservation?.release(); throw new Error('Text atlas budget exhausted'); }
    this.textureReservation = textureReservation; this.canvasReservation = canvasReservation;
    this.canvas = createCanvas(); this.canvas.width = this.width; this.canvas.height = 1;
    const context = this.canvas.getContext('2d');
    if (!context) { textureReservation.release(); canvasReservation.release(); throw new Error('Font rasterizer unavailable'); }
    this.context = context;
    const texture = gl.createTexture();
    if (!texture) { textureReservation.release(); canvasReservation.release(); throw new Error('Text atlas allocation failed'); }
    this.textureValue = texture;
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, true);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, this.width, this.height, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    // Reserved white texels are used for solid-color rectangles by geometry layers.
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, 2, 2, gl.RGBA, gl.UNSIGNED_BYTE,
      new Uint8Array([255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255]));
  }
  get texture(): WebGLTexture { return this.textureValue; }
  get size(): number { return this.cells.size; }
  beginFrame(budgetBytes = 1 << 20): void {
    if (!Number.isSafeInteger(budgetBytes) || budgetBytes < 0) throw new RangeError('Invalid atlas frame budget');
    this.frameBudget = budgetBytes; this.frameBytes = 0; this.stats.newCells = 0; this.stats.uploads = 0;
  }
  endFrame(): void { /* staging storage is persistent across frames */ }
  private grow(): boolean {
    const next = this.width < 2048 ? Math.min(2048, this.width * 2) : this.width;
    const nextHeight = next === this.width ? Math.min(2048, this.height * 2) : this.height;
    if (next === this.width && nextHeight === this.height) return false;
    const bytes = next * nextHeight * 4;
    const reservation = this.ledger.allocate('atlas', bytes);
    if (!reservation) return false;
    const texture = this.gl.createTexture();
    if (!texture) { reservation.release(); return false; }
    this.gl.bindTexture(this.gl.TEXTURE_2D, texture);
    this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_MIN_FILTER, this.gl.LINEAR);
    this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_MAG_FILTER, this.gl.LINEAR);
    this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_WRAP_S, this.gl.CLAMP_TO_EDGE);
    this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_WRAP_T, this.gl.CLAMP_TO_EDGE);
    this.gl.texImage2D(this.gl.TEXTURE_2D, 0, this.gl.RGBA, next, nextHeight, 0, this.gl.RGBA, this.gl.UNSIGNED_BYTE, null);
    this.gl.deleteTexture(this.textureValue); this.textureReservation.release();
    this.textureValue = texture; this.textureReservation = reservation;
    this.width = next; this.height = nextHeight; this.x = 2; this.y = 0; this.shelfHeight = 2;
    this.clearCells(); this.stats.generation++; this.stats.growths++;
    this.gl.bindTexture(this.gl.TEXTURE_2D, texture);
    this.gl.texSubImage2D(this.gl.TEXTURE_2D, 0, 0, 0, 2, 2, this.gl.RGBA, this.gl.UNSIGNED_BYTE,
      new Uint8Array([255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255]));
    return true;
  }
  private clearCells(): void {
    for (const entry of this.cells.values()) entry.reservation.release();
    this.cells.clear();
  }
  cell(request: CellRequest): AtlasCell | null {
    if (this.disposed) throw new Error('Atlas disposed');
    const { text, font, dpr } = request;
    if (!Number.isFinite(dpr) || dpr <= 0 || !Number.isFinite(request.width) || request.width <= 0) throw new RangeError('Invalid atlas cell');
    const pad = Math.ceil(PAD_DPR * dpr);
    const w = Math.max(1, Math.ceil(request.width * dpr) + pad * 2);
    const h = Math.max(1, Math.ceil(font.lineHeight * dpr));
    const key = `${request.kind}|${font.font}|${font.fallback ?? ''}|${font.generation ?? 0}|${dpr}|${text}|${request.piece?.from ?? 0}|${request.piece?.to ?? text.length}|${request.chunkX ?? 0}`;
    const hit = this.cells.get(key);
    if (hit) { this.stats.hits++; return hit.cell; }
    const bytes = w * h * 4;
    if (bytes > this.frameBudget - this.frameBytes) return null;
    if (w > this.width || h > this.height) return null;
    if (this.x + w > this.width) { this.x = 0; this.y += this.shelfHeight; this.shelfHeight = 0; }
    if (this.y + h > this.height) {
      if (!this.grow()) { this.reset(); if (this.y + h > this.height) return null; }
    }
    const reservation = this.ledger.allocate('geometry', 96 + key.length * 2);
    if (!reservation) return null;
    if (this.canvas.height < h) {
      const nextStaging = this.ledger.allocate('geometry', this.width * h * 4);
      if (!nextStaging) { reservation.release(); return null; }
      this.canvasReservation.release(); this.canvasReservation = nextStaging; this.canvas.height = h;
    }
    const ctx = this.context;
    ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.clearRect(0, 0, w, h);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0); ctx.font = font.font; ctx.textBaseline = 'alphabetic';
    ctx.fillStyle = request.kind === 'mask' ? '#ffffff' : (request.color ?? '#d8dee9');
    const run = request.run;
    const offset = request.piece?.from ?? 0;
    if (run && request.kind === 'run') {
      ctx.save(); ctx.beginPath(); ctx.rect(pad / dpr, 0, request.width, font.lineHeight); ctx.clip();
      ctx.fillText(text, pad / dpr - (request.chunkX ?? 0), font.baseline); ctx.restore();
    } else ctx.fillText(text, pad / dpr - offset, font.baseline);
    const px = this.x, py = this.y;
    this.gl.bindTexture(this.gl.TEXTURE_2D, this.textureValue);
    const pixels = this.context.getImageData(0, 0, w, h).data;
    this.gl.texSubImage2D(this.gl.TEXTURE_2D, 0, px, py, w, h, this.gl.RGBA, this.gl.UNSIGNED_BYTE, pixels);
    const cell: AtlasCell = { u0: px / this.width, v0: py / this.height, u1: (px + w) / this.width, v1: (py + h) / this.height, width: w, height: h, pad, generation: this.stats.generation };
    this.cells.set(key, { cell, reservation, key }); this.x += w; this.shelfHeight = Math.max(this.shelfHeight, h);
    this.frameBytes += bytes; this.stats.uploads++; this.stats.newCells++;
    return cell;
  }
  /** Temporary adapter for consumers not yet migrated to cell geometry. */
  tile(request: { run: ShapedRun; font: LayoutFont; dpr: number; x: number; width: number; color?: string; styles?: readonly RunStyle[] }): { texture: WebGLTexture; width: number; height: number; x: number; cssWidth: number; cell: AtlasCell } | null {
    const cell = this.cell({ kind: request.styles?.length ? 'run' : 'mask', text: request.run.text, run: request.run, piece: { from: request.x, to: request.x + request.width }, chunkX: request.x, font: request.font, dpr: request.dpr, width: request.width, color: request.color });
    return cell ? { texture: this.textureValue, width: cell.width, height: cell.height, x: request.x, cssWidth: request.width, cell } : null;
  }
  get tilePixels(): number { return this.width; }
  supportsRunRaster(run: ShapedRun, _font: LayoutFont, _dpr: number): boolean { return run.width > this.width; }
  reset(): void {
    this.clearCells(); this.x = 2; this.y = 0; this.shelfHeight = 2;
    this.stats.generation++; this.stats.resets++; this.stats.evictions++;
    this.gl.bindTexture(this.gl.TEXTURE_2D, this.textureValue);
    this.gl.texImage2D(this.gl.TEXTURE_2D, 0, this.gl.RGBA, this.width, this.height, 0, this.gl.RGBA, this.gl.UNSIGNED_BYTE, null);
  }
  invalidate(): void { this.reset(); }
  contextLost(): void { this.clearCells(); this.canvasReservation.release(); this.textureReservation.release(); this.disposed = true; }
  dispose(): void {
    if (this.disposed) return;
    this.clearCells(); this.canvasReservation.release(); this.textureReservation.release();
    this.gl.deleteTexture(this.textureValue); this.canvas.width = 0; this.canvas.height = 0; this.disposed = true;
  }
}
