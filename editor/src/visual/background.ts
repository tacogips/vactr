import type { EditorDeps } from '../app/deps';
import { ResourceLedger } from '../code/resources';
import { startFrameLoop, windowScheduler, type FrameLoop, type FrameScheduler } from './frame';
import { BackgroundOverlays } from './overlays';
import { EditorSettingsSource, mediaUrl, type BackgroundSettings, type ParsedEditorSettings } from './settings';

const MAX_MEDIA_PIXELS = 16_777_216;
interface BackgroundOptions {
  scheduler?: FrameScheduler;
  shaderAvailable: boolean;
  hasShader?(output: 0 | 1 | 2 | 3): boolean;
  selectShader(output: 0 | 1 | 2 | 3): void;
}

/** Owns source-comment presentation only; it never evaluates or writes code. */
export class EditorBackground {
  readonly el: HTMLElement;
  readonly status: HTMLElement;
  private readonly source: EditorSettingsSource | null;
  private readonly overlays: BackgroundOverlays;
  private readonly offLevels: () => void;
  private readonly budget;
  private readonly host: HTMLElement;
  private loop: FrameLoop | null = null;
  private media: HTMLImageElement | HTMLVideoElement | null = null;
  private mediaBytes = 0;
  private revision = 0;
  private disposed = false;
  private mediaKey = '';
  private backgroundNotice = '';
  private shaderOutput: 0 | 1 | 2 | 3 | null = null;
  private overlayNotice = '';
  private errors: string[] = [];

  constructor(root: HTMLElement, statusParent: HTMLElement, private readonly deps: EditorDeps, private readonly opts: BackgroundOptions) {
    const doc = root.ownerDocument;
    this.host = root.querySelector<HTMLElement>('.vact-code') ?? root;
    this.el = doc.createElement('div');
    this.el.className = 'vact-editor-background';
    this.el.setAttribute('aria-hidden', 'true');
    this.host.prepend(this.el);
    this.status = doc.createElement('div');
    this.status.className = 'visual-editor-settings-status';
    this.status.hidden = true;
    this.status.setAttribute('role', 'status');
    this.status.setAttribute('aria-live', 'polite');
    statusParent.appendChild(this.status);
    this.budget = deps.resourceBudget ?? new ResourceLedger();
    this.overlays = new BackgroundOverlays(this.el, this.budget, (message) => { this.overlayNotice = message; this.showStatus(); });
    this.offLevels = deps.store.subscribe(['levels'], () => {
      if (deps.store.levels) this.overlays.receive(deps.store.levels, deps.clock.now());
    });
    if (deps.store.levels) this.overlays.receive(deps.store.levels, deps.clock.now());
    doc.addEventListener('visibilitychange', this.visibility);
    this.host.addEventListener('pointerdown', this.retryVideo);
    this.source = deps.code ? new EditorSettingsSource(deps.code.surface, (parsed) => this.configure(parsed)) : null;
  }

  private configure(parsed: ParsedEditorSettings): void {
    if (this.disposed) return;
    const { background, visualizers } = parsed.settings;
    this.errors = parsed.errors;
    this.deps.code?.setBackgroundOpacity?.(background?.scrim ?? (visualizers.length ? 0.55 : 0.85));
    this.host.dataset.editorBackground = background?.kind ?? 'default';
    const key = JSON.stringify(background);
    if (key !== this.mediaKey) { this.mediaKey = key; this.setBackground(background); }
    this.overlays.configure(visualizers);
    this.loop?.dispose(); this.loop = null;
    if (visualizers.length) {
      this.loop = startFrameLoop({ core: { frame() {} }, host: { draw() {} }, clock: this.deps.clock,
        audible: this.deps.audible, scheduler: this.opts.scheduler ?? windowScheduler(this.host.ownerDocument.defaultView),
        onFrame: (time) => this.overlays.present(time, this.deps.tier === 'native' && !this.deps.audible) });
    }
    this.visibility(); this.showStatus();
  }

