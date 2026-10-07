// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import type { EditorDeps } from '../../src/app/deps';
import { buildLayout } from '../../src/app/layout';
import { mount } from '../../src/code/mount';
import type { VactrPerf } from '../../src/code/perf-hook';
import { MemoryFiles } from '../../src/platform/files';
import { Client } from '../../src/protocol/client';
import { Store } from '../../src/protocol/store';
import { RecordingTransport } from '../support/recording';
import { installCanvasFakes, type CanvasFakes } from '../support/canvas';
import { MockClock } from '../support/clock';

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
  return {
    callbacks,
    devicePixelRatio: 1,
    innerHeight: 800,
    requestAnimationFrame(cb) { const id = next++; callbacks.set(id, cb); return id; },
    cancelAnimationFrame(id) { callbacks.delete(id); },
    step(time) {
      const row = callbacks.entries().next().value as [number, FrameRequestCallback] | undefined;
      if (!row) return;
      callbacks.delete(row[0]); row[1](time);
    },
  };
}

interface Rig {
  root: HTMLElement;
  code: HTMLElement;
  hostEl: HTMLElement;
  deps: EditorDeps;
  mounted: ReturnType<typeof mount>;
  host: RafHost;
  fakes: CanvasFakes;
}

const rigs: Rig[] = [];

function setup(search = '/', renderer?: 'canvas' | 'dom'): Rig {
  window.history.replaceState({}, '', search);
  const fakes = installCanvasFakes();
  const context = fakes.ctx(document.createElement('canvas')) as unknown as Record<string, unknown>;
  Object.assign(Object.getPrototypeOf(context) as object, {
    measureText: (text: string) => ({ width: text.length * 8 }),
    setTransform() {}, save() {}, restore() {}, rect() {}, clip() {},
  });
  const root = document.createElement('div'); document.body.append(root);
  const layout = buildLayout(root); const store = new Store(); const transport = new RecordingTransport();
  const deps: EditorDeps = {
    client: new Client(transport, { store }), store, clock: new MockClock(), tier: 'browser', files: new MemoryFiles(),
  };
  const host = rafHost();
  const mounted = mount(root, deps, { frameHost: host, ...(renderer ? { renderer } : {}) });
  const hostEl = layout.code.querySelector<HTMLElement>('.vact-code')!;
  const rig = { root, code: layout.code, hostEl, deps, mounted, host, fakes };
  rigs.push(rig); return rig;
}

function pointer(element: HTMLElement, type: string, props: Partial<PointerEvent> = {}): void {
  const event = new Event(type, { bubbles: true, cancelable: true });
  for (const [name, value] of Object.entries({ pointerId: 1, pointerType: 'mouse', button: 0, isPrimary: true,
    clientX: 72, clientY: 9, detail: 1, ...props })) Object.defineProperty(event, name, { value });
  element.dispatchEvent(event);
}

afterEach(() => {
  for (const rig of rigs.splice(0)) { rig.mounted.dispose(); rig.deps.client.close(); rig.deps.store.dispose(); rig.root.remove(); rig.fakes.restore(); }
  window.history.replaceState({}, '', '/'); vi.restoreAllMocks();
});

