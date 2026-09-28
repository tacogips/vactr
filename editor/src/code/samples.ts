// The sample bank browser and the browser-tier sample library (design
// 15.1.5 "Sample bank browser", 16.1).
//
// - The browser lists `manifest.sounds` on both tiers.
// - Browser tier: `SampleLibrary.loadMap(url)` fetches a user-entered map
//   `{"<bank>": ["<url>", ...]}` (entry urls resolve against the map url);
//   `loadBank(bank)` fetches and decodes each entry with `decodeAudioData`
//   and hands it to the session as `bank:index` via `WasmCore.samplePut`
//   (admission and the arena are 16.1's). `frames(bank, index)` returns the
//   decoded interleaved frames (previews, the ED-PARAMS sampler editor).
//   Each loaded entry shows a small canvas waveform preview, skipped when
//   no 2D context is available. The map url is kept in `localStorage`.
// - Native tier: names only (no map, no previews).

import { createComponent, createSignal, type Setter } from 'solid-js';
import { render } from 'solid-js/web';
import { SampleView, type BankView } from './sample-view';
import type { SampleFrames } from '../app/apis';
import type { Tier } from '../app/deps';

export const SAMPLE_MAP_KEY = 'vactr.sampleMap';

export type SampleMap = Record<string, string[]>;

/** The part of an `AudioBuffer` the library reads. */
export interface DecodedAudio {
  numberOfChannels: number;
  sampleRate: number;
  length: number;
  getChannelData(channel: number): Float32Array;
}

export interface SampleSink {
  samplePut(key: string, frames: Float32Array, rate: number, channels: number): void;
}

export type FetchLike = (url: string) => Promise<{ ok: boolean; status: number; json(): Promise<unknown>; arrayBuffer(): Promise<ArrayBuffer> }>;

export interface SampleLibraryOptions {
  core?: SampleSink;
  fetch?: FetchLike;
  /** `AudioContext.decodeAudioData`. */
  decode?: (bytes: ArrayBuffer) => Promise<DecodedAudio>;
}

/** Validates a sample map document. */
export function parseSampleMap(doc: unknown, base?: string): SampleMap {
  if (typeof doc !== 'object' || doc === null || Array.isArray(doc)) throw new Error('sample map: not an object');
  const out: SampleMap = {};
  for (const [bank, urls] of Object.entries(doc as Record<string, unknown>)) {
    if (!Array.isArray(urls) || !urls.every((u) => typeof u === 'string')) {
      throw new Error(`sample map: bank ${bank} is not a list of urls`);
    }
    out[bank] = (urls as string[]).map((u) => (base ? new URL(u, base).href : u));
  }
  return out;
}

/** Interleaves the channels of a decoded buffer. */
export function interleave(buf: DecodedAudio): Float32Array {
  const ch = Math.max(1, buf.numberOfChannels);
  const out = new Float32Array(buf.length * ch);
  for (let c = 0; c < ch; c += 1) {
    const data = buf.getChannelData(c);
    for (let i = 0; i < buf.length; i += 1) out[i * ch + c] = data[i] ?? 0;
  }
  return out;
}

export class SampleLibrary {
  private readonly opts: SampleLibraryOptions;
  private mapDoc: SampleMap = {};
  private readonly decoded = new Map<string, SampleFrames>();

  constructor(opts: SampleLibraryOptions) {
    this.opts = opts;
  }

  get map(): SampleMap {
    return this.mapDoc;
  }

  banks(): string[] {
    return Object.keys(this.mapDoc).sort();
  }

  async loadMap(url: string): Promise<SampleMap> {
    const res = await this.fetch(url);
    if (!res.ok) throw new Error(`sample map: HTTP ${res.status}`);
    const base = typeof location === 'undefined' ? undefined : new URL(url, location.href).href;
    this.mapDoc = parseSampleMap(await res.json(), base);
    return this.mapDoc;
  }

  /** Fetches, decodes and hands over every entry of `bank`; returns the count loaded. */
  async loadBank(bank: string): Promise<number> {
    const urls = this.mapDoc[bank];
    if (!urls) throw new Error(`sample map: no bank ${bank}`);
    const decode = this.opts.decode;
    if (!decode) throw new Error('sample library: no decoder');
    let loaded = 0;
    for (const [index, url] of urls.entries()) {
      const res = await this.fetch(url);
      if (!res.ok) continue;
      let buf: DecodedAudio;
      try {
        buf = await decode(await res.arrayBuffer());
      } catch {
        continue;
      }
      const frames: SampleFrames = { data: interleave(buf), rate: buf.sampleRate, channels: Math.max(1, buf.numberOfChannels) };
      this.decoded.set(`${bank}:${index}`, frames);
      this.opts.core?.samplePut(`${bank}:${index}`, frames.data, frames.rate, frames.channels);
      loaded += 1;
    }
    return loaded;
  }

  frames(bank: string, index: number): SampleFrames | null {
    return this.decoded.get(`${bank}:${index}`) ?? null;
  }

