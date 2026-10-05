import { afterEach, describe, expect, it, vi } from 'vitest';

import { VideoBackground } from '../../src/visual/video';

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

function setup(size = { width: 1920, height: 1080 }, allowBudget = true) {
  vi.useFakeTimers();
  const video = document.createElement('video');
  Object.defineProperties(video, {
    videoWidth: { configurable: true, value: size.width },
    videoHeight: { configurable: true, value: size.height },
    currentTime: { configurable: true, writable: true, value: 0 },
    play: { configurable: true, value: vi.fn(() => Promise.resolve()) },
    pause: { configurable: true, value: vi.fn() },
    load: { configurable: true, value: vi.fn() },
  });
  const createElement = document.createElement.bind(document);
  vi.spyOn(document, 'createElement').mockImplementation(((tag: string, options?: ElementCreationOptions) =>
    tag === 'video' ? video : createElement(tag, options)) as typeof document.createElement);
  const reserve = vi.fn(() => allowBudget);
  const release = vi.fn();
  const statuses: string[] = [];
  const canvas = { width: 0, height: 0, getContext: () => ({ drawImage: vi.fn() }) } as unknown as HTMLCanvasElement;
  const revoked: string[] = [];
  vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:visual-test');
  vi.spyOn(URL, 'revokeObjectURL').mockImplementation((url) => revoked.push(url));
  const background = new VideoBackground(document, {
    ledger: { reserve, release, usedBytes: 0, limitBytes: 96 * 1024 * 1024 },
    createCanvas: () => canvas,
    onStatus: (s) => statuses.push(s),
  });
  return { video, background, reserve, release, statuses, canvas, revoked };
}

describe('VideoBackground', () => {
  it('loads a muted local URL and downsizes oversized frames to a reused capped canvas', async () => {
    const f = setup();
    await f.background.load(new Blob(['video'], { type: 'video/mp4' }));
    expect(f.video.muted).toBe(true);
    expect(f.video.volume).toBe(0);
    expect(f.video.playsInline).toBe(true);
    expect(f.video.loop).toBe(true);
    expect(f.video.src).toContain('blob:visual-test');
    f.video.currentTime = 1;
    const frame = f.background.frame();
    expect(frame?.source).toBe(f.canvas);
    expect(f.canvas.width).toBe(1280);
    expect(f.canvas.height).toBe(720);
    expect(frame?.revision).toBe(1);
    expect(f.reserve).toHaveBeenCalledWith(1280 * 720 * 4);
    f.background.dispose();
    expect(f.revoked).toEqual(['blob:visual-test']);
    expect(f.video.getAttribute('src')).toBeNull();
    expect(f.video.load).toHaveBeenCalled();
    expect(f.release).toHaveBeenCalledWith(1280 * 720 * 4);
  });

  it('accepts changed frames no faster than 30 Hz and pauses while hidden', async () => {
    const f = setup({ width: 640, height: 360 });
    await f.background.load(new Blob(['video']));
    f.video.currentTime = 1;
    expect(f.background.frame()?.revision).toBe(1);
    vi.advanceTimersByTime(10);
    f.video.currentTime = 1.1;
    expect(f.background.frame()).toBeNull();
    vi.advanceTimersByTime(24);
    f.video.currentTime = 1.2;
    expect(f.background.frame()?.revision).toBe(2);
    f.background.setVisible(false);
    expect(f.video.pause).toHaveBeenCalled();
    expect(f.background.frame()).toBeNull();
    f.background.dispose();
  });

  it('consumes each requestVideoFrameCallback notification once', async () => {
    const f = setup({ width: 640, height: 360 });
    let callback: ((now: number, metadata: { mediaTime?: number }) => void) | undefined;
    let callbackId = 0;
    Object.defineProperties(f.video, {
      requestVideoFrameCallback: { configurable: true, value: (cb: typeof callback) => { callback = cb; return ++callbackId; } },
      cancelVideoFrameCallback: { configurable: true, value: vi.fn() },
    });
    await f.background.load(new Blob(['video']));
    callback?.(0, { mediaTime: 1 });
    expect(f.background.frame()?.revision).toBe(1);
    expect(f.background.frame()).toBeNull();
    vi.advanceTimersByTime(40);
    expect(f.background.frame()).toBeNull();
    callback?.(40, { mediaTime: 2 });
    expect(f.background.frame()?.revision).toBe(2);
    f.background.dispose();
  });

  it('refuses uploads when the shared video budget is unavailable', async () => {
    const f = setup({ width: 640, height: 360 }, false);
    await f.background.load(new Blob(['video']));
    f.video.currentTime = 1;
    expect(f.background.frame()).toBeNull();
    expect(f.statuses).toContain('video exceeds GPU budget');
    expect(f.reserve).toHaveBeenCalledTimes(1);
    f.background.dispose();
    expect(f.release).not.toHaveBeenCalled();
  });

  it('reports decode errors and stops reporting after dispose', async () => {
    const f = setup({ width: 640, height: 360 });
    await f.background.load(new Blob(['video']));
    f.video.dispatchEvent(new Event('error'));
    expect(f.statuses).toContain('video decode failed');

    const statusCount = f.statuses.length;
    f.background.dispose();
    f.video.dispatchEvent(new Event('error'));
    expect(f.statuses).toHaveLength(statusCount);
  });

  it('reports a rejected play as a decode failure', async () => {
    const f = setup({ width: 640, height: 360 });
    Object.defineProperty(f.video, 'play', {
      configurable: true,
      value: vi.fn(() => Promise.reject(new Error('NotSupported'))),
    });
    await f.background.load(new Blob(['video']));
    await Promise.resolve();
    expect(f.statuses.some((status) => status.startsWith('video decode failed: '))).toBe(true);
    f.background.dispose();
  });
});
