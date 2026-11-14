import { isolateHistory } from '@codemirror/commands';
import { Text } from '@codemirror/state';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { EditorDeps } from '../../src/app/deps';
import { CodeSurface } from '../../src/code/surface';
import { DocumentSync } from '../../src/code/sync';
import { ResourceLedger } from 'emaki/engine';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { EditorBackground } from '../../src/visual/background';
import { MockClock } from '../support/clock';
import { RecordingTransport } from '../support/recording';
import { installCanvasFakes, type CanvasFakes } from '../support/canvas';

let fakes: CanvasFakes;
const cleanups: (() => void)[] = [];
beforeEach(() => {
  fakes = installCanvasFakes();
  vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue();
  vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => {});
  vi.spyOn(HTMLMediaElement.prototype, 'load').mockImplementation(() => {});
});
afterEach(() => { for (const cleanup of cleanups.splice(0).reverse()) cleanup(); fakes.restore(); vi.restoreAllMocks(); document.body.innerHTML = ''; });
function setup() {
  const parent = document.createElement('div'); parent.innerHTML = '<div class="vact-code"><canvas class="vact-code-canvas"></canvas></div>';
  const statusParent = document.createElement('div'); document.body.append(parent, statusParent);
  const store = new Store(); const recording = new RecordingTransport(); const client = new Client(recording, { store });
  const surface = new CodeSurface({ sync: new DocumentSync(client.document('main.vact'), Text.empty) });
  const opacity = vi.fn(); const budget = new ResourceLedger();
  const deps = { store, client, clock: new MockClock(0), tier: 'native', files: new MemoryFiles(), resourceBudget: budget,
    code: { surface, setBackgroundOpacity: opacity } } as unknown as EditorDeps;
  const background = new EditorBackground(parent, statusParent, deps, { shaderAvailable: false, selectShader() {},
    scheduler: { request: () => 1, cancel() {} } });
  const dispose = () => { background.dispose(); surface.dispose(); client.close(); };
  cleanups.push(dispose);
  const replace = (text: string) => surface.dispatch({ changes: { from: 0, to: surface.state.doc.length, insert: text }, annotations: isolateHistory.of('full') });
  return { parent, background, budget, opacity, recording, replace, surface, dispose };
}
const comment = (value: unknown) => `#@ editor ${JSON.stringify(value)}`;
function dimensions(element: HTMLElement, widthKey: string, heightKey: string, width: number, height: number) {
  Object.defineProperties(element, { [widthKey]: { configurable: true, value: width }, [heightKey]: { configurable: true, value: height } });
}

describe('comment-driven editor media', () => {
  it('loads an image below code, applies presentation options, and removes/restores it without evaluating music', () => {
    const f = setup();
    const text = `${comment({ background: { kind: 'image', src: '/cover.png', opacity: 0.8, blur: 3, scrim: 0.4 } })}\nlet x 1`;
    f.replace(text);
    const image = f.parent.querySelector('img')!;
    expect(image.src).toContain('/cover.png'); expect(image.style.opacity).toBe('0.8'); expect(image.style.filter).toBe('blur(3px)');
    dimensions(image, 'naturalWidth', 'naturalHeight', 100, 50); image.dispatchEvent(new Event('load'));
    expect(f.budget.usedBytes).toBe(20_000); expect(f.background.status.hidden).toBe(true);
    expect(f.opacity).toHaveBeenLastCalledWith(0.4);
    expect(f.parent.querySelector('.vact-editor-background')?.nextElementSibling?.className).toBe('vact-code-canvas');
    f.replace('let x 1');
    expect(f.parent.querySelector('img')).toBeNull(); expect(image.hasAttribute('src')).toBe(false); expect(f.budget.usedBytes).toBe(0);
    expect(f.opacity).toHaveBeenLastCalledWith(0.85);
    expect(f.surface.undo()).toBe(true); expect(f.parent.querySelector('img')).not.toBeNull();
    expect(f.recording.kinds()).not.toContain('eval'); expect(f.recording.kinds()).not.toContain('apply-song');
  });

  it('ignores stale image completions and surfaces malformed comments, unsafe URLs and load failures', () => {
    const f = setup(); f.replace(comment({ background: { kind: 'image', src: '/old.png' } }));
    const old = f.parent.querySelector('img')!; const oldLoad = old.onload!;
    f.replace(comment({ background: { kind: 'image', src: '/new.png' } }));
    dimensions(old, 'naturalWidth', 'naturalHeight', 100, 100); oldLoad.call(old, new Event('load'));
    expect(f.budget.usedBytes).toBe(0); expect(f.parent.querySelector('img')?.src).toContain('/new.png');
    f.parent.querySelector('img')!.dispatchEvent(new Event('error'));
    expect(f.background.status.textContent).toContain('could not be loaded');
    f.replace(comment({ background: { kind: 'image', src: 'javascript:alert(1)' } }));
    expect(f.parent.querySelector('img')).toBeNull(); expect(f.background.status.textContent).toContain('scheme');
    f.replace('#@ editor {'); expect(f.background.status.textContent).toContain('Line 1:');
    expect(f.surface.state.doc.toString()).toBe('#@ editor {');
  });

  it('keeps videos muted/inline/looping, suspends them while hidden and releases them on removal/disposal', async () => {
    let visibility: DocumentVisibilityState = 'visible';
    vi.spyOn(document, 'visibilityState', 'get').mockImplementation(() => visibility);
    const f = setup(); f.replace(comment({ background: { kind: 'video', src: '/loop.webm' } }));
    const video = f.parent.querySelector('video')!;
    dimensions(video, 'videoWidth', 'videoHeight', 640, 360);
    expect(video.muted).toBe(true); expect(video.volume).toBe(0); expect(video.playsInline).toBe(true); expect(video.loop).toBe(true);
    video.dispatchEvent(new Event('loadedmetadata')); await Promise.resolve();
    expect(video.play).toHaveBeenCalled(); expect(f.background.status.hidden).toBe(true);
    visibility = 'hidden'; document.dispatchEvent(new Event('visibilitychange')); expect(video.pause).toHaveBeenCalled();
    visibility = 'visible'; Object.defineProperty(video, 'readyState', { value: 1 }); document.dispatchEvent(new Event('visibilitychange'));
    await Promise.resolve(); expect(video.play).toHaveBeenCalledTimes(2);
    f.replace('let x 1'); expect(video.hasAttribute('src')).toBe(false); expect(f.budget.usedBytes).toBe(0);
    expect(f.parent.querySelector('video')).toBeNull();
    f.dispose(); expect(f.parent.querySelector('.vact-editor-background')).toBeNull();
  });

  it('reports an autoplay failure and retries from an editor gesture', async () => {
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');
    const play = vi.mocked(HTMLMediaElement.prototype.play); play.mockRejectedValueOnce(new Error('blocked'));
    const f = setup(); f.replace(comment({ background: { kind: 'video', src: '/loop.mp4' } }));
    const video = f.parent.querySelector('video')!; dimensions(video, 'videoWidth', 'videoHeight', 640, 360);
    video.dispatchEvent(new Event('loadedmetadata')); await Promise.resolve();
    expect(f.background.status.textContent).toContain('click the editor');
    f.parent.querySelector('.vact-code')!.dispatchEvent(new Event('pointerdown')); await Promise.resolve();
    expect(f.background.status.hidden).toBe(true);
  });
});
