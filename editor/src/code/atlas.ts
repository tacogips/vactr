import { ResourceLedger, type Reservation } from './resources';
import type { LayoutFont, ShapedRun } from './layout';

export type CanvasFactory = () => HTMLCanvasElement;
export interface CellRequest {
  kind: 'mask' | 'color' | 'run'; text: string; font: LayoutFont; dpr: number; width: number;
  run?: ShapedRun; piece?: { from: number; to: number }; chunkX?: number;
}
export interface AtlasCell { u0: number; v0: number; u1: number; v1: number; width: number; height: number; pad: number; generation: number }
interface Entry { cell: AtlasCell; reservation: Reservation }
const PAD_DPR = 2;
const MAX_ATLAS_BYTES = 16 * 1024 * 1024;

/** Append-only white glyph cells. Cell colors are supplied by their instances. */
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
  private maxTextureSize: number;
  private frameBudget = 1 << 20;
  private frameBytes = 0;
  private pendingGrowth = false;
  private pendingReset = false;
  private rasterFontKey = '';
  private invalidated = false;
  private disposed = false;
  readonly stats = { uploads: 0, newCells: 0, hits: 0, resets: 0, growths: 0, generation: 1, evictions: 0 };

  constructor(private gl: WebGL2RenderingContext, private ledger: ResourceLedger, createCanvas: CanvasFactory = () => document.createElement('canvas')) {
    this.maxTextureSize = gl.getParameter(gl.MAX_TEXTURE_SIZE) as number;
    if (!Number.isInteger(this.maxTextureSize) || this.maxTextureSize < 1) throw new Error('Invalid texture limit');
    this.width = Math.min(1024, this.maxTextureSize);
    this.height = Math.min(1024, this.maxTextureSize);
    const textureReservation = ledger.allocate('atlas', this.width * this.height * 4);
    const canvasReservation = ledger.allocate('geometry', this.width * 4);
    if (!textureReservation || !canvasReservation) {
      textureReservation?.release(); canvasReservation?.release();
      throw new Error('Text atlas budget exhausted');
    }
    this.textureReservation = textureReservation;
    this.canvasReservation = canvasReservation;
    this.canvas = createCanvas(); this.canvas.width = this.width; this.canvas.height = 1;
    const context = this.canvas.getContext('2d');
    if (!context) { textureReservation.release(); canvasReservation.release(); throw new Error('Font rasterizer unavailable'); }
    this.context = context;
    const texture = gl.createTexture();
    if (!texture) { textureReservation.release(); canvasReservation.release(); throw new Error('Text atlas allocation failed'); }
    this.textureValue = texture;
    try {
      this.configure(texture, this.width, this.height);
      this.writeWhiteTexels();
    } catch {
      gl.deleteTexture(texture); textureReservation.release(); canvasReservation.release();
      throw new Error('Text atlas allocation failed');
    }
  }

  get texture(): WebGLTexture { return this.textureValue; }
  get size(): number { return this.cells.size; }
  get maxCellWidth(): number { return this.width; }
  get dimensions(): readonly [number, number] { return [this.width, this.height]; }

  beginFrame(budgetBytes = 1 << 20): void {
    if (!Number.isSafeInteger(budgetBytes) || budgetBytes < 0) throw new RangeError('Invalid atlas frame budget');
    this.frameBudget = budgetBytes; this.frameBytes = 0;
    if (this.invalidated) { this.resetNow(); this.invalidated = false; }
    else if (this.pendingGrowth) {
      this.pendingGrowth = false;
      if (!this.grow()) { this.pendingReset = true; }
    }
    if (this.pendingReset) { this.resetNow(); this.pendingReset = false; }
  }

  endFrame(): void { /* the staging surface is retained for the atlas lifetime */ }

  cell(request: CellRequest): AtlasCell | null {
    if (this.disposed) throw new Error('Atlas disposed');
    const { text, font, dpr } = request;
    if (!Number.isFinite(dpr) || dpr <= 0 || !Number.isFinite(request.width) || request.width <= 0) throw new RangeError('Invalid atlas cell');
    const pad = Math.ceil(PAD_DPR * dpr);
    const width = Math.max(1, Math.ceil(request.width * dpr) + pad * 2);
    const height = Math.max(1, Math.ceil(font.lineHeight * dpr));
    const exact = text.length <= 256 ? `:${text}` : '';
    const key = `${request.kind}|${font.font}|${font.fallback ?? ''}|${font.generation ?? 0}|${dpr}|${text.length}:${hashText(text)}${exact}|${request.piece?.from ?? 0}|${request.piece?.to ?? text.length}|${request.chunkX ?? 0}`;
    const hit = this.cells.get(key);
    if (hit) { this.stats.hits++; return hit.cell; }
    const bytes = width * height * 4;
    if (bytes > this.frameBudget - this.frameBytes || width > this.width || height > this.height) return null;
    let x = this.x, y = this.y, shelfHeight = this.shelfHeight;
    if (x + width > this.width) { x = 0; y += shelfHeight; shelfHeight = 0; }
    if (y + height > this.height) { this.pendingGrowth = true; return null; }
    // Cell metadata is CPU-only. The atlas texture and staging surface carry
    // the GPU/storage reservations; per-glyph JS keys do not consume GPU budget.
    const reservation = this.ledger.allocate('geometry', 0);
    if (!reservation) return null;
    const previousCanvasHeight = this.canvas.height;
    let stagingReservation: Reservation | null = null;
    if (this.canvas.height < height) {
      stagingReservation = this.ledger.allocate('geometry', this.width * height * 4);
      if (!stagingReservation) { reservation.release(); return null; }
      this.canvas.height = height;
      this.rasterFontKey = '';
    }
    const ctx = this.context;
    ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.clearRect(0, 0, width, height);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const fontKey = `${font.font}|${font.fallback ?? ''}|${font.generation ?? 0}|${dpr}|${this.stats.generation}`;
    if (this.rasterFontKey !== fontKey) { ctx.font = font.font; this.rasterFontKey = fontKey; }
    ctx.textBaseline = 'alphabetic'; ctx.fillStyle = '#ffffff';
    const pieceOffset = request.piece?.from ?? 0;
    if (request.run && request.kind === 'run') {
      ctx.save(); ctx.beginPath(); ctx.rect(pad / dpr, 0, request.width, font.lineHeight); ctx.clip();
      ctx.fillText(text, pad / dpr - (request.chunkX ?? 0), font.baseline); ctx.restore();
    } else ctx.fillText(text, pad / dpr - pieceOffset, font.baseline);
    this.gl.bindTexture(this.gl.TEXTURE_2D, this.textureValue);
    this.gl.pixelStorei(this.gl.UNPACK_SKIP_PIXELS, 0);
    this.gl.pixelStorei(this.gl.UNPACK_SKIP_ROWS, 0);
    this.gl.pixelStorei(this.gl.UNPACK_ROW_LENGTH, 0);
    try {
      this.gl.texSubImage2D(this.gl.TEXTURE_2D, 0, x, y, width, height, this.gl.RGBA, this.gl.UNSIGNED_BYTE, this.canvas);
    } catch (error) {
      reservation.release();
      if (stagingReservation) {
        this.canvas.height = previousCanvasHeight;
        stagingReservation.release();
        this.rasterFontKey = '';
      }
      throw error;
    }
    if (stagingReservation) { this.canvasReservation.release(); this.canvasReservation = stagingReservation; }
    const cell: AtlasCell = { u0: x / this.width, v0: y / this.height, u1: (x + width) / this.width, v1: (y + height) / this.height, width, height, pad, generation: this.stats.generation };
    this.cells.set(key, { cell, reservation }); this.x = x + width; this.y = y; this.shelfHeight = Math.max(shelfHeight, height);
    this.frameBytes += bytes; this.stats.uploads++; this.stats.newCells++;
    return cell;
  }

  invalidate(): void { this.invalidated = true; }
  contextLost(): void { this.clearCells(); this.canvasReservation.release(); this.textureReservation.release(); this.disposed = true; }
  dispose(): void {
    if (this.disposed) return;
    this.clearCells(); this.canvasReservation.release(); this.textureReservation.release();
    this.gl.deleteTexture(this.textureValue); this.canvas.width = 0; this.canvas.height = 0; this.disposed = true;
  }

  private configure(texture: WebGLTexture, width: number, height: number): void {
    this.gl.bindTexture(this.gl.TEXTURE_2D, texture);
    this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_MIN_FILTER, this.gl.LINEAR);
    this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_MAG_FILTER, this.gl.LINEAR);
    this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_WRAP_S, this.gl.CLAMP_TO_EDGE);
    this.gl.texParameteri(this.gl.TEXTURE_2D, this.gl.TEXTURE_WRAP_T, this.gl.CLAMP_TO_EDGE);
    this.gl.texImage2D(this.gl.TEXTURE_2D, 0, this.gl.RGBA, width, height, 0, this.gl.RGBA, this.gl.UNSIGNED_BYTE, null);
    if (this.gl.getError() !== this.gl.NO_ERROR) throw new Error('GL allocation failed');
  }

  private writeWhiteTexels(): void {
    this.gl.bindTexture(this.gl.TEXTURE_2D, this.textureValue);
    this.gl.texSubImage2D(this.gl.TEXTURE_2D, 0, 0, 0, 2, 2, this.gl.RGBA, this.gl.UNSIGNED_BYTE,
      new Uint8Array([255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255]));
  }

  private grow(): boolean {
    const nextWidth = this.width < 2048 ? Math.min(2048, this.width * 2, this.maxTextureSize) : this.width;
    const nextHeight = nextWidth === this.width ? Math.min(2048, this.height * 2, this.maxTextureSize) : this.height;
    if (nextWidth === this.width && nextHeight === this.height) return false;
    const bytes = nextWidth * nextHeight * 4;
    if (bytes > MAX_ATLAS_BYTES) return false;
    const reservation = this.ledger.allocate('atlas', bytes);
    if (!reservation) return false;
    const staging = this.ledger.allocate('geometry', nextWidth * this.canvas.height * 4);
    if (!staging) { reservation.release(); return false; }
    const texture = this.gl.createTexture();
    if (!texture) { reservation.release(); staging.release(); return false; }
    try { this.configure(texture, nextWidth, nextHeight); }
    catch { this.gl.deleteTexture(texture); reservation.release(); staging.release(); return false; }
    this.gl.deleteTexture(this.textureValue); this.textureReservation.release();
    this.textureValue = texture; this.textureReservation = reservation; this.width = nextWidth; this.height = nextHeight;
    this.canvasReservation.release(); this.canvasReservation = staging; this.canvas.width = this.width;
    this.x = 2; this.y = 0; this.shelfHeight = 2; this.clearCells();
    this.stats.generation++; this.stats.growths++;
    this.writeWhiteTexels();
    return true;
  }

  private resetNow(): void {
    this.clearCells(); this.x = 2; this.y = 0; this.shelfHeight = 2;
    this.gl.bindTexture(this.gl.TEXTURE_2D, this.textureValue);
    this.gl.texImage2D(this.gl.TEXTURE_2D, 0, this.gl.RGBA, this.width, this.height, 0, this.gl.RGBA, this.gl.UNSIGNED_BYTE, null);
    if (this.gl.getError() !== this.gl.NO_ERROR) throw new Error('Text atlas allocation failed');
    this.writeWhiteTexels(); this.stats.generation++; this.stats.resets++; this.stats.evictions++;
  }

  private clearCells(): void { for (const entry of this.cells.values()) entry.reservation.release(); this.cells.clear(); }
}

function hashText(text: string): string {
  let a = 2166136261, b = 0x9e3779b9;
  for (let i = 0; i < text.length; i++) {
    const code = text.charCodeAt(i); a = Math.imul(a ^ code, 16777619); b = Math.imul(b ^ (code + i), 2246822519);
  }
  return `${a >>> 0}.${b >>> 0}`;
}