  private setBackground(settings: BackgroundSettings | null): void {
    this.releaseMedia();
    this.backgroundNotice = '';
    if (!settings || settings.kind === 'none') return;
    if (settings.kind === 'shader') {
      if (!this.opts.shaderAvailable) this.backgroundNotice = 'Shader backgrounds require the browser render host';
      else { this.shaderOutput = settings.output; this.opts.selectShader(settings.output); this.shaderChanged(); }
      this.host.style.setProperty('--vt-shader-opacity', String(settings.opacity));
      this.host.style.setProperty('--vt-shader-blur', `${settings.blur}px`);
      return;
    }
    const doc = this.host.ownerDocument;
    const revision = this.revision;
    const current = () => !this.disposed && revision === this.revision;
    const kind = settings.kind;
    let url: string;
    try { url = mediaUrl(settings.src!, kind, doc.baseURI); }
    catch (error) { this.backgroundNotice = String(error); return; }
    const media = kind === 'image' ? doc.createElement('img') : doc.createElement('video');
    this.media = media;
    media.className = 'vact-background-media';
    media.dataset.kind = kind;
    media.style.opacity = String(settings.opacity);
    media.style.objectFit = settings.fit;
    media.style.filter = `blur(${settings.blur}px)`;
    const fail = (message: string) => {
      if (!current()) return;
      this.revision++;
      this.backgroundNotice = message;
      media.hidden = true; media.onload = null; media.onerror = null;
      if (media instanceof HTMLVideoElement) { media.onloadedmetadata = null; media.pause(); }
      media.removeAttribute('src');
      if (media instanceof HTMLVideoElement) media.load();
      if (this.mediaBytes) { this.budget.release(this.mediaBytes); this.mediaBytes = 0; }
      this.showStatus();
    };
    media.onerror = () => fail(`Background ${kind} could not be loaded: ${settings.src!.startsWith('data:') ? 'embedded media' : settings.src!.slice(0, 160)}`);
    this.backgroundNotice = `Loading background ${kind}…`;
    if (media instanceof HTMLImageElement) {
      media.alt = ''; media.decoding = 'async'; media.referrerPolicy = 'no-referrer';
      media.onload = () => {
        if (!current() || this.mediaBytes) return;
        const pixels = media.naturalWidth * media.naturalHeight;
        if (!pixels || pixels > MAX_MEDIA_PIXELS || !this.budget.reserve(pixels * 4)) {
          fail('Background image exceeds the editor resource budget');
          media.removeAttribute('src'); return;
        }
        this.mediaBytes = pixels * 4;
        this.backgroundNotice = ''; this.showStatus();
      };
    } else {
      media.muted = true; media.volume = 0; media.loop = true; media.playsInline = true; media.preload = 'metadata';
      media.onloadedmetadata = () => {
        if (!current() || this.mediaBytes) return;
        const pixels = media.videoWidth * media.videoHeight;
        if (!pixels || pixels > MAX_MEDIA_PIXELS || !this.budget.reserve(pixels * 4)) {
          fail('Background video exceeds the editor resource budget');
          media.removeAttribute('src'); media.load(); return;
        }
        this.mediaBytes = pixels * 4;
        if (doc.visibilityState !== 'hidden') this.playVideo(media, revision);
      };
    }
    this.el.prepend(media);
    media.src = url;
  }

  shaderChanged(): void {
    if (this.shaderOutput === null) return;
    this.backgroundNotice = this.opts.hasShader?.(this.shaderOutput) === false
      ? `Awaiting shader output o${this.shaderOutput}; evaluate its visual chain` : '';
    this.showStatus();
  }

  private playVideo(video: HTMLVideoElement, revision: number): void {
    void video.play().then(() => {
      if (this.disposed || revision !== this.revision) return;
      this.backgroundNotice = ''; this.showStatus();
    }, () => {
      if (this.disposed || revision !== this.revision) return;
      this.backgroundNotice = 'Background video is paused by the browser; click the editor to start it'; this.showStatus();
    });
  }
  private readonly retryVideo = (): void => {
    if (this.media instanceof HTMLVideoElement && this.media.hasAttribute('src') && this.media.paused && this.host.ownerDocument.visibilityState !== 'hidden') this.playVideo(this.media, this.revision);
  };
  private readonly visibility = (): void => {
    const visible = this.host.ownerDocument.visibilityState !== 'hidden';
    this.loop?.setVisible(visible);
    if (this.media instanceof HTMLVideoElement) {
      if (!visible) this.media.pause();
      else if (this.media.readyState >= 1) this.playVideo(this.media, this.revision);
    }
  };
  private releaseMedia(): void {
    this.revision++;
    this.shaderOutput = null;
    if (this.media) {
      this.media.onload = null; this.media.onerror = null;
      if (this.media instanceof HTMLVideoElement) { this.media.onloadedmetadata = null; this.media.pause(); }
      this.media.removeAttribute('src');
      if (this.media instanceof HTMLVideoElement) this.media.load();
      this.media.remove(); this.media = null;
    }
    if (this.mediaBytes) { this.budget.release(this.mediaBytes); this.mediaBytes = 0; }
    this.host.style.removeProperty('--vt-shader-opacity');
    this.host.style.removeProperty('--vt-shader-blur');
  }
  private showStatus(): void {
    this.status.textContent = [...this.errors, this.backgroundNotice, this.overlayNotice].filter(Boolean).join('\n');
    this.status.hidden = !this.status.textContent;
  }
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.source?.dispose(); this.offLevels(); this.loop?.dispose(); this.overlays.dispose(); this.releaseMedia();
    this.host.ownerDocument.removeEventListener('visibilitychange', this.visibility);
    this.host.removeEventListener('pointerdown', this.retryVideo);
    delete this.host.dataset.editorBackground;
    this.deps.code?.setBackgroundOpacity?.(0.85);
    this.el.remove(); this.status.remove();
  }
}
