// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import type { EditorDeps } from '../../src/app/deps';
import { mount } from '../../src/code/mount';
import { AudibleClock } from '../../src/app/clock';
import * as PerfHook from '../../src/code/perf-hook';
import type { VactrPerf } from '../../src/code/perf-hook';
import { buildLayout } from '../../src/app/layout';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { MockClock } from '../support/clock';
import { RecordingTransport } from '../support/recording';
import { installCanvasFakes, type CanvasFakes } from '../support/canvas';
import { CanvasRenderer } from '../../src/code/renderer';

interface RafHost {
  requestAnimationFrame(cb: FrameRequestCallback): number;
  cancelAnimationFrame(id: number): void;
  devicePixelRatio: number;
  innerHeight: number;
  callbacks: Map<number, FrameRequestCallback>;
  step(time: number): void;
}
function rafHost(): RafHost {
  let next = 1;
  const callbacks = new Map<number, FrameRequestCallback>();
  return { callbacks, devicePixelRatio: 1, innerHeight: 800,
    requestAnimationFrame(cb) { const id = next++; callbacks.set(id, cb); return id; },
    cancelAnimationFrame(id) { callbacks.delete(id); },
    step(time) { const row = callbacks.entries().next().value as [number, FrameRequestCallback] | undefined; if (!row) return; callbacks.delete(row[0]); row[1](time); },
  };
}

function rendererGL(): WebGL2RenderingContext {
  let next = 1;
  const gl: Record<string, unknown> = {};
  const constants = ['MAX_TEXTURE_SIZE', 'TEXTURE_2D', 'TEXTURE_MIN_FILTER', 'TEXTURE_MAG_FILTER', 'LINEAR', 'NEAREST',
    'TEXTURE_WRAP_S', 'TEXTURE_WRAP_T', 'CLAMP_TO_EDGE', 'UNPACK_PREMULTIPLY_ALPHA_WEBGL', 'RGBA', 'UNSIGNED_BYTE',
    'VERTEX_SHADER', 'FRAGMENT_SHADER', 'COMPILE_STATUS', 'LINK_STATUS', 'ARRAY_BUFFER', 'STATIC_DRAW', 'FLOAT',
    'FRAMEBUFFER', 'SCISSOR_TEST', 'BLEND', 'ONE', 'ONE_MINUS_SRC_ALPHA', 'COLOR_BUFFER_BIT', 'TEXTURE0', 'TRIANGLES'];
  constants.forEach((name, index) => { gl[name] = index + 1; }); gl.NO_ERROR = 0;
  for (const kind of ['Texture', 'Shader', 'Program', 'Buffer', 'VertexArray']) {
    gl[`create${kind}`] = () => ({ id: next++ }); gl[`delete${kind}`] = () => {};
  }
  for (const name of ['texParameteri', 'pixelStorei', 'shaderSource', 'compileShader', 'attachShader', 'linkProgram',
    'bindVertexArray', 'bindBuffer', 'bufferData', 'enableVertexAttribArray', 'vertexAttribPointer', 'bindFramebuffer',
    'useProgram', 'viewport', 'disable', 'enable', 'blendFunc', 'clearColor', 'clear', 'uniform2f', 'uniform1i',
    'activeTexture', 'scissor', 'drawArrays', 'bindTexture', 'texImage2D', 'texSubImage2D', 'uniform4f']) gl[name] = () => {};
  gl.getParameter = () => 4096; gl.getShaderParameter = () => true; gl.getProgramParameter = () => true;
  gl.getShaderInfoLog = () => ''; gl.getProgramInfoLog = () => ''; gl.getAttribLocation = () => 0;
  gl.getUniformLocation = (_program: unknown, name: string) => ({ name }); gl.getError = () => gl.NO_ERROR;
  gl.isContextLost = () => false; gl.isTexture = () => true;
  return gl as unknown as WebGL2RenderingContext;
}

