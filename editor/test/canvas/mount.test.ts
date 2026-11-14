// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import { Text } from '@codemirror/state';
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
import { CodeViewHost } from '../../src/code/view-host';
import { TextLayout } from '../../src/code/layout';
import { DocumentSync } from '../../src/code/sync';
import { FallbackSpans } from '../../src/code/syntax';
import { startFrameLoop } from '../../src/visual/frame';
import { WasmCore } from '../../src/protocol/wasm';
import { VactrHost } from '../../worklet/host.js';
import { FakeCore, fakeNode } from '../support/fake-core';

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

type RenderCalls = { getError: number; getParameter: number; texSubImage2D: number; bufferData: number; bufferSubData: number; drawArraysInstanced: number };
function rendererGL(calls?: RenderCalls): WebGL2RenderingContext {
  let next = 1;
  const gl: Record<string, unknown> = {};
  const constants = ['MAX_TEXTURE_SIZE', 'TEXTURE_2D', 'TEXTURE_MIN_FILTER', 'TEXTURE_MAG_FILTER', 'LINEAR', 'NEAREST',
    'TEXTURE_WRAP_S', 'TEXTURE_WRAP_T', 'CLAMP_TO_EDGE', 'UNPACK_PREMULTIPLY_ALPHA_WEBGL', 'UNPACK_SKIP_PIXELS', 'UNPACK_SKIP_ROWS', 'UNPACK_ROW_LENGTH', 'RGBA', 'RG', 'RG32F', 'UNSIGNED_SHORT', 'UNSIGNED_BYTE',
    'VERTEX_SHADER', 'FRAGMENT_SHADER', 'COMPILE_STATUS', 'LINK_STATUS', 'ARRAY_BUFFER', 'STATIC_DRAW', 'DYNAMIC_DRAW', 'FLOAT',
    'FRAMEBUFFER', 'BLEND', 'ONE', 'ONE_MINUS_SRC_ALPHA', 'COLOR_BUFFER_BIT', 'TEXTURE0', 'TEXTURE1', 'TRIANGLES'];
  constants.forEach((name, index) => { gl[name] = index + 1; }); gl.NO_ERROR = 0;
  for (const kind of ['Texture', 'Shader', 'Program', 'Buffer', 'VertexArray']) {
    gl[`create${kind}`] = () => ({ id: next++ }); gl[`delete${kind}`] = () => {};
  }
  for (const name of ['texParameteri', 'pixelStorei', 'shaderSource', 'compileShader', 'attachShader', 'linkProgram',
    'bindVertexArray', 'bindBuffer', 'bufferData', 'bufferSubData', 'enableVertexAttribArray', 'vertexAttribPointer', 'vertexAttribIPointer', 'vertexAttribDivisor', 'bindFramebuffer',
    'useProgram', 'viewport', 'disable', 'enable', 'blendFunc', 'clearColor', 'clear', 'uniform2f', 'uniform1i',
    'activeTexture', 'drawArraysInstanced', 'bindTexture', 'texImage2D', 'texSubImage2D', 'uniform4f', 'uniform1f']) gl[name] = () => {
    if (calls && name in calls) calls[name as keyof RenderCalls]++;
  };
  gl.getParameter = () => { if (calls) calls.getParameter++; return 4096; }; gl.getShaderParameter = () => true; gl.getProgramParameter = () => true;
  gl.getShaderInfoLog = () => ''; gl.getProgramInfoLog = () => ''; gl.getAttribLocation = () => 0;
  gl.getUniformLocation = (_program: unknown, name: string) => ({ name }); gl.getError = () => { if (calls) calls.getError++; return gl.NO_ERROR; };
  gl.isContextLost = () => false; gl.isTexture = () => true;
  return gl as unknown as WebGL2RenderingContext;
}

