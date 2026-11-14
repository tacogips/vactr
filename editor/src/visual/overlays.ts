import type { ResourceBudget } from '../app/apis';
import type { LevelsBody } from '../protocol/types';
import { LevelsTimeline, drawMeter } from './meters';
import { drawScope, drawSpectrogram } from './scopes';
import { drawBars, masterLevel } from './spectrum';
import type { VisualizerSettings } from './settings';

const WIDTH = 1024;
const HEIGHT = 256;
const BYTES = WIDTH * HEIGHT * 4;
const HISTORY = 96;
interface Overlay { canvas: HTMLCanvasElement; ctx: CanvasRenderingContext2D | null; settings: VisualizerSettings }

/** Master telemetry overlays; no microphone, audio graph, or synthetic waveform. */
export class BackgroundOverlays {
  private readonly overlays: Overlay[] = [];
  private readonly timeline = new LevelsTimeline();
  private history: number[][] = [];
  private last: LevelsBody | null = null;
  private latest: LevelsBody | null = null;
  private message = '';
  private configNotice = '';
  private receivedAt = -Infinity;
  constructor(private readonly parent: HTMLElement, private readonly budget: ResourceBudget,
    private readonly onStatus: (status: string) => void) {}

  configure(settings: readonly VisualizerSettings[]): void {
    this.clear();
    for (const setting of settings) {
      if (!this.budget.reserve(BYTES)) { this.configNotice = 'Visualizer exceeds the editor resource budget'; this.status(''); break; }
      const canvas = this.parent.ownerDocument.createElement('canvas');
      canvas.className = 'vact-background-visualizer';
      canvas.dataset.kind = setting.kind;
      canvas.dataset.position = setting.position;
      canvas.setAttribute('aria-hidden', 'true');
      canvas.width = WIDTH; canvas.height = HEIGHT; canvas.hidden = true;
      canvas.style.opacity = String(setting.opacity);
      const ctx = canvas.getContext('2d');
      this.parent.appendChild(canvas);
      this.overlays.push({ canvas, ctx, settings: setting });
      if (!ctx) { this.configNotice = 'Canvas rendering is unavailable for editor visualizers'; this.status(''); }
    }
    if (this.overlays.length) this.status('Awaiting master output analysis');
  }
  receive(body: LevelsBody, now: number): void {
    this.latest = body;
    this.receivedAt = now;
    this.timeline.push(body, now);
  }
  present(time: number, latestOnReceipt = false): void {
    if (!this.overlays.length) return;
    const body = latestOnReceipt ? (time - this.receivedAt <= 2 ? this.latest : null) : this.timeline.at(time);
    if (!body) { this.hide('Awaiting master output analysis'); return; }
    if (body === this.last) return;
    this.last = body;
    const master = masterLevel(body);
    if (!master) { this.hide('Awaiting master output analysis'); return; }
    if (master.bands?.length) {
      this.history.push(master.bands.slice(0, 8));
      if (this.history.length > HISTORY) this.history.shift();
    }
    const missing: string[] = [];
    for (const overlay of this.overlays) {
      const { canvas, ctx, settings } = overlay;
      ctx?.clearRect(0, 0, WIDTH, HEIGHT);
      const available = settings.kind === 'oscilloscope'
        ? master.waveform?.length === 256
        : settings.kind === 'level' || !!master.bands?.length;
      canvas.hidden = !available || !ctx;
      if (!available) { missing.push(settings.kind); continue; }
      if (!ctx) continue;
      if (settings.kind === 'oscilloscope') drawScope(ctx, WIDTH, HEIGHT, master.waveform!);
      else if (settings.kind === 'spectrum') drawBars(ctx, WIDTH, HEIGHT, master.bands!);
      else if (settings.kind === 'spectrogram') drawSpectrogram(ctx, WIDTH, HEIGHT, this.history);
      else drawMeter(ctx, WIDTH, HEIGHT, [master.rms]);
    }
    this.status(missing.length ? `Awaiting master ${[...new Set(missing)].join(', ')} data (the audio host may need an update)` : '');
  }
  private hide(message: string): void {
    this.last = null;
    for (const { canvas } of this.overlays) canvas.hidden = true;
    this.status(message);
  }
  private status(message: string): void {
    message = [this.configNotice, message].filter(Boolean).join('\n');
    if (this.message !== message) { this.message = message; this.onStatus(message); }
  }
  private clear(): void {
    for (const { canvas } of this.overlays.splice(0)) { canvas.remove(); canvas.width = 0; canvas.height = 0; this.budget.release(BYTES); }
    this.history = []; this.last = null; this.configNotice = '';
    this.status('');
  }
  dispose(): void { this.clear(); }
}
