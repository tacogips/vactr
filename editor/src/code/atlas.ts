import { ResourceLedger, type Reservation } from './resources';
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
  private disposed = false;
  private runIds = new WeakMap<ShapedRun, number>();
  private nextRunId = 1;
  readonly stats = { uploads: 0, hits: 0, evictions: 0 };
  readonly tilePixels: number;
  constructor(private gl: WebGL2RenderingContext, private ledger: ResourceLedger, private createCanvas: CanvasFactory = () => document.createElement('canvas')) {
    this.tilePixels = Math.min(1024, gl.getParameter(gl.MAX_TEXTURE_SIZE) as number);
    if (!Number.isInteger(this.tilePixels) || this.tilePixels < 1) throw new Error('Invalid texture limit');
  }
  get size(): number { return this.entries.size; }
  tile(request: TileRequest): AtlasTile {
    if (this.disposed) throw new Error('Atlas disposed');
    const { run, font, dpr, x, width } = request;
    const w = Math.ceil(width * dpr); const h = Math.ceil(font.lineHeight * dpr);
    if (![dpr, x, width].every(Number.isFinite) || dpr <= 0 || x < 0 || w < 1 || w > this.tilePixels || h < 1 || h > this.tilePixels) throw new RangeError('Invalid atlas tile');
    const identity = JSON.stringify([font, dpr, x, width, request.color, request.styles]);
    let id = this.runIds.get(run);
    if (id === undefined) { id = this.nextRunId++; this.runIds.set(run, id); }
    const key = `${id}:${hash(identity)}:${x}`;
    const hit = this.entries.get(key);
    if (hit && hit.identity === identity) {
      this.entries.delete(key); this.entries.set(key, hit); this.stats.hits++; return hit.tile;
    }
    if (hit) this.remove(key, hit); // Hash collision: equality above is authoritative.
    const bytes = w * h * 4;
    // Reserve persistent metadata and temporary raster staging together with the tile.
    // Weak run identities keep historical source strings out of the atlas entirely.
    let reservation: Reservation | null = null;
    let metadata: Reservation | null = null;
    let staging: Reservation | null = null;
    while (true) {
      reservation = this.ledger.allocate('atlas', bytes);
      metadata = reservation ? this.ledger.allocate('geometry', 128 + identity.length * 2) : null;
      staging = metadata ? this.ledger.allocate('geometry', bytes) : null;
      if (reservation && metadata && staging) break;
      reservation?.release(); metadata?.release(); staging?.release();
      if (!this.entries.size) throw new Error('Text atlas/staging budget exhausted');
      const first = this.entries.entries().next().value!; this.remove(first[0], first[1]);
    }
    let texture: WebGLTexture | null = null; let canvas: HTMLCanvasElement | null = null;
    try {
      canvas = this.createCanvas(); canvas.width = w; canvas.height = h;
      const ctx = canvas.getContext('2d'); if (!ctx) throw new Error('Font rasterizer unavailable');
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
      this.entries.set(key, { tile, reservation, identity, metadata }); this.stats.uploads++;
      return tile;
    } catch (error) {
      if (texture) this.gl.deleteTexture(texture); reservation.release(); metadata.release(); throw error;
    } finally { staging.release(); if (canvas) { canvas.width = 0; canvas.height = 0; } }
  }
  private remove(key: string, entry: Entry): void {
    this.gl.deleteTexture(entry.tile.texture); entry.reservation.release(); entry.metadata.release(); this.entries.delete(key); this.stats.evictions++;
  }
  invalidate(): void { for (const [key, entry] of this.entries) this.remove(key, entry); }
  /** Lost-context objects have already been invalidated by WebGL. */
  contextLost(): void { for (const e of this.entries.values()) { e.reservation.release(); e.metadata.release(); } this.entries.clear(); }
  dispose(): void { if (this.disposed) return; this.invalidate(); this.disposed = true; }
}