interface Rig { root: HTMLElement; code: HTMLElement; deps: EditorDeps; mounted: ReturnType<typeof mount>; transport: RecordingTransport; fakes: CanvasFakes; host: RafHost; gl?: WebGL2RenderingContext; calls?: RenderCalls; measureTextCalls: { value: number } }
const rigs: Rig[] = [];
function setup(perf = false, withGl = false, audible?: AudibleClock, core?: WasmCore, measureTextCalls = { value: 0 }, calls: RenderCalls = { getError: 0, getParameter: 0, texSubImage2D: 0, bufferData: 0, bufferSubData: 0, drawArraysInstanced: 0 }): Rig {
  const gl = withGl ? rendererGL(calls) : undefined;
  const fakes = installCanvasFakes(withGl ? { webgl2: () => gl! } : {});
  const context = fakes.ctx(document.createElement('canvas')) as unknown as Record<string, unknown>;
  Object.assign(Object.getPrototypeOf(context) as object, {
    measureText: (text: string) => { measureTextCalls.value++; return { width: text.length * 8 }; },
    setTransform() {}, save() {}, restore() {}, rect() {}, clip() {},
  });
  const root = document.createElement('div'); document.body.append(root);
  const layout = buildLayout(root); const store = new Store(); const transport = new RecordingTransport();
  const deps: EditorDeps = { client: new Client(transport, { store }), store, clock: new MockClock(), tier: 'browser', files: new MemoryFiles(), ...(audible ? { audible } : {}), ...(core ? { core } : {}) };
  const host = rafHost();
  if (perf) window.history.replaceState({}, '', '?perf=1');
  const mounted = mount(root, deps, { frameHost: host, ...(gl ? { gl } : {}) });
  const rig = { root, code: layout.code, deps, mounted, transport, fakes, host, measureTextCalls, ...(withGl ? { gl, calls } : {}) };
  rigs.push(rig); return rig;
}

afterEach(() => {
  for (const rig of rigs.splice(0)) { rig.mounted.dispose(); rig.deps.client.close(); rig.deps.store.dispose(); rig.root.remove(); rig.fakes.restore(); }
  window.history.replaceState({}, '', '/'); vi.useRealTimers(); document.body.replaceChildren();
});