interface Rig { root: HTMLElement; code: HTMLElement; deps: EditorDeps; mounted: ReturnType<typeof mount>; transport: RecordingTransport; fakes: CanvasFakes; host: RafHost; gl?: WebGL2RenderingContext }
const rigs: Rig[] = [];
function setup(perf = false, withGl = false, audible?: AudibleClock): Rig {
  const gl = withGl ? rendererGL() : undefined;
  const fakes = installCanvasFakes(withGl ? { webgl2: () => gl! } : {});
  const context = fakes.ctx(document.createElement('canvas')) as unknown as Record<string, unknown>;
  Object.assign(Object.getPrototypeOf(context) as object, {
    measureText: (text: string) => ({ width: text.length * 8 }),
    setTransform() {}, save() {}, restore() {}, rect() {}, clip() {},
  });
  const root = document.createElement('div'); document.body.append(root);
  const layout = buildLayout(root); const store = new Store(); const transport = new RecordingTransport();
  const deps: EditorDeps = { client: new Client(transport, { store }), store, clock: new MockClock(), tier: 'browser', files: new MemoryFiles(), ...(audible ? { audible } : {}) };
  const host = rafHost();
  if (perf) window.history.replaceState({}, '', '?perf=1');
  const mounted = mount(root, deps, { frameHost: host, ...(gl ? { gl } : {}) });
  const rig = { root, code: layout.code, deps, mounted, transport, fakes, host, ...(withGl ? { gl } : {}) };
  rigs.push(rig); return rig;
}

afterEach(() => {
  for (const rig of rigs.splice(0)) { rig.mounted.dispose(); rig.deps.client.close(); rig.deps.store.dispose(); rig.root.remove(); rig.fakes.restore(); }
  window.history.replaceState({}, '', '/'); vi.useRealTimers(); document.body.replaceChildren();
});

