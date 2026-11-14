import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  SAMPLE_MAP_KEY,
  SampleBrowser,
  SampleLibrary,
  interleave,
  parseSampleMap,
  type DecodedAudio,
  type FetchLike,
} from '../../src/code/samples';

function decoded(channels: Float32Array[], rate = 48000): DecodedAudio {
  return {
    numberOfChannels: channels.length,
    sampleRate: rate,
    length: channels[0]?.length ?? 0,
    getChannelData: (c) => channels[c] ?? new Float32Array(0),
  };
}

/** A fetch over an in-memory table; records every url. */
function fakeFetch(table: Record<string, unknown>): FetchLike & { urls: string[] } {
  const urls: string[] = [];
  const f = (async (url: string) => {
    urls.push(url);
    const body = table[url];
    if (body === undefined) return { ok: false, status: 404, json: async () => null, arrayBuffer: async () => new ArrayBuffer(0) };
    return {
      ok: true,
      status: 200,
      json: async () => body,
      arrayBuffer: async () => (body instanceof ArrayBuffer ? body : new ArrayBuffer(0)),
    };
  }) as FetchLike & { urls: string[] };
  f.urls = urls;
  return f;
}

function memoryStorage(): Pick<Storage, 'getItem' | 'setItem'> & { data: Map<string, string> } {
  const data = new Map<string, string>();
  return { data, getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v) };
}

const parents: HTMLElement[] = [];
function parent(): HTMLElement {
  const p = document.createElement('div');
  document.body.appendChild(p);
  parents.push(p);
  return p;
}

afterEach(() => {
  for (const p of parents.splice(0)) p.remove();
  vi.restoreAllMocks();
});

describe('sample map', () => {
  it('validates and resolves entry urls against the map url', () => {
    expect(parseSampleMap({ bd: ['a.wav', 'https://cdn/x.wav'] }, 'https://h/s/map.json')).toEqual({
      bd: ['https://h/s/a.wav', 'https://cdn/x.wav'],
    });
    expect(() => parseSampleMap([])).toThrow();
    expect(() => parseSampleMap({ bd: 'a.wav' })).toThrow();
    expect(() => parseSampleMap({ bd: [1] })).toThrow();
  });

  it('interleaves decoded channels', () => {
    const out = interleave(decoded([new Float32Array([1, 2]), new Float32Array([-1, -2])]));
    expect([...out]).toEqual([1, -1, 2, -2]);
  });
});

describe('SampleLibrary', () => {
  it('loadBank decodes each entry and hands it over as bank:index via samplePut', async () => {
    const wav = new ArrayBuffer(8);
    const fetch = fakeFetch({ 'https://h/map.json': { bd: ['https://h/bd0.wav', 'https://h/bd1.wav'] }, 'https://h/bd0.wav': wav, 'https://h/bd1.wav': wav });
    const decode = vi.fn(async () => decoded([new Float32Array([0.5, -0.5, 0.25]), new Float32Array([0, 0, 0])], 44100));
    const samplePut = vi.fn();
    const lib = new SampleLibrary({ core: { samplePut }, fetch, decode });
    expect(await lib.loadMap('https://h/map.json')).toEqual({ bd: ['https://h/bd0.wav', 'https://h/bd1.wav'] });
    expect(await lib.loadBank('bd')).toBe(2);
    expect(decode).toHaveBeenCalledTimes(2);
    expect(samplePut.mock.calls.map((c) => [c[0], c[2], c[3]])).toEqual([
      ['bd:0', 44100, 2],
      ['bd:1', 44100, 2],
    ]);
    expect([...(samplePut.mock.calls[0]?.[1] as Float32Array)]).toEqual([0.5, 0, -0.5, 0, 0.25, 0]);
    const frames = lib.frames('bd', 1);
    expect(frames?.rate).toBe(44100);
    expect(frames?.channels).toBe(2);
    expect(frames?.data.length).toBe(6);
    expect(lib.frames('bd', 2)).toBeNull();
  });

  it('skips entries that fail to fetch or decode', async () => {
    const fetch = fakeFetch({ 'https://h/m.json': { sd: ['https://h/missing.wav', 'https://h/bad.wav', 'https://h/ok.wav'] }, 'https://h/bad.wav': new ArrayBuffer(1), 'https://h/ok.wav': new ArrayBuffer(2) });
    let n = 0;
    const decode = vi.fn(async () => {
      n += 1;
      if (n === 1) throw new Error('corrupt');
      return decoded([new Float32Array([1])]);
    });
    const samplePut = vi.fn();
    const lib = new SampleLibrary({ core: { samplePut }, fetch, decode });
    await lib.loadMap('https://h/m.json');
    expect(await lib.loadBank('sd')).toBe(1);
    expect(samplePut.mock.calls.map((c) => c[0])).toEqual(['sd:2']);
    await expect(lib.loadBank('nope')).rejects.toThrow();
  });
});