describe('headless canvas mount', () => {
  it('invalidates the cached rect on text frames and resets width for one full-document replacement', () => {
    const rig = setup();
    const invalidate = vi.spyOn(CodeViewHost.prototype, 'invalidateRect');
    const refresh = vi.spyOn(CodeViewHost.prototype, 'refreshRect');
    const reset = vi.spyOn(TextLayout.prototype, 'resetWidestShaped');
    rig.host.step(16);
    invalidate.mockClear(); refresh.mockClear();
    const surface = rig.deps.code!.surface, oldLength = surface.state.doc.length;
    surface.dispatch({ changes: { from: 0, to: oldLength, insert: 'replacement document' } });
    expect(reset).toHaveBeenCalledOnce();
    rig.host.step(32);
    expect(invalidate).toHaveBeenCalledOnce();
    expect(refresh).toHaveBeenCalledOnce();
    refresh.mockClear();
    rig.transport.emit({ kind: 'playing', body: { events: [] } });
    rig.host.step(48);
    expect(invalidate).toHaveBeenCalledOnce();
    expect(refresh).not.toHaveBeenCalled();
  });

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

  it('stacks the visual canvas under code and never copies it during animation', () => {
    const rig = setup(true, true); const visualCanvas = document.createElement('canvas');
    let notify: ((canvas: HTMLCanvasElement | null) => void) | undefined;
    rig.deps.visual = {
      onBackgroundCanvas(callback) { notify = callback; callback(visualCanvas); return () => {}; },
      mountSpectrum() { return { dispose() {} }; },
    };
    const codeHost = rig.code.querySelector<HTMLElement>('.vact-code')!;
    const gl = rig.gl!;
    const textureUploads = vi.spyOn(gl, 'texImage2D'); const textureSubUploads = vi.spyOn(gl, 'texSubImage2D');
    const drawImageCount = (): number => rig.fakes.canvases2d.reduce((count, canvas) => count + rig.fakes.ctx(canvas).named('drawImage').length, 0);
    rig.host.step(16);
    const codeCanvas = rig.code.querySelector('canvas.vact-code-canvas')!;
    expect(codeHost.firstElementChild).toBe(visualCanvas);
    expect(visualCanvas.nextElementSibling).toBe(codeCanvas);
    expect(visualCanvas.classList.contains('vact-code-backdrop')).toBe(true);
    expect(visualCanvas.getAttribute('aria-hidden')).toBe('true');
    const images = drawImageCount(), imagesBefore = textureUploads.mock.calls.length, subsBefore = textureSubUploads.mock.calls.length;
    rig.transport.emit({ kind: 'playing', body: { events: [{ slot: 'd1', beat: [0, 1], time: 0, end_time: 100,
      dur: [1, 1], src: { file: 'main.vact', span: { start: 0, end: 3 }, doc_revision: rig.deps.code!.currentRevision('main.vact'), form_gen: 1 } }] } });
    for (let frame = 0; frame < 60; frame++) rig.host.step(32 + frame * 16);
    expect(drawImageCount()).toBe(images);
    expect(textureUploads).toHaveBeenCalledTimes(imagesBefore);
    expect(textureSubUploads).toHaveBeenCalledTimes(subsBefore);
    notify?.(null); expect(codeHost.contains(visualCanvas)).toBe(false);
    notify?.(visualCanvas); expect(codeHost.firstElementChild).toBe(visualCanvas);
    rig.mounted.dispose(); expect(codeHost.contains(visualCanvas)).toBe(false);
  });

  it('installs no performance hook without the query flag', () => {
    const install = vi.spyOn(PerfHook, 'installPerfHook');
    const rig = setup();
    expect((window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf).toBeUndefined();
    expect((globalThis as typeof globalThis & { __vactrPhaseTimer?: unknown }).__vactrPhaseTimer).toBeUndefined();
    expect(install).not.toHaveBeenCalled();
    rig.mounted.dispose();
  });

  it('keeps active playing frames separate from text work and records the audible presentation contract', () => {
    const audible = new AudibleClock({ at: () => ({ time: 1.05, uncertainty: 0.002, provenance: 'measured' }) });
    const rig = setup(true, true, audible); const perf = (window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf;
    expect(perf).toBeDefined();
    expect((globalThis as typeof globalThis & { __vactrPhaseTimer?: unknown }).__vactrPhaseTimer).toBeDefined();
    const textarea = rig.code.querySelector('textarea')!;
    textarea.dispatchEvent(new InputEvent('beforeinput', { inputType: 'insertText', data: 'a', bubbles: true, cancelable: true }));
    const surface = rig.deps.code!.surface;
    surface.dispatch({ changes: { from: 0, insert: 'let x 1' } });
    rig.host.step(16);
    const phaseRows = perf!.phases().rows;
    expect(perf!.phases().names).toEqual(['input', 'caret', 'shaping', 'syntax', 'upload', 'frame', 'tick']);
    expect(phaseRows.some((row) => row[0] === perf!.phases().names.indexOf('input'))).toBe(true);
    expect(phaseRows.some((row) => row[0] === perf!.phases().names.indexOf('frame'))).toBe(true);
    expect(phaseRows.every((row) => row.length === 9)).toBe(true);
    expect(phaseRows.some((row) => row[perf!.phases().names.indexOf('shaping') + 2]! > 0)).toBe(true);
    expect(phaseRows.some((row) => row[perf!.phases().names.indexOf('upload') + 2]! > 0)).toBe(true);
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
    expect(perf?.presented().at(-1)).toMatchObject({ activeKey: '0-3', audibleTime: sample.time, targetMs: sample.targetMs, beatCycle: 2.025, epoch: 'e1' });
    expect(perf?.transportSample()).toMatchObject({ epoch: 'e1', cycle: [2, 1], sample_time: 1 });
    expect(perf?.onsets()).toHaveLength(1);
    expect(perf?.onsets()[0]).toMatchObject({ from: 0, to: 3, time: 1, end: 2, receivedMs: expect.any(Number) });
  });

  it('does no whole-document work on an edit frame or active animation-only frames', () => {
    const calls: RenderCalls = { getError: 0, getParameter: 0, texSubImage2D: 0, bufferData: 0, bufferSubData: 0, drawArraysInstanced: 0 };
    const measureTextCalls = { value: 0 };
    const rig = setup(true, true, undefined, undefined, measureTextCalls, calls); const perf = (window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf!;
    const surface = rig.deps.code!.surface;
    const longLine = `const ${'x'.repeat(1_530)} 日本 👨‍👩‍👧‍👦`;
    const documentLines = Array.from({ length: 20_000 }, (_, n) => `const value${n} = alpha beta gamma${' '.repeat(22)}`);
    documentLines[0] = longLine;
    const largeText = documentLines.join('\n');
    surface.dispatch({ changes: { from: 0, insert: largeText } }); rig.host.step(16);
    rig.transport.emit({ kind: 'playing', body: { events: [{ slot: 'd1', beat: [0, 1], time: 0, end_time: 100,
      dur: [1, 1], src: { file: 'main.vact', span: { start: 0, end: longLine.length }, doc_revision: rig.deps.code!.currentRevision('main.vact'), form_gen: 1 } }] } });
    const position = surface.state.doc.line(2).from + 12;
    surface.dispatch({ selection: { anchor: position } });
    const textarea = rig.code.querySelector('textarea[aria-label="Code editor"]') as HTMLTextAreaElement;
    rig.host.step(20);
    const layoutStats = perf.counters().layout;
    const segment = vi.spyOn(Intl.Segmenter.prototype, 'segment');
    const animationCalls = layoutStats.measuredTextCalls, animationChars = layoutStats.measuredTextChars;
    const animationSegments = layoutStats.segmentations;
    for (let frame = 0; frame < 3; frame++) rig.host.step(24 + frame * 16);
    expect(layoutStats.measuredTextCalls).toBe(animationCalls);
    expect(layoutStats.measuredTextChars).toBe(animationChars);
    expect(layoutStats.segmentations).toBe(animationSegments);
    expect(segment).not.toHaveBeenCalled();
    const measuredBeforeEdit = layoutStats.measuredTextChars;
    const rendererBeforeEdit = { ...perf.counters().renderer };
    const uploadBeforeEdit = calls.texSubImage2D, measureCallsBeforeEdit = measureTextCalls.value;
    const glQueriesBeforeEdit = { getError: calls.getError, getParameter: calls.getParameter };
    const toString = vi.spyOn(Text.prototype, 'toString');
    const sliceString = vi.spyOn(Object.getPrototypeOf(surface.state.doc) as Text & { sliceString: Text['sliceString'] }, 'sliceString');
    textarea.dispatchEvent(new InputEvent('beforeinput', { inputType: 'insertText', data: 'x', bubbles: true, cancelable: true }));
    rig.host.step(72);
    const rendererAfterEdit = perf.counters().renderer;
    expect(rendererAfterEdit.textBuilds - rendererBeforeEdit.textBuilds,
      JSON.stringify({ before: rendererBeforeEdit, after: rendererAfterEdit, revision: surface.state.doc.length, view: rig.deps.code!.surface.state.selection.main.head })).toBe(1);
    expect(rendererAfterEdit.geometryBytes - rendererBeforeEdit.geometryBytes,
      JSON.stringify({ delta: rendererAfterEdit.geometryBytes - rendererBeforeEdit.geometryBytes,
        lineLength: surface.state.doc.line(2).length, clusters: Array.from(surface.state.doc.line(2).text).length,
        builds: rendererAfterEdit.textBuilds - rendererBeforeEdit.textBuilds })).toBeLessThanOrEqual(2048);
    expect(rendererAfterEdit.lastFrameDraws).toBeLessThanOrEqual(4);
    expect(calls.getError - glQueriesBeforeEdit.getError).toBe(0);
    expect(calls.getParameter - glQueriesBeforeEdit.getParameter).toBe(0);
    expect(calls.texSubImage2D - uploadBeforeEdit).toBeLessThanOrEqual(1);
    expect(measureTextCalls.value - measureCallsBeforeEdit).toBe(0);
    expect(layoutStats.measuredTextChars - measuredBeforeEdit).toBeLessThanOrEqual(4 * documentLines[1]!.length + 4_096);
    const editHadLargeSlice = sliceString.mock.calls.some(([from, to]) => (to ?? Infinity) - from > 64 * 1024);
    const builds = perf.counters().renderer.textBuilds;
    toString.mockClear(); sliceString.mockClear();
    for (let frame = 0; frame < 10; frame++) rig.host.step(48 + frame * 16);
    expect(toString).not.toHaveBeenCalled();
    expect(sliceString.mock.calls.some(([from, to]) => (to ?? Infinity) - from > 64 * 1024)).toBe(false);
    expect(perf.counters().renderer.textBuilds).toBe(builds);
    expect(editHadLargeSlice).toBe(false);
    const japaneseMeasureCount = measureTextCalls.value, japaneseUploads = calls.texSubImage2D;
    textarea.dispatchEvent(new InputEvent('beforeinput', { inputType: 'insertText', data: '漢', bubbles: true, cancelable: true }));
    rig.host.step(224);
    expect(measureTextCalls.value > japaneseMeasureCount || calls.texSubImage2D > japaneseUploads).toBe(true);
    rig.mounted.dispose();
    const controlRig = setup(true, true); const controlPerf = (window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf!;
    controlRig.deps.code!.surface.dispatch({ changes: { from: 0, insert: 'const fresh = 1' } }); controlRig.host.step(16);
    expect(controlPerf.counters().layout.measuredTextCalls).toBeGreaterThan(0);
    segment.mockRestore();
  });

  it('keeps each animation-only ABI frame bounded and free of layout and GL probes', () => {
    vi.useFakeTimers();
    const fake = new FakeCore(); const core = new WasmCore();
    core.attach(new VactrHost(null, fakeNode(), fake.exports, { wasmUrl: '', processorUrl: '', init: 'session', onRecord: core.onRecord }));
    const rig = setup(true, true, undefined, core); const perf = (window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf!;
    const surface = rig.deps.code!.surface, renderer = perf.counters().renderer, gl = rig.gl!;
    const isTexture = vi.spyOn(gl, 'isTexture'), getError = vi.spyOn(gl, 'getError');
    const segment = vi.spyOn(Intl.Segmenter.prototype, 'segment');
    const largeText = `const 日本 = "👨‍👩‍👧‍👦";\n${Array.from({ length: 20_000 }, (_, n) => `const value${n} = alpha beta gamma${' '.repeat(22)}`).join('\n')}`;
    surface.dispatch({ changes: { from: 0, insert: largeText } }); rig.host.step(16);
    expect(getError).toHaveBeenCalled(); expect(segment).toHaveBeenCalled();
    rig.transport.emit({ kind: 'playing', body: { events: [{ slot: 'd1', beat: [0, 1], time: 0, end_time: 100,
      dur: [1, 1], src: { file: 'main.vact', span: { start: 0, end: 5 }, doc_revision: rig.deps.code!.currentRevision('main.vact'), form_gen: 1 } }] } });
    rig.host.step(32);
    gl.isTexture({} as WebGLTexture); gl.getError();
    expect(isTexture).toHaveBeenCalledTimes(1); expect(getError).toHaveBeenCalled();
    isTexture.mockClear(); getError.mockClear(); segment.mockClear();
    fake.calls.length = 0;
    const loop = startFrameLoop({ core, host: { draw() {} }, clock: rig.deps.clock,
      scheduler: { request: cb => rig.host.requestAnimationFrame(cb), cancel: id => rig.host.cancelAnimationFrame(id) } });
    const runRegistered = (time: number): void => {
      const registered = [...rig.host.callbacks.entries()];
      for (const [id, callback] of registered) if (rig.host.callbacks.delete(id)) callback(time);
    };
    const builds = renderer.textBuilds;
    for (let frame = 0; frame < 10; frame++) runRegistered(48 + frame * 16);
    loop.dispose();
    expect(fake.callsOf('session_frame')).toHaveLength(10);
    expect(fake.callsOf('session_check')).toHaveLength(0);
    expect(fake.calls.every(call => !call.text || new TextEncoder().encode(call.text).byteLength <= 64 * 1024)).toBe(true);
    expect(fake.calls.every(call => !call.bytes || call.bytes.byteLength <= 64 * 1024)).toBe(true);
    expect(fake.calls.every(call => call.args.every(arg => typeof arg !== 'number' || arg <= 64 * 1024))).toBe(true);
    expect(isTexture).not.toHaveBeenCalled(); expect(getError).not.toHaveBeenCalled(); expect(segment).not.toHaveBeenCalled();
    expect(renderer.textBuilds).toBe(builds);

    const map = vi.spyOn(DocumentSync.prototype, 'mapWireSpan');
    const now = rig.deps.clock.now();
    rig.transport.emit({ kind: 'playing', body: { events: [{ slot: 'overlay-budget', beat: [0, 1], time: now,
      end_time: now + 100, dur: [1, 1], src: { file: 'main.vact', span: { start: 0, end: 5 },
        doc_revision: rig.deps.code!.currentRevision('main.vact'), form_gen: 1 } }] } });
    rig.host.step(240);
    expect(map).toHaveBeenCalled();
    map.mockClear();
    const calls = rig.calls!;
    calls.getError = 0; calls.getParameter = 0; calls.texSubImage2D = 0;
    const measureStart = rig.measureTextCalls.value;
    const canvasesStart = rig.fakes.canvases2d.length;
    const drawImages = () => rig.fakes.canvases2d.reduce((sum, canvas) => sum + rig.fakes.ctx(canvas).named('drawImage').length, 0);
    const copiesStart = drawImages();
    const textBuilds = renderer.textBuilds, layoutBuilds = perf.counters().layout.builds;
    const frameDraws: number[] = [];
    for (let frame = 0; frame < 100; frame++) {
      rig.host.step(256 + frame * 16);
      frameDraws.push(renderer.lastFrameDraws);
    }
    expect(renderer.textBuilds).toBe(textBuilds);
    expect(perf.counters().layout.builds).toBe(layoutBuilds);
    expect(calls.texSubImage2D).toBe(0);
    expect(rig.fakes.canvases2d.length - canvasesStart).toBe(0);
    expect(drawImages() - copiesStart).toBe(0);
    expect(perf.counters().ledger.byKind.backdrop).toBe(0);
    expect(calls.getParameter).toBe(0); expect(calls.getError).toBe(0);
    expect(rig.measureTextCalls.value - measureStart).toBe(0);
    expect(map).not.toHaveBeenCalled();
    expect(frameDraws).toHaveLength(100); expect(frameDraws.every(count => count <= 4)).toBe(true);
    surface.dispatch({ changes: { from: 0, insert: 'x' } }); rig.host.step(1900);
    expect(map).toHaveBeenCalled();
    map.mockRestore();
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

  it('does not request syntax spans for a caret-only frame and captures after scrolling', () => {
    const spans = vi.spyOn(FallbackSpans.prototype, 'spans');
    const rig = setup(true, true);
    const perf = (window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf!;
    const host = rig.code.firstElementChild as HTMLElement;
    host.getBoundingClientRect = () => ({ left: 0, right: 600, top: 0, bottom: 800, x: 0, y: 0, width: 600, height: 800, toJSON: () => ({}) });
    const surface = rig.deps.code!.surface;
    surface.dispatch({ changes: { from: 0, insert: 'let x 1\n'.repeat(200) } });
    rig.host.step(16);
    for (let frame = 0; frame < 5 && rig.host.callbacks.size; frame += 1) rig.host.step(24 + frame * 8);
    expect(spans).toHaveBeenCalled();
    spans.mockClear();
    const before = perf.counters();
    const render = vi.spyOn(CanvasRenderer.prototype, 'render');
    const head = surface.state.selection.main.head, line = surface.state.doc.lineAt(head);
    surface.dispatch({ selection: { anchor: head < line.to ? head + 1 : Math.max(line.from, head - 1) } });
    rig.host.step(32);
    expect(spans).not.toHaveBeenCalled();
    expect(perf.counters().renderer.textBuilds).toBe(before.renderer.textBuilds);
    expect(perf.counters().layout.builds).toBe(before.layout.builds);
    expect(render.mock.calls.at(-1)?.[0]?.cursor).not.toBe(head);
    const buildsBeforeTypedControl = perf.counters().renderer.textBuilds;
    surface.dispatch({ changes: { from: 0, insert: 'x' } }); rig.host.step(40);
    expect(perf.counters().renderer.textBuilds).toBeGreaterThan(buildsBeforeTypedControl);
    spans.mockClear();
    rig.code.querySelector('canvas.vact-code-canvas')!.dispatchEvent(new WheelEvent('wheel', { deltaY: 180, bubbles: true, cancelable: true }));
    rig.host.step(56);
    expect(spans).toHaveBeenCalled();
  });

  it('keeps the GPU status badge above both canvas layers', async () => {
    const fsName: string = 'node:fs';
    const fs = await import(/* @vite-ignore */ fsName) as { readFileSync(path: string, encoding: 'utf8'): string };
    const runtime = globalThis as unknown as { process: { cwd(): string } };
    const css = fs.readFileSync(`${runtime.process.cwd()}/src/code/code.css`, 'utf8');
    const rule = (selector: string): string => {
      const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      return css.match(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`))?.[1] ?? '';
    };
    const zIndex = (selector: string): number => Number(rule(selector).match(/z-index:\s*(\d+)/)?.[1]);
    expect(zIndex('.vact-code-gpu-status')).toBe(2);
    expect(zIndex('.vact-code-canvas')).toBe(1);
    expect(zIndex('.vact-code-backdrop')).toBe(0);
  });

  it('renders the state document after cancelling IME preedit and continues editing', () => {
    const setText = vi.spyOn(CanvasRenderer.prototype, 'setText');
    const rig = setup(false, true);
    const surface = rig.deps.code!.surface;
    const textarea = rig.code.querySelector('textarea[aria-label="Code editor"]') as HTMLTextAreaElement;
    surface.dispatch({ changes: { from: 0, insert: 'abc' } });
    surface.dispatch({ selection: { anchor: 3 } });
    rig.host.step(16);

    setText.mockClear();
    textarea.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
    textarea.dispatchEvent(new CompositionEvent('compositionupdate', { data: 'にほん', bubbles: true }));
    rig.host.step(32);
    expect(setText.mock.calls.at(-1)?.[0].toString()).toBe('abcにほん');

    setText.mockClear();
    textarea.dispatchEvent(new CompositionEvent('compositionend', { data: '', bubbles: true }));
    expect(rig.host.callbacks.size).toBeGreaterThan(0);
    rig.host.step(48);
    expect(setText.mock.calls.at(-1)?.[0].toString()).toBe('abc');
    expect(surface.state.doc.toString()).toBe('abc');

    setText.mockClear();
    const edit = new InputEvent('beforeinput', { inputType: 'insertText', data: 'x', bubbles: true, cancelable: true });
    expect(() => textarea.dispatchEvent(edit)).not.toThrow();
    expect(surface.state.doc.toString()).toBe('abcx');
    rig.host.step(64);
    expect(setText.mock.calls.at(-1)?.[0].toString()).toBe('abcx');
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