describe('headless canvas mount', () => {
  it('edits through the accessibility bridge and keeps visible source off the DOM', () => {
    const rig = setup(); const surface = rig.deps.code?.surface;
    expect(surface).toBeDefined();
    const textarea = rig.code.querySelector('textarea[aria-label="Code editor"]') as HTMLTextAreaElement;
    textarea.dispatchEvent(new InputEvent('beforeinput', { inputType: 'insertText', data: 'let x 1', bubbles: true, cancelable: true }));
    expect(surface?.state.doc.toString()).toBe('let x 1');
    expect(rig.code.textContent).not.toContain('let x 1');
    expect(rig.code.querySelector('canvas.vact-code-canvas')).not.toBeNull();
    rig.host.step(16);
    expect(rig.host.callbacks.size).toBe(0);
  });

  it('keeps editing available when WebGL2 is unavailable and releases resources on dispose', () => {
    const rig = setup(true); const perf = (window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf;
    expect(rig.code.querySelector('.vact-code-gpu-status')?.textContent).toContain('GPU unavailable');
    expect(perf).toBeDefined();
    rig.deps.code?.surface.dispatch({ changes: { from: 0, insert: 'a' } });
    expect(perf?.doc()).toBe('a');
    rig.mounted.dispose();
    expect(perf?.ledger.usedBytes).toBe(0);
    expect((window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf).toBeUndefined();
    expect(rig.deps.code).toBeUndefined();
  });

  it('installs no performance hook without the query flag', () => {
    const install = vi.spyOn(PerfHook, 'installPerfHook');
    const rig = setup();
    expect((window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf).toBeUndefined();
    expect(install).not.toHaveBeenCalled();
    rig.mounted.dispose();
  });

  it('keeps active playing frames separate from text work and records the audible presentation contract', () => {
    const audible = new AudibleClock({ at: () => ({ time: 1.05, uncertainty: 0.002, provenance: 'measured' }) });
    const rig = setup(true, true, audible); const perf = (window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf;
    expect(perf).toBeDefined();
    const surface = rig.deps.code!.surface;
    surface.dispatch({ changes: { from: 0, insert: 'let x 1' } });
    rig.host.step(16);
    expect(perf!.counters().gpuStatus.kind).not.toBe('unavailable');
    expect(perf!.counters().renderer.textBuilds).toBeGreaterThan(0);
    rig.deps.store.apply({ kind: 'tempo', body: { bpm: 120, beats_per_cycle: 4, cycle: [0, 1], transport: {
      epoch: 'e1', sample_time: 1, cycle: [2, 1], bpm: 120, beats_per_cycle: 4, running: true,
      latency_seconds: 0, latency_kind: 'measured', uncertainty_seconds: 0,
    } } });
    rig.transport.emit({ kind: 'playing', body: { events: [{ slot: 'd1', beat: [0, 1], time: 1, end_time: 2,
      dur: [1, 1], src: { file: 'main.vact', span: { start: 0, end: 3 }, doc_revision: rig.deps.code!.currentRevision('main.vact'), form_gen: 1 } }] } });
    expect(rig.host.callbacks.size).toBeGreaterThan(0);
    const builds = perf!.counters().renderer.textBuilds;
    rig.host.step(32);
    expect(rig.host.callbacks.size).toBeGreaterThan(0);
    rig.host.step(48);
    expect(rig.host.callbacks.size).toBeGreaterThan(0);
    expect(perf?.counters().renderer.textBuilds).toBe(builds);
    expect(perf?.perf.snapshot().frames.length).toBeGreaterThan(0);
    const sample = audible.sample(48);
    expect(perf?.presented().at(-1)).toMatchObject({ activeKey: '0-3', audibleTime: sample.time, targetMs: sample.targetMs, beatCycle: 2.025 });
    expect(perf?.transportSample()).toMatchObject({ epoch: 'e1', cycle: [2, 1], sample_time: 1 });
    expect(perf?.onsets()).toHaveLength(1);
    expect(perf?.onsets()[0]).toMatchObject({ from: 0, to: 3, time: 1, end: 2, receivedMs: expect.any(Number) });
  });

  it('keeps syntax annotations stable across playing frames and recomputes them after wheel scrolling', () => {
    const renderSpy = vi.spyOn(CanvasRenderer.prototype, 'render');
    const rig = setup(true, true); const perf = (window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf!;
    const surface = rig.deps.code!.surface;
    surface.dispatch({ changes: { from: 0, insert: 'let x 1\n'.repeat(80) } });
    rig.transport.emit({ kind: 'playing', body: { events: [{ slot: 'd1', beat: [0, 1], time: 0, end_time: 1,
      dur: [1, 1], src: { file: 'main.vact', span: { start: 0, end: 3 }, doc_revision: rig.deps.code!.currentRevision('main.vact'), form_gen: 1 } }] } });
    rig.host.step(16);
    expect(perf.counters().gpuStatus.kind).toBe('ready');
    expect(perf.counters().renderer.textBuilds).toBeGreaterThan(0);
    rig.host.step(32); rig.host.step(48);
    const syntaxByFrame = renderSpy.mock.calls.map(([feedback]) => feedback?.annotations?.filter((a) => a.kind === 'syntax'));
    expect(syntaxByFrame.length).toBeGreaterThanOrEqual(3);
    expect(syntaxByFrame[0]).toEqual(expect.arrayContaining([expect.objectContaining({ className: 'vact-tok-head' }), expect.objectContaining({ className: 'vact-tok-number' })]));
    expect(syntaxByFrame[1]).toEqual(syntaxByFrame[0]);
    expect(syntaxByFrame[2]).toEqual(syntaxByFrame[0]);
    const builds = perf.counters().renderer.textBuilds;
    expect(perf.perf.snapshot().frames.slice(-2).every((row) => row[2] === 0)).toBe(true);
    expect(perf.counters().renderer.textBuilds).toBe(builds);

    const canvas = rig.code.querySelector('canvas.vact-code-canvas')!;
    const positionBeforeScroll = surface.posAtCoords({ x: 55, y: 5 });
    canvas.dispatchEvent(new WheelEvent('wheel', { deltaY: 24, bubbles: true, cancelable: true }));
    rig.host.step(64);
    expect(perf.perf.snapshot().frames.at(-1)?.[2]).toBe(1);
    expect(surface.posAtCoords({ x: 55, y: 5 })).not.toBe(positionBeforeScroll);
    const scrollSyntax = renderSpy.mock.calls.at(-1)?.[0]?.annotations?.filter((a) => a.kind === 'syntax');
    expect(scrollSyntax).toEqual(expect.arrayContaining([expect.objectContaining({ className: 'vact-tok-head' }), expect.objectContaining({ className: 'vact-tok-number' })]));
  });

  it('shows diagnostic messages in a positioned tooltip and hides it on leave, outside, scroll and document change', () => {
    vi.useFakeTimers();
    const rig = setup(true, true); const surface = rig.deps.code!.surface;
    surface.dispatch({ changes: { from: 0, insert: 'x' } });
    rig.transport.emit({ kind: 'eval-result', body: { file: 'main.vact', doc_revision: rig.deps.code!.currentRevision('main.vact'),
      forms: [], sites: [], diagnostics: [{ code: 'E_TEST', severity: 'error', message: 'expected value', span: { start: 0, end: 1 }, file: 'main.vact' }],
      directives: { file_level: {}, entries: [] } } });
    rig.host.step(16);
    const posAtCoords = vi.spyOn(surface, 'posAtCoords').mockReturnValue(0);
    vi.spyOn(surface, 'coordsAtPos').mockReturnValue({ left: 70, right: 78, top: 20, bottom: 38 });
    const canvas = rig.code.querySelector('canvas.vact-code-canvas')!;
    const move = (x = 72, y = 24): void => { canvas.dispatchEvent(new MouseEvent('pointermove', { clientX: x, clientY: y, bubbles: true })); };
    const tip = rig.code.querySelector<HTMLElement>('[role="tooltip"]')!;
    move();
    expect(tip.hidden).toBe(false);
    expect(tip.textContent).toBe('expected value');
    expect(tip.style.left).toBe('70px');
    expect(tip.style.top).toBe('38px');
    canvas.dispatchEvent(new MouseEvent('pointerleave'));
    expect(tip.hidden).toBe(true);
    move(); posAtCoords.mockReturnValue(null); move(900, 900);
    expect(tip.hidden).toBe(true);
    posAtCoords.mockReturnValue(0); move();
    canvas.dispatchEvent(new WheelEvent('wheel', { deltaY: 12, bubbles: true, cancelable: true }));
    expect(tip.hidden).toBe(true);
    move(); surface.dispatch({ changes: { from: 1, insert: 'y' } });
    expect(tip.hidden).toBe(true);
  });

  it('records bridge keys and exposes the current document and selection through the perf hook', () => {
    const rig = setup(true); const perf = (window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf!;
    const textarea = rig.code.querySelector('textarea[aria-label="Code editor"]') as HTMLTextAreaElement;
    textarea.dispatchEvent(new InputEvent('beforeinput', { inputType: 'insertText', data: 'ab', bubbles: true, cancelable: true }));
    const before = perf.perf.snapshot().keys.length;
    textarea.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', ctrlKey: true, bubbles: true, cancelable: true }));
    expect(perf.perf.snapshot().keys.length).toBe(before + 1);
    expect(perf.doc()).toBe('ab');
    const selection = rig.deps.code!.surface.state.selection.main;
    expect(perf.selection()).toEqual({ anchor: selection.anchor, head: selection.head });
    expect(perf.selection()).toEqual({ anchor: 0, head: 2 });
    expect(perf.revision()).toBe(rig.deps.code!.currentRevision('main.vact'));
    expect(perf.transportSample()).toBeNull();
  });

  it('evaluates on Mod-Enter and expires the flash after 200 ms without replay', () => {
    vi.useFakeTimers();
    const rig = setup(true, true);
    const surface = rig.deps.code!.surface;
    surface.dispatch({ changes: { from: 0, insert: 'let x 1' } });
    rig.host.step(16);
    const textarea = rig.code.querySelector('textarea[aria-label="Code editor"]') as HTMLTextAreaElement;
    const mac = /Mac|iP/.test(navigator.platform);
    expect(rig.host.callbacks.size).toBe(0);
    const key = new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: !mac, metaKey: mac, bubbles: true, cancelable: true });
    textarea.dispatchEvent(key);
    expect(key.defaultPrevented).toBe(true);
    expect(rig.transport.kinds()).toContain('eval');
    expect(rig.host.callbacks.size).toBeGreaterThan(0);
    vi.advanceTimersByTime(199);
    rig.host.step(32);
    expect(rig.host.callbacks.size).toBeGreaterThan(0);
    vi.advanceTimersByTime(1);
    rig.host.step(300);
    expect(rig.host.callbacks.size).toBe(0);
  });

  it('matches macOS Shift-Alt-f through KeyF and refuses format and eval during composition', async () => {
    const rig = setup();
    const formatter = { format: vi.fn(async () => ({ status: 0, text: 'formatted' })) };
    rig.deps.formatter = formatter;
    const textarea = rig.code.querySelector('textarea[aria-label="Code editor"]') as HTMLTextAreaElement;
    const formatKey = () => textarea.dispatchEvent(new KeyboardEvent('keydown', {
      key: '\u00cf', code: 'KeyF', shiftKey: true, altKey: true, bubbles: true, cancelable: true,
    }));
    formatKey();
    expect(formatter.format).toHaveBeenCalledTimes(1);
    await Promise.resolve();
    textarea.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
    formatKey();
    const mac = /Mac|iP/.test(navigator.platform);
    textarea.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: !mac, metaKey: mac, bubbles: true, cancelable: true }));
    expect(formatter.format).toHaveBeenCalledTimes(1);
    expect(rig.transport.kinds()).not.toContain('eval');
  });

  it('removes mount listeners and clears the dependency and resource ledger on dispose', () => {
    const add = vi.spyOn(EventTarget.prototype, 'addEventListener');
    const remove = vi.spyOn(EventTarget.prototype, 'removeEventListener');
    const rig = setup(true, true);
    const textarea = rig.code.querySelector('textarea[aria-label="Code editor"]') as HTMLTextAreaElement;
    const canvas = rig.code.querySelector('canvas.vact-code-canvas')!;
    const targets = new Set<EventTarget>([textarea, canvas, window, document]);
    const counts = (spy: typeof add) => spy.mock.contexts.flatMap((target, index) => {
      if (!targets.has(target as EventTarget)) return [];
      const [type, listener] = spy.mock.calls[index]!;
      return [`${targets.has(target as EventTarget) ? (target === textarea ? 'textarea' : target === canvas ? 'canvas' : target === window ? 'window' : 'document') : ''}:${type}:${String(listener)}`];
    }).sort();
    const added = counts(add);
    const perf = (window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf!;
    rig.mounted.dispose();
    expect(counts(remove)).toEqual(added);
    expect(perf.ledger.usedBytes).toBe(0);
    expect(rig.deps.code).toBeUndefined();
    add.mockRestore(); remove.mockRestore();
  });

  it('marks future playing telemetry as dropped in the status element', () => {
    const audible = new AudibleClock({ at: () => ({ time: 1, uncertainty: 0, provenance: 'measured' }) });
    const rig = setup(true, false, audible); const perf = (window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf!;
    rig.deps.code!.surface.dispatch({ changes: { from: 0, insert: 'let x 1' } });
    const revision = rig.deps.code!.currentRevision('main.vact');
    rig.transport.emit({ kind: 'playing', body: { events: [{ slot: 'd1', beat: [0, 1], time: 4, end_time: 5,
      dur: [1, 1], src: { file: 'main.vact', span: { start: 0, end: 3 }, doc_revision: revision, form_gen: 1 } }] } });
    expect(perf.counters().highlight.horizonDrops).toBe(1);
    rig.deps.code!.surface.dispatch({ changes: { from: 7, insert: ' ' } });
    rig.host.step(16);
    expect(rig.code.querySelector<HTMLElement>('.vact-code-gpu-status')?.dataset.telemetryDropped).toBe('1');
  });

  it('sends a one-character edit in a 1 MiB document as an incremental payload', () => {
    vi.useFakeTimers();
    const rig = setup(); const surface = rig.deps.code!.surface;
    const large = 'x'.repeat(1024 * 1024);
    surface.dispatch({ changes: { from: 0, insert: large } });
    vi.advanceTimersByTime(200);
    rig.transport.clear();
    surface.dispatch({ changes: { from: large.length, insert: '!' } });
    vi.advanceTimersByTime(200);
    const payloads = rig.transport.envelopes;
    const body = payloads.find((env) => env.kind === 'doc-changed')?.body;
    expect(body).toBeDefined();
    expect(JSON.stringify(body).length).toBeLessThan(256);
    expect(payloads.every((env) => !('code' in env) && !('text' in env))).toBe(true);
  });

  it('disposes through the retained perf hook', () => {
    const rig = setup(true, true); const perf = (window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf!;
    rig.deps.code!.surface.dispatch({ changes: { from: 0, insert: 'text' } });
    perf.disposeCode();
    expect(perf.ledger.usedBytes).toBe(0);
    expect((window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf).toBeUndefined();
    expect(rig.deps.code).toBeUndefined();
  });
});
