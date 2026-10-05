import type { ResourceBudget } from '../app/apis';
import { RESOURCE_LIMITS } from '../code/resources';

const VIDEO_BYTES = RESOURCE_LIMITS.videoWidth * RESOURCE_LIMITS.videoHeight * 4;
const FRAME_INTERVAL_MS = 1000 / 30;

interface VideoFrameMetadata { mediaTime?: number }
type VideoWithFrameCallback = HTMLVideoElement & {
  requestVideoFrameCallback?: (cb: (now: number, metadata: VideoFrameMetadata) => void) => number;
  cancelVideoFrameCallback?: (id: number) => void;
};

export interface VideoFrame { source: TexImageSource; revision: number }

/** One local, muted video source with bounded frame rate and optional downscale staging. */
export class VideoBackground {
  private readonly video: VideoWithFrameCallback;
  private readonly ledger?: ResourceBudget;
  private readonly createCanvas: () => HTMLCanvasElement;
  private readonly onStatus?: (status: string) => void;
  private canvas: HTMLCanvasElement | null = null;
  private canvasContext: CanvasRenderingContext2D | null = null;
  private url: string | null = null;
  private reservation = false;
  private visible = true;
  private disposed = false;
  private frameCallback: number | null = null;
  private callbackReady = false;
  private lastTime: number | null = null;
  private lastAcceptedMs = -Infinity;
  private revision = 0;
  private lastFrame: VideoFrame | null = null;

  constructor(doc: Document, opts: { ledger?: ResourceBudget; createCanvas?: () => HTMLCanvasElement; onStatus?(s: string): void } = {}) {
    this.video = doc.createElement('video') as VideoWithFrameCallback;
    this.video.muted = true;
    this.video.volume = 0;
    this.video.playsInline = true;
    this.video.loop = true;
    this.ledger = opts.ledger;
    this.createCanvas = opts.createCanvas ?? (() => doc.createElement('canvas'));
    this.onStatus = opts.onStatus;
    this.video.addEventListener('error', this.onDecodeError);
  }

  async load(file: Blob): Promise<void> {
    if (this.disposed) return;
    this.releaseSource();
    this.callbackReady = false;
    this.lastTime = null;
    this.lastAcceptedMs = -Infinity;
    this.lastFrame = null;
    try {
      this.url = URL.createObjectURL(file);
      this.video.src = this.url;
      this.video.muted = true;
      this.video.volume = 0;
      this.video.playsInline = true;
      this.video.loop = true;
      this.scheduleVideoFrameCallback();
      if (this.visible) await this.video.play();
      this.onStatus?.('');
    } catch (error) {
      this.onStatus?.(`video decode failed: ${String(error)}`);
    }
  }

  frame(): VideoFrame | null {
    if (this.disposed || !this.visible || this.video.videoWidth <= 0 || this.video.videoHeight <= 0) return null;
    const now = typeof performance === 'undefined' ? Date.now() : performance.now();
    const time = this.video.currentTime;
    const supportsFrameCallback = typeof (this.video as unknown as { requestVideoFrameCallback?: unknown }).requestVideoFrameCallback === 'function';
    const changed = supportsFrameCallback ? this.callbackReady : time !== this.lastTime;
    this.lastTime = time;
    if (!changed) return null;
    this.callbackReady = false;
    if (now - this.lastAcceptedMs < FRAME_INTERVAL_MS) return null;
    if (!this.reservation && this.ledger && !this.ledger.reserve(VIDEO_BYTES)) {
      this.onStatus?.('video exceeds GPU budget');
      return null;
    }
    this.reservation = !!this.ledger;

    let source: TexImageSource = this.video;
    if (this.video.videoWidth > RESOURCE_LIMITS.videoWidth || this.video.videoHeight > RESOURCE_LIMITS.videoHeight) {
      if (!this.canvas) {
        this.canvas = this.createCanvas();
        this.canvasContext = this.canvas.getContext('2d');
      }
      const scale = Math.min(RESOURCE_LIMITS.videoWidth / this.video.videoWidth, RESOURCE_LIMITS.videoHeight / this.video.videoHeight);
      const width = Math.max(1, Math.floor(this.video.videoWidth * scale));
      const height = Math.max(1, Math.floor(this.video.videoHeight * scale));
      if (this.canvas.width !== width || this.canvas.height !== height) {
        this.canvas.width = width;
        this.canvas.height = height;
      }
      if (!this.canvasContext) {
        this.onStatus?.('video decode failed: canvas unavailable');
        return null;
      }
      this.canvasContext.drawImage(this.video, 0, 0, width, height);
      source = this.canvas;
    }
    this.lastAcceptedMs = now;
    this.revision++;
    this.lastFrame = { source, revision: this.revision };
    return this.lastFrame;
  }

  setVisible(visible: boolean): void {
    if (this.disposed || this.visible === visible) return;
    this.visible = visible;
    if (!visible) {
      this.callbackReady = false;
      if (this.frameCallback !== null) this.video.cancelVideoFrameCallback?.(this.frameCallback);
      this.frameCallback = null;
      this.video.pause();
      return;
    }
    this.scheduleVideoFrameCallback();
    void this.video.play().catch((error: unknown) => this.onStatus?.(`video decode failed: ${String(error)}`));
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.video.removeEventListener('error', this.onDecodeError);
    if (this.frameCallback !== null) this.video.cancelVideoFrameCallback?.(this.frameCallback);
    this.frameCallback = null;
    this.releaseSource();
    if (this.reservation) this.ledger?.release(VIDEO_BYTES);
    this.reservation = false;
    this.canvas = null;
    this.canvasContext = null;
    this.lastFrame = null;
  }

  private readonly onDecodeError = (): void => this.onStatus?.('video decode failed');

  private scheduleVideoFrameCallback(): void {
    if (!this.visible || this.disposed || !this.video.requestVideoFrameCallback || this.frameCallback !== null) return;
    this.frameCallback = this.video.requestVideoFrameCallback(() => {
      this.frameCallback = null;
      this.callbackReady = true;
      this.scheduleVideoFrameCallback();
    });
  }

  private releaseSource(): void {
    if (this.frameCallback !== null) this.video.cancelVideoFrameCallback?.(this.frameCallback);
    this.frameCallback = null;
    if (this.url) URL.revokeObjectURL(this.url);
    this.url = null;
    this.video.pause();
    this.video.removeAttribute('src');
    this.video.load();
  }
}