  private fetch(url: string): ReturnType<FetchLike> {
    const f = this.opts.fetch ?? (globalThis.fetch as unknown as FetchLike | undefined);
    if (!f) return Promise.reject(new Error('no fetch'));
    return f(url);
  }
}

/** Draws `frames` (first channel) as a min/max waveform; false without a 2D context. */
export function drawWaveform(canvas: HTMLCanvasElement, frames: SampleFrames): boolean {
  let ctx: CanvasRenderingContext2D | null = null;
  try {
    ctx = canvas.getContext('2d');
  } catch {
    ctx = null;
  }
  if (!ctx) return false;
  const { width, height } = canvas;
  const n = Math.floor(frames.data.length / frames.channels);
  ctx.clearRect(0, 0, width, height);
  ctx.beginPath();
  for (let x = 0; x < width; x += 1) {
    const a = Math.floor((x * n) / width);
    const b = Math.max(a + 1, Math.floor(((x + 1) * n) / width));
    let lo = 0;
    let hi = 0;
    for (let i = a; i < b && i < n; i += 1) {
      const v = frames.data[i * frames.channels] ?? 0;
      lo = Math.min(lo, v);
      hi = Math.max(hi, v);
    }
    ctx.moveTo(x + 0.5, ((1 - hi) * height) / 2);
    ctx.lineTo(x + 0.5, ((1 - lo) * height) / 2);
  }
  ctx.stroke();
  return true;
}

export interface SampleBrowserOptions {
  tier: Tier;
  library?: SampleLibrary;
  storage?: Pick<Storage, 'getItem' | 'setItem'> | null;
}

export class SampleBrowser {
  readonly el: HTMLDetailsElement;
  private readonly opts: SampleBrowserOptions;
  private readonly setSoundsSignal: Setter<readonly string[]>;
  private readonly setBanks: Setter<BankView[]>;
  private readonly setStatus: Setter<string>;
  private readonly disposeView: () => void;

  constructor(parent: HTMLElement, opts: SampleBrowserOptions) {
    this.opts = opts;
    const holder = parent.ownerDocument.createElement('div');
    const [sounds, setSounds] = createSignal<readonly string[]>([]);
    const [banks, setBanks] = createSignal<BankView[]>([]);
    const [status, setStatus] = createSignal('');
    this.setSoundsSignal = setSounds;
    this.setBanks = setBanks;
    this.setStatus = setStatus;
    this.disposeView = render(() => createComponent(SampleView, {
      tier: opts.tier,
      browserLibrary: opts.library !== undefined,
      url: this.storedUrl() ?? '',
      sounds, banks, status,
      onLoadMap: (url) => void this.loadMap(url),
      onLoadBank: (bank) => void this.loadBank(bank),
      preview: (canvas, frames) => {
        queueMicrotask(() => { if (!drawWaveform(canvas, frames)) canvas.remove(); });
      },
    }), holder);
    this.el = holder.firstElementChild as HTMLDetailsElement;
    parent.appendChild(this.el);
  }

  setSounds(sounds: readonly string[]): void {
    this.setSoundsSignal([...sounds]);
  }

  open(bank?: string): void {
    this.el.open = true;
    if (bank === undefined) return;
    const target =
      this.el.querySelector<HTMLElement>(`[data-bank="${attrValue(bank)}"]`) ??
      this.el.querySelector<HTMLElement>(`[data-sound="${attrValue(bank)}"]`);
    target?.scrollIntoView?.({ block: 'nearest' });
  }

  async loadMap(url: string): Promise<void> {
    const lib = this.opts.library;
    if (!lib || url.trim() === '') return;
    try {
      await lib.loadMap(url.trim());
      this.opts.storage?.setItem(SAMPLE_MAP_KEY, url.trim());
      const input = this.el.querySelector<HTMLInputElement>('input');
      if (input) input.value = url.trim();
      this.setStatus(`${lib.banks().length} banks`);
      this.setBanks(lib.banks().map((name) => ({ name, count: lib.map[name]?.length ?? 0, entries: [] })));
    } catch (error) {
      this.setStatus(String(error instanceof Error ? error.message : error));
    }
  }

  async loadBank(bank: string): Promise<void> {
    const lib = this.opts.library;
    if (!lib) return;
    try {
      const loaded = await lib.loadBank(bank);
      this.setStatus(`${bank}: ${loaded} loaded`);
      this.setBanks((old) => old.map((item) => item.name !== bank ? item : {
        ...item,
        entries: Array.from({ length: lib.map[bank]?.length ?? 0 }, (_, index) => ({
          key: `${bank}:${index}`, frames: lib.frames(bank, index),
        })),
      }));
    } catch (error) {
      this.setStatus(String(error instanceof Error ? error.message : error));
    }
  }

  dispose(): void {
    this.disposeView();
    this.el.remove();
  }

  private storedUrl(): string | null {
    try {
      return this.opts.storage?.getItem(SAMPLE_MAP_KEY) ?? null;
    } catch {
      return null;
    }
  }
}

function attrValue(s: string): string {
  return s.replace(/["\\]/g, '\\$&');
}
