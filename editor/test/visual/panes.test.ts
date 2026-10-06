// The output panes and the visual area mount (design 9.3, 15.1.8): four
// panes, the selection, the diagnostic banner, the tier notices, and the
// wiring of `0x72` records and the frame loop to the WebGL2 host.

import { afterEach, describe, expect, it } from 'vitest';

import type { EditorDeps } from '../../src/app/deps';
import { buildLayout } from '../../src/app/layout';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import type { OutputIndex, RenderRecord } from '../../src/protocol/types';
import type { WasmCore } from '../../src/protocol/wasm';
import type { FrameScheduler } from '../../src/visual/frame';
import { mount, NATIVE_NOTICE, NO_WEBGL2_NOTICE } from '../../src/visual/mount';
import { VisualPanes } from '../../src/visual/panes';
import { installCanvasFakes, type CanvasFakes } from '../support/canvas';
import { MockClock } from '../support/clock';
import { RecordingGL } from '../support/gl';
import { RecordingTransport } from '../support/recording';

let fakes: CanvasFakes | null = null;

afterEach(() => {
  fakes?.restore();
  fakes = null;
  document.body.innerHTML = '';
});

function visible(root: HTMLElement): string[] {
  return [...root.querySelectorAll<HTMLElement>('[data-output]')]
    .filter((p) => !p.hidden)
    .map((p) => p.dataset.output ?? '');
}

describe('VisualPanes', () => {
  it('shows four panes, one selected output, and the 2x2 tiling', () => {
    fakes = installCanvasFakes();
    const parent = document.createElement('div');
    document.body.appendChild(parent);
    const source = document.createElement('canvas');
    const panes = new VisualPanes(parent, { source });
    expect(parent.querySelectorAll('[data-output]')).toHaveLength(4);
    expect(visible(parent)).toEqual(['o0']);

    const select = parent.querySelector<HTMLSelectElement>('[data-role="pane-select"]') as HTMLSelectElement;
    select.value = 'tile';
    select.dispatchEvent(new Event('change'));
    expect(panes.selection).toBe('tile');
    expect(visible(parent)).toEqual(['o0', 'o1', 'o2', 'o3']);
    expect(panes.el.dataset.selection).toBe('tile');

    panes.select(2);
    expect(select.value).toBe('o2');
    expect(visible(parent)).toEqual(['o2']);

    const presented: OutputIndex[] = [];
    panes.present({ present: (o) => void presented.push(o) });
    expect(presented).toEqual([2]);
    const paneCanvas = parent.querySelector<HTMLCanvasElement>('[data-output="o2"] canvas') as HTMLCanvasElement;
    expect(fakes.ctx(paneCanvas).named('drawImage')[0]?.args[0]).toBe(source);
  });

  it('shows a diagnostic banner until the output accepts a program again', () => {
    fakes = installCanvasFakes();
    const parent = document.createElement('div');
    const panes = new VisualPanes(parent, { source: document.createElement('canvas') });
    const banner = parent.querySelector<HTMLElement>('[data-role="diagnostic"]') as HTMLElement;
    expect(banner.hidden).toBe(true);
    panes.showDiagnostic({ code: 'shader-compile', out: 1, message: 'ERROR: 0:3: x' });
    expect(banner.hidden).toBe(false);
    expect(banner.textContent).toBe('o1: shader-compile: ERROR: 0:3: x');
    panes.clearDiagnostic(0);
    expect(banner.hidden).toBe(false);
    panes.clearDiagnostic(1);
    expect(banner.hidden).toBe(true);
  });

  it('exposes local video selection and status text', () => {
    fakes = installCanvasFakes();
    const parent = document.createElement('div');
    let selected: File | undefined;
    const panes = new VisualPanes(parent, { notice: 'no render', onVideoFile: (file) => { selected = file; } });
    const input = parent.querySelector<HTMLInputElement>('[data-role="video-input"]')!;
    expect(input.accept).toBe('video/*');
    const file = new File(['video'], 'background.mp4', { type: 'video/mp4' });
    Object.defineProperty(input, 'files', { configurable: true, value: [file] });
    input.dispatchEvent(new Event('change'));
    expect(selected).toBe(file);
    panes.showVideoStatus('video decode failed');
    expect(parent.querySelector('[data-role="video-status"]')?.textContent).toBe('video decode failed');
    panes.dispose();
  });
});

function deps(tier: 'browser' | 'native', core?: WasmCore): EditorDeps {
  const store = new Store();
  const d: EditorDeps = {
    client: new Client(new RecordingTransport(), { store }),
    store,
    clock: new MockClock(2),
    tier,
    files: new MemoryFiles(),
  };
  if (core) d.core = core;
  return d;
}

