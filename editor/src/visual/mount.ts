// The `visual` area (design 9.2-9.4, 15.1.3, 15.1.8). It builds the output
// panes o0..o3 in the `visual` pane and the analyzer displays in the
// `analyzers` pane, and sets `deps.visual` (`VisualApi`).
//
// Browser tier: the wasm core's `0x72` render records drive a WebGL2
// `GlRenderHost` on an offscreen canvas, and the frame loop runs
// `session_frame` then `draw` once per animation frame. A null
// `getContext('webgl2')` shows "WebGL2 not available". The native tier
// renders nothing (the session uses NoopRender) and says so. Meters and
// scopes render from `levels` on both tiers.

import type { VisualApi } from '../app/apis';
import type { EditorDeps, Mounted } from '../app/deps';
import { buildLayout } from '../app/layout';
import { startFrameLoop, windowScheduler, type FrameLoop, type FrameScheduler } from './frame';
import { AnalyzerArea } from './meters';
import { VisualPanes } from './panes';
import { GlRenderHost, type RenderSize } from './render-host';
import { mountSpectrum } from './spectrum';

export const RENDER_SIZE: RenderSize = { width: 640, height: 360 };
export const NATIVE_NOTICE = 'visuals: browser tier only';
export const NO_WEBGL2_NOTICE = 'WebGL2 not available';

// The pane's stylesheet as a Vite asset URL. A side-effect `import
// './visual.css'` fails `tsc` (TS2882) without a CSS module declaration,
// which no editor plan owns (the ED-MIDI precedent).
const STYLESHEET = new URL('./visual.css', import.meta.url).href;

function addStylesheet(doc: Document): HTMLLinkElement | null {
  if (doc.head.querySelector('link[data-style="visual"]')) return null;
  const link = doc.createElement('link');
  link.rel = 'stylesheet';
  link.href = STYLESHEET;
  link.dataset.style = 'visual';
  doc.head.appendChild(link);
  return link;
}

/** Test seams; the default is the page's `requestAnimationFrame`. */
export interface VisualMountOptions {
  scheduler?: FrameScheduler;
}

export function mount(root: HTMLElement, deps: EditorDeps, opts: VisualMountOptions = {}): Mounted {
  const doc = root.ownerDocument;
  const stylesheet = addStylesheet(doc);
  const layout = buildLayout(root);
  const cleanups: (() => void)[] = [];

  const analyzers = new AnalyzerArea(layout.analyzers, deps.store);
  cleanups.push(() => analyzers.dispose());

  const api: VisualApi = {
    mountSpectrum: (el, source) => mountSpectrum(el, deps.store, source),
  };
  deps.visual = api;

  const core = deps.tier === 'browser' ? deps.core : undefined;
  const glCanvas = core ? doc.createElement('canvas') : null;
  const gl = glCanvas ? glCanvas.getContext('webgl2') : null;
  if (!core || !glCanvas || !gl) {
    const notice = deps.tier === 'native' ? NATIVE_NOTICE : core ? NO_WEBGL2_NOTICE : 'visuals: no render source';
    const panes = new VisualPanes(layout.visual, { notice });
    cleanups.push(() => panes.dispose());
  } else {
    glCanvas.width = RENDER_SIZE.width;
    glCanvas.height = RENDER_SIZE.height;
    const host = new GlRenderHost(gl, RENDER_SIZE, { createCanvas: () => doc.createElement('canvas') });
    const panes = new VisualPanes(layout.visual, { source: glCanvas });
    cleanups.push(host.onDiagnostic((d) => panes.showDiagnostic(d)));
    cleanups.push(host.onProgram((out) => panes.clearDiagnostic(out)));
    cleanups.push(core.onRender((rec) => host.onRecord(rec)));
    const loop: FrameLoop = startFrameLoop({
      core,
      host,
      clock: deps.clock,
      scheduler: opts.scheduler ?? windowScheduler(doc.defaultView),
      onFrame: () => panes.present(host),
      onError: (e) => panes.showDiagnostic({ code: 'frame-loop', out: 0, message: `stopped: ${String(e)}` }),
    });
    const onVisibility = (): void => loop.setVisible(doc.visibilityState !== 'hidden');
    doc.addEventListener('visibilitychange', onVisibility);
    onVisibility();
    cleanups.push(() => doc.removeEventListener('visibilitychange', onVisibility));
    cleanups.push(() => loop.dispose());
    cleanups.push(() => host.dispose());
    cleanups.push(() => panes.dispose());
  }

  return {
    dispose(): void {
      for (const c of cleanups.splice(0)) c();
      if (deps.visual === api) delete deps.visual;
      stylesheet?.remove();
    },
  };
}
