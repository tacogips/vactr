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

import type { SampleFrames } from '../app/apis';
import type { Tier } from '../app/deps';

export const SAMPLE_MAP_KEY = 'vactrol.sampleMap';

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
  private readonly soundsEl: HTMLElement;
  private readonly banksEl: HTMLElement | null = null;
  private readonly statusEl: HTMLElement;
  private readonly urlInput: HTMLInputElement | null = null;

  constructor(parent: HTMLElement, opts: SampleBrowserOptions) {
    this.opts = opts;
    const doc = parent.ownerDocument;
    this.el = doc.createElement('details');
    this.el.className = 'vact-samples';
    const summary = doc.createElement('summary');
    summary.textContent = 'samples';
    this.el.appendChild(summary);
    this.soundsEl = doc.createElement('ul');
    this.soundsEl.className = 'vact-sounds';
    this.el.appendChild(this.soundsEl);
    if (opts.tier === 'browser' && opts.library) {
      const row = doc.createElement('div');
      row.className = 'vact-sample-map';
      const input = doc.createElement('input');
      input.type = 'url';
      input.placeholder = 'sample map url';
      input.value = this.storedUrl() ?? '';
      const load = doc.createElement('button');
      load.type = 'button';
      load.textContent = 'load map';
      load.addEventListener('click', () => void this.loadMap(input.value));
      row.append(input, load);
      this.el.appendChild(row);
      this.urlInput = input;
      this.banksEl = doc.createElement('ul');
      this.banksEl.className = 'vact-banks';
      this.el.appendChild(this.banksEl);
    }
    this.statusEl = doc.createElement('div');
    this.statusEl.className = 'vact-samples-status';
    this.el.appendChild(this.statusEl);
    parent.appendChild(this.el);
  }

  /** Lists the manifest's sounds. */
  setSounds(sounds: readonly string[]): void {
    const doc = this.el.ownerDocument;
    this.soundsEl.replaceChildren(
      ...sounds.map((s) => {
        const li = doc.createElement('li');
        li.className = 'vact-sound';
        li.dataset.sound = s;
        li.textContent = s;
        return li;
      }),
    );
  }

  /** Opens the browser, scrolled to `bank` when given. */
  open(bank?: string): void {
    this.el.open = true;
    if (bank === undefined) return;
    const target =
      this.el.querySelector<HTMLElement>(`[data-bank="${attrValue(bank)}"]`) ??
      this.el.querySelector<HTMLElement>(`[data-sound="${attrValue(bank)}"]`);
    target?.scrollIntoView?.({ block: 'nearest' });
  }

  /** Loads the map at `url`, remembers the url, and lists its banks. */
  async loadMap(url: string): Promise<void> {
    const lib = this.opts.library;
    if (!lib || url.trim() === '') return;
    try {
      await lib.loadMap(url.trim());
      this.opts.storage?.setItem(SAMPLE_MAP_KEY, url.trim());
      if (this.urlInput) this.urlInput.value = url.trim();
      this.status(`${lib.banks().length} banks`);
      this.renderBanks();
    } catch (e) {
      this.status(String(e instanceof Error ? e.message : e));
    }
  }

  /** Loads one bank and draws its previews. */
  async loadBank(bank: string): Promise<void> {
    const lib = this.opts.library;
    if (!lib) return;
    try {
      const n = await lib.loadBank(bank);
      this.status(`${bank}: ${n} loaded`);
      this.renderBank(bank);
    } catch (e) {
      this.status(String(e instanceof Error ? e.message : e));
    }
  }

  dispose(): void {
    this.el.remove();
  }

  private storedUrl(): string | null {
    try {
      return this.opts.storage?.getItem(SAMPLE_MAP_KEY) ?? null;
    } catch {
      return null;
    }
  }

  private status(text: string): void {
    this.statusEl.textContent = text;
  }

  private renderBanks(): void {
    const lib = this.opts.library;
    const list = this.banksEl;
    if (!lib || !list) return;
    const doc = this.el.ownerDocument;
    list.replaceChildren(
      ...lib.banks().map((bank) => {
        const li = doc.createElement('li');
        li.className = 'vact-bank';
        li.dataset.bank = bank;
        const name = doc.createElement('span');
        name.textContent = `${bank} (${lib.map[bank]?.length ?? 0})`;
        const load = doc.createElement('button');
        load.type = 'button';
        load.textContent = 'load';
        load.addEventListener('click', () => void this.loadBank(bank));
        const entries = doc.createElement('ul');
        entries.className = 'vact-entries';
        li.append(name, load, entries);
        return li;
      }),
    );
  }

  private renderBank(bank: string): void {
    const lib = this.opts.library;
    const li = this.banksEl?.querySelector<HTMLElement>(`[data-bank="${attrValue(bank)}"]`);
    const entries = li?.querySelector<HTMLElement>('.vact-entries');
    if (!lib || !entries) return;
    const doc = this.el.ownerDocument;
    const count = lib.map[bank]?.length ?? 0;
    const items: HTMLElement[] = [];
    for (let i = 0; i < count; i += 1) {
      const frames = lib.frames(bank, i);
      const item = doc.createElement('li');
      item.className = 'vact-entry';
      item.dataset.key = `${bank}:${i}`;
      item.textContent = `${bank}:${i}`;
      if (frames) {
        const canvas = doc.createElement('canvas');
        canvas.className = 'vact-preview';
        canvas.width = 96;
        canvas.height = 24;
        if (drawWaveform(canvas, frames)) item.appendChild(canvas);
      } else item.dataset.missing = 'true';
      items.push(item);
    }
    entries.replaceChildren(...items);
  }
}

function attrValue(s: string): string {
  return s.replace(/["\\]/g, '\\$&');
}