class FakeRenderCore {
  readonly frames: number[] = [];
  private readonly listeners: ((r: RenderRecord) => void)[] = [];

  onRender(cb: (r: RenderRecord) => void): () => void {
    this.listeners.push(cb);
    return () => this.listeners.splice(this.listeners.indexOf(cb), 1);
  }

  frame(now: number): void {
    this.frames.push(now);
  }

  emit(r: RenderRecord): void {
    for (const cb of this.listeners) cb(r);
  }

  get listening(): number {
    return this.listeners.length;
  }

  as(): WasmCore {
    return this as unknown as WasmCore;
  }
}

class StepRaf implements FrameScheduler {
  pending: ((frameMs: number) => void) | null = null;

  request(cb: (frameMs: number) => void): number {
    this.pending = cb;
    return 1;
  }

  cancel(): void {
    this.pending = null;
  }

  step(): void {
    const cb = this.pending;
    this.pending = null;
    cb?.(1000);
  }
}

const SOURCE = `#version 300 es
precision highp float;
uniform float time;
uniform vec2 resolution;
out vec4 fragColor;
void main() { fragColor = vec4(1.0); }
`;

describe('visual mount', () => {
  it('shows the browser-only notice on the native tier and sets deps.visual', () => {
    fakes = installCanvasFakes();
    const root = document.createElement('div');
    buildLayout(root);
    const d = deps('native');
    const m = mount(root, d);
    expect(root.querySelector('[data-role="notice"]')?.textContent).toBe(NATIVE_NOTICE);
    expect(root.querySelectorAll('[data-output]')).toHaveLength(0);
    expect(typeof d.visual?.mountSpectrum).toBe('function');
    expect(root.querySelector('[data-pane="analyzers"] [data-area="analyzers"]')).not.toBeNull();
    m.dispose();
    expect(d.visual).toBeUndefined();
    expect(root.querySelector('[data-area="visual"]')).toBeNull();
  });

  it('reports WebGL2 not available', () => {
    fakes = installCanvasFakes();
    const root = document.createElement('div');
    const core = new FakeRenderCore();
    const m = mount(root, deps('browser', core.as()), { scheduler: new StepRaf() });
    expect(root.querySelector('[data-role="notice"]')?.textContent).toBe(NO_WEBGL2_NOTICE);
    expect(core.listening).toBe(0);
    m.dispose();
  });

  it('routes render records to the WebGL2 host and runs frame-then-draw per frame', () => {
    const gl = new RecordingGL();
    fakes = installCanvasFakes({ webgl2: () => gl });
    const root = document.createElement('div');
    document.body.appendChild(root);
    const core = new FakeRenderCore();
    const raf = new StepRaf();
    const d = deps('browser', core.as());
    const m = mount(root, d, { scheduler: raf });
    const background: (HTMLCanvasElement | null)[] = [];
    const stopBackground = d.visual?.onBackgroundCanvas?.((canvas) => background.push(canvas));
    const canvas = background[0] as HTMLCanvasElement;
    expect(background).toEqual([canvas]);
    expect(root.querySelectorAll('[data-pane="visual"] [data-output]')).toHaveLength(4);

    core.emit({ op: 'program', out: 0, source: SOURCE, uniform_names: [], assets: [] });
    expect(gl.named('linkProgram')).toHaveLength(1);
    const mark = gl.calls.length;
    raf.step();
    expect(core.frames).toEqual([2]);
    expect(gl.since(mark).filter((c) => c.fn === 'drawArrays')).toHaveLength(1);
    expect(gl.since(mark).filter((c) => c.fn === 'blitFramebuffer')).toHaveLength(1);
    expect(background).toEqual([canvas]);
    const stopped: (HTMLCanvasElement | null)[] = [];
    const unsubscribe = d.visual?.onBackgroundCanvas?.((value) => stopped.push(value));
    unsubscribe?.();

    gl.failNextCompile('bad shader');
    core.emit({ op: 'program', out: 0, source: 'x', uniform_names: [], assets: [] });
    const banner = root.querySelector<HTMLElement>('[data-role="diagnostic"]') as HTMLElement;
    expect(banner.textContent).toBe('o0: shader-compile: bad shader');
    core.emit({ op: 'program', out: 0, source: SOURCE, uniform_names: [], assets: [] });
    expect(banner.hidden).toBe(true);

    m.dispose();
    expect(background).toEqual([canvas, null]);
    expect(stopped).toEqual([canvas]);
    stopBackground?.();
    expect(core.listening).toBe(0);
    expect(raf.pending).toBeNull();
    expect(root.querySelector('[data-area="visual"]')).toBeNull();
  });
});