describe('SampleBrowser', () => {
  it('lists the manifest sounds', () => {
    const p = parent();
    const b = new SampleBrowser(p, { tier: 'browser', library: new SampleLibrary({}), storage: memoryStorage() });
    b.setSounds(['bd', 'sd', 'hh']);
    expect([...p.querySelectorAll<HTMLElement>('.vact-sound')].map((e) => e.dataset.sound)).toEqual(['bd', 'sd', 'hh']);
    b.open('sd');
    expect(b.el.open).toBe(true);
  });

  it('keeps the map url in storage and draws previews when a 2D context exists', async () => {
    const storage = memoryStorage();
    const fetch = fakeFetch({ 'https://h/map.json': { bd: ['https://h/0.wav'] }, 'https://h/0.wav': new ArrayBuffer(4) });
    const decode = vi.fn(async () => decoded([new Float32Array([0, 1, -1, 0])]));
    const ctx = { clearRect: vi.fn(), beginPath: vi.fn(), moveTo: vi.fn(), lineTo: vi.fn(), stroke: vi.fn() };
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(ctx as unknown as CanvasRenderingContext2D);
    const p = parent();
    const lib = new SampleLibrary({ core: { samplePut: vi.fn() }, fetch, decode });
    const b = new SampleBrowser(p, { tier: 'browser', library: lib, storage });
    await b.loadMap('https://h/map.json');
    expect(storage.data.get(SAMPLE_MAP_KEY)).toBe('https://h/map.json');
    await b.loadBank('bd');
    const entry = p.querySelector<HTMLElement>('[data-key="bd:0"]');
    expect(entry?.querySelector('canvas.vact-preview')).not.toBeNull();
    expect(ctx.stroke).toHaveBeenCalled();
    // A second browser restores the url.
    const b2 = new SampleBrowser(parent(), { tier: 'browser', library: lib, storage });
    expect(b2.el.querySelector<HTMLInputElement>('input')?.value).toBe('https://h/map.json');
  });

  it('skips the preview when no 2D context is available', async () => {
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
    const fetch = fakeFetch({ 'https://h/map.json': { bd: ['https://h/0.wav'] }, 'https://h/0.wav': new ArrayBuffer(4) });
    const lib = new SampleLibrary({ fetch, decode: async () => decoded([new Float32Array([0])]) });
    const p = parent();
    const b = new SampleBrowser(p, { tier: 'browser', library: lib, storage: null });
    await b.loadMap('https://h/map.json');
    await b.loadBank('bd');
    expect(p.querySelector('[data-key="bd:0"]')).not.toBeNull();
    expect(p.querySelector('canvas')).toBeNull();
  });

  it('shows names only on the native tier: no map input and no previews', () => {
    const p = parent();
    const b = new SampleBrowser(p, { tier: 'native', storage: memoryStorage() });
    b.setSounds(['bd']);
    expect(p.querySelector('input')).toBeNull();
    expect(p.querySelector('.vact-banks')).toBeNull();
    expect(p.querySelector('canvas')).toBeNull();
    expect(p.querySelector('[data-sound="bd"]')?.textContent).toBe('bd');
  });
});
