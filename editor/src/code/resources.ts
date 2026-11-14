import type { ResourceBudget } from '../app/apis';

export const MiB = 1024 * 1024;
export const RESOURCE_LIMITS = Object.freeze({
  total: 96 * MiB, atlas: 16 * MiB, geometry: 8 * MiB, layout: 8 * MiB,
  canvasPixels: 4_000_000, visualText: 8 * MiB, visualTargets: 8,
  visualSide: 1024, videoWidth: 1280, videoHeight: 720,
});
export type ResourceKind = 'atlas' | 'geometry' | 'canvas' | 'backdrop' | 'visualText' | 'visualTarget' | 'video' | 'other';
export interface Reservation { readonly bytes: number; release(): void }
export interface ResourceCounters { usedBytes: number; byKind: Readonly<Record<ResourceKind, number>>; pixels: number; visualTargets: number; refusals: number }

/** One instance shared by the code compositor and visual host. CPU layout is separate. */
export class ResourceLedger implements ResourceBudget {
  readonly limitBytes: number;
  private kinds: Record<ResourceKind, number> = { atlas: 0, geometry: 0, canvas: 0, backdrop: 0, visualText: 0, visualTarget: 0, video: 0, other: 0 };
  private pixels = 0;
  private targets = 0;
  private refused = 0;
  constructor(limitBytes = RESOURCE_LIMITS.total) {
    if (!Number.isSafeInteger(limitBytes) || limitBytes < 0 || limitBytes > RESOURCE_LIMITS.total) throw new RangeError('Invalid GPU budget');
    this.limitBytes = limitBytes;
  }
  get usedBytes(): number { return Object.values(this.kinds).reduce((a, b) => a + b, 0); }
  get counters(): ResourceCounters { return { usedBytes: this.usedBytes, byKind: { ...this.kinds }, pixels: this.pixels, visualTargets: this.targets, refusals: this.refused }; }
  reserve(bytes: number): boolean {
    const r = this.allocate('other', bytes);
    return r !== null;
  }
  release(bytes: number): void {
    if (!Number.isSafeInteger(bytes) || bytes < 0 || bytes > this.kinds.other) throw new RangeError('Unowned release');
    this.kinds.other -= bytes;
  }
  allocate(kind: ResourceKind, bytes: number, size?: { width: number; height: number }): Reservation | null {
    let pixelCount = 0;
    const sized = kind === 'canvas' || kind === 'backdrop' || kind === 'visualTarget' || kind === 'video';
    let valid = Number.isSafeInteger(bytes) && bytes >= 0;
    if (sized) {
      valid &&= !!size && Number.isSafeInteger(size.width) && Number.isSafeInteger(size.height) && size.width > 0 && size.height > 0;
      if (valid && size) {
        pixelCount = size.width * size.height;
        valid &&= Number.isSafeInteger(pixelCount) && bytes >= pixelCount * 4;
        if (kind === 'visualTarget') valid &&= size.width <= RESOURCE_LIMITS.visualSide && size.height <= RESOURCE_LIMITS.visualSide && this.targets < RESOURCE_LIMITS.visualTargets;
        if (kind === 'video') valid &&= size.width <= RESOURCE_LIMITS.videoWidth && size.height <= RESOURCE_LIMITS.videoHeight && this.kinds.video === 0;
      }
    }
    const categoryLimit = kind === 'atlas' ? RESOURCE_LIMITS.atlas : kind === 'geometry' ? RESOURCE_LIMITS.geometry : kind === 'visualText' ? RESOURCE_LIMITS.visualText : this.limitBytes;
    const countedPixels = kind === 'canvas' || kind === 'backdrop' ? pixelCount : 0;
    if (!valid || this.usedBytes + bytes > this.limitBytes || this.kinds[kind] + bytes > categoryLimit || this.pixels + countedPixels > RESOURCE_LIMITS.canvasPixels) {
      this.refused++; return null;
    }
    this.kinds[kind] += bytes; this.pixels += countedPixels;
    if (kind === 'visualTarget') this.targets++;
    let live = true;
    return { bytes, release: () => {
      if (!live) return;
      live = false; this.kinds[kind] -= bytes; this.pixels -= countedPixels;
      if (kind === 'visualTarget') this.targets--;
    } };
  }
}

/** Size only; the caller reserves the replacement before touching live targets. */
export function effectiveSize(width: number, height: number, dpr: number, maxTextureSize: number, pixelLimit: number = RESOURCE_LIMITS.canvasPixels): { width: number; height: number; dpr: number; reduced: boolean } {
  if (![width, height, dpr, maxTextureSize, pixelLimit].every(Number.isFinite) || width <= 0 || height <= 0 || dpr <= 0 || maxTextureSize < 1 || pixelLimit < 1) throw new RangeError('Invalid viewport');
  const scale = Math.min(dpr, maxTextureSize / width, maxTextureSize / height, Math.sqrt(pixelLimit / (width * height)));
  return { width: Math.max(1, Math.floor(width * scale)), height: Math.max(1, Math.floor(height * scale)), dpr: scale, reduced: scale < dpr };
}