describe('DOM renderer mount seam', () => {
  it('keeps canvas as the default and falls back for an unknown URL renderer', () => {
    const defaultRig = setup();
    expect(defaultRig.code.dataset.renderer).toBe('canvas');
    expect(defaultRig.code.querySelector('canvas.vact-code-canvas')).not.toBeNull();
    expect(defaultRig.code.querySelector('.vact-code-dom')).toBeNull();
    const unknownRig = setup('/?renderer=bogus');
    expect(unknownRig.code.dataset.renderer).toBe('canvas');
    expect(unknownRig.code.querySelector('canvas.vact-code-canvas')).not.toBeNull();
    expect(unknownRig.code.querySelector('.vact-code-dom')).toBeNull();
  });

  it('selects DOM from the URL and gives an explicit option precedence', () => {
    const urlRig = setup('/?renderer=dom');
    expect(urlRig.code.dataset.renderer).toBe('dom');
    expect(urlRig.code.querySelector('.vact-code-dom')).not.toBeNull();
    expect(urlRig.code.querySelector('canvas.vact-code-canvas')).toBeNull();
    const optionRig = setup('/', 'dom');
    expect(optionRig.code.dataset.renderer).toBe('dom');
    expect(optionRig.code.querySelector('.vact-code-dom')).not.toBeNull();
    const overrideRig = setup('/?renderer=dom', 'canvas');
    expect(overrideRig.code.dataset.renderer).toBe('canvas');
    expect(overrideRig.code.querySelector('canvas.vact-code-canvas')).not.toBeNull();
  });

  it('renders literal document text in the DOM surface', () => {
    const rig = setup('/?renderer=dom');
    rig.deps.code!.surface.dispatch({ changes: { from: 0, insert: 'let a 1' } });
    rig.host.step(16);
    expect(rig.code.querySelector('.vact-dom-line')?.textContent).toContain('let a 1');
  });

  it('uses the DOM surface as the pointer target and selects through shared layout geometry', () => {
    const rig = setup('/?renderer=dom');
    rig.deps.code!.surface.dispatch({ changes: { from: 0, insert: 'let abc 1' } });
    rig.hostEl.getBoundingClientRect = () => ({ left: 0, top: 0, right: 400, bottom: 200, x: 0, y: 0,
      width: 400, height: 200, toJSON: () => ({}) });
    rig.host.step(16);
    const surfaceEl = rig.code.querySelector<HTMLElement>('.vact-code-dom')!;
    pointer(surfaceEl, 'pointerdown'); pointer(surfaceEl, 'pointerup');
    expect(rig.deps.code!.surface.state.selection.main.head).toBe(3);
  });

  it('reports the selected renderer, DOM node count, and ready DOM status through perf counters', () => {
    const domRig = setup('/?perf=1&renderer=dom');
    const domPerf = (window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf!;
    const domCounters = domPerf.counters();
    expect(domCounters.rendererKind).toBe('dom');
    expect(domCounters.domNodes).toBeGreaterThan(0);
    expect(domCounters.gpuStatus.kind).toBe('ready');
    expect(domCounters.textPending).toBe(false);
    const canvasRig = setup('/?perf=1');
    const canvasCounters = (window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf!.counters();
    expect(canvasCounters.rendererKind).toBe('canvas');
    expect(canvasCounters.domNodes).toBeGreaterThan(0);
    expect(domRig.code.dataset.renderer).toBe('dom');
    expect(canvasRig.code.dataset.renderer).toBe('canvas');
  });

  it('does at most one layout rect read per edited frame', () => {
    const rig = setup('/?renderer=dom');
    rig.host.step(16);
    const readRect = vi.spyOn(Element.prototype, 'getBoundingClientRect');
    const textarea = rig.code.querySelector<HTMLTextAreaElement>('textarea[aria-label="Code editor"]')!;
    for (let frame = 0; frame < 5; frame++) {
      textarea.dispatchEvent(new InputEvent('beforeinput', { inputType: 'insertText', data: 'x', bubbles: true, cancelable: true }));
      rig.host.step(32 + frame * 16);
      expect(readRect.mock.calls.length).toBeLessThanOrEqual(frame + 1);
    }
    expect(readRect).toHaveBeenCalledTimes(5);
  });

  it('places the visual backdrop immediately before the DOM surface', () => {
    const rig = setup('/?renderer=dom'); const backdrop = document.createElement('canvas');
    rig.deps.visual = {
      onBackgroundCanvas(callback) { callback(backdrop); return () => {}; },
      mountSpectrum() { return { dispose() {} }; },
    };
    rig.host.step(16);
    expect(backdrop.parentElement).toBe(rig.hostEl);
    expect(backdrop.nextElementSibling).toBe(rig.code.querySelector('.vact-code-dom'));
  });

  it('disposes DOM nodes, perf hook, and renderer resources', () => {
    const rig = setup('/?perf=1&renderer=dom');
    const perf = (window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf!;
    rig.mounted.dispose();
    expect(perf.ledger.usedBytes).toBe(0);
    expect(rig.code.querySelector('.vact-code-dom')).toBeNull();
    expect((window as Window & { __vactrPerf?: VactrPerf }).__vactrPerf).toBeUndefined();
  });
});
