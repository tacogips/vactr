import { EditorState, Transaction, type Extension, type TransactionSpec } from '@codemirror/state';
import { EditorCore, type EditorUpdate } from './core/editor-core';
import { DecorationStore } from './decor/store';
import { InputController } from './input/input';
import { PointerController, type SelectionHandle } from './input/pointer';
import { IncrementalHighlighter } from './language/incremental';
import { createMeasureContext } from './layout/advances';
import { TextLayout } from './layout/layout';
import { PluginRuntime } from './plugin/runtime';
import { WidgetSet } from './plugin/widgets';
import { FrameScheduler, type FrameHost } from './render/frame';
import { FALLBACK_PALETTE, type Palette } from './render/palette';
import { CanvasRenderer } from './render/renderer';
import { ResourceLedger } from './render/resources';
import { ViewHost } from './view/view-host';
import { WidgetLayer } from './view/widget-layer';
import { renderEditorFrame } from './editor-frame';
import type { EmakiPlugin, LanguageProvider, ModalKeyHook, Rgba } from './types';

export interface EditorOptions {
  doc?: string; language?: LanguageProvider; plugins?: readonly EmakiPlugin[];
  theme?: { palette?: Partial<Palette>; tokens?: Record<string, Rgba> };
  font?: { family: string; size: number; lineHeight: number; baseline: number };
  readOnly?: boolean; lineNumbers?: boolean; extensions?: Extension; modalKeys?: ModalKeyHook;
  gl?: WebGL2RenderingContext; frameHost?: FrameHost; createCanvas?: () => HTMLCanvasElement;
}
export interface PresentSample { frameStart: number; frameEnd: number; workMs: number; revision: number; visibleComplete: boolean }
export interface InputSample { eventTimeStamp: number; start: number; end: number; workMs: number; revisionAfter: number }
export interface EditorHandle {
  readonly state: EditorState; dispatch(spec: TransactionSpec): void; undo(): boolean; redo(): boolean; setDoc(text: string): void;
  onUpdate(cb: (update: EditorUpdate) => void): () => void; onScroll(cb: (top: number) => void): () => void;
  readonly scrollTop: number; scrollTo(target: { top: number } | { pos: number }): void;
  lineTop(line: number): number; lineAtHeight(y: number): number; addPlugin(plugin: EmakiPlugin): () => void;
  focus(): void; requestFrame(): void; readonly revision: number; readonly stats: Readonly<Record<string, number>>;
  readonly perf: { onPresent(cb: (sample: PresentSample) => void): () => void; onInput(cb: (sample: InputSample) => void): () => void };
  destroy(): void;
}
export function createEditor(host: HTMLElement, options: EditorOptions = {}): EditorHandle {
  const doc = host.ownerDocument; const win = doc.defaultView;
  if (!win) throw new Error('Editor requires a window');
  const frameHost = options.frameHost ?? win as unknown as FrameHost;
  const font = options.font ?? { family: 'ui-monospace, SFMono-Regular, Menlo, monospace', size: 13, lineHeight: 18, baseline: 14 };
  const palette: Palette = { ...FALLBACK_PALETTE, ...options.theme?.palette,
    token: { ...FALLBACK_PALETTE.token, ...options.theme?.palette?.token, ...options.theme?.tokens } };
  const core = new EditorCore({ doc: options.doc ?? '', extensions: [options.extensions ?? [], ...(options.readOnly ? [
    EditorState.readOnly.of(true), EditorState.transactionFilter.of(tr => tr.docChanged && /^(input|delete)(\.|$)/.test(tr.annotation(Transaction.userEvent) ?? '') ? [] : tr),
  ] : [])] });
  const previousPosition = host.style.position;
  if (win.getComputedStyle(host).position === 'static') host.style.position = 'relative';
  const canvas = doc.createElement('canvas'); canvas.className = 'emaki-canvas'; canvas.setAttribute('aria-hidden', 'true');
  Object.assign(canvas.style, { display: 'block', width: '100%', height: '100%' });
  const bridge = doc.createElement('div'); bridge.className = 'emaki-input-bridge'; host.append(canvas, bridge);
  const metrics = createMeasureContext(doc) ?? { font: '', measureText: (text: string) => ({ width: text.length * 8 }) };
  const layout = new TextLayout(metrics, { font: `${font.size}px ${font.family}`, lineHeight: font.lineHeight, baseline: font.baseline });
  layout.setText(core.state.doc);
  const ledger = new ResourceLedger(); const renderer = new CanvasRenderer(canvas, layout, { ledger, palette,
    ...(options.gl ? { gl: options.gl } : {}), ...(options.createCanvas ? { createCanvas: options.createCanvas } : {}) });
  const widgets = new WidgetSet(core.state.doc.length); const store = new DecorationStore(() => core.state.doc.length);
  const layer = new WidgetLayer(host, layout, ledger);
  let scheduler: FrameScheduler | undefined; let revision = 0; let presentationRevision = 0; let disposed = false; let widgetRevision = -1; let dpr = 1;
  let handles: readonly SelectionHandle[] = []; const updates = new Set<(u: EditorUpdate) => void>(); const scrolls = new Set<(top: number) => void>();
  const presents = new Set<(s: PresentSample) => void>(); const inputs = new Set<(s: InputSample) => void>();
  const view = new ViewHost(host, core, layout, frameHost, () => { scheduler?.invalidateText('view'); for (const cb of scrolls) cb(view.scrollPosition.top); }, options.lineNumbers !== false);
  const input = new InputController(core, bridge, { scrollCaret: () => view.scrollCaret(), onPresentation: p => {
    layout.setText(p.doc, p.changes); presentationRevision++; scheduler?.invalidateText('doc');
  } });
  input.setModalHook(options.modalKeys ?? null); view.setFocus(() => input.focus());
  const pointer = new PointerController(core, canvas, { focus: () => input.focus(), scrollBy: (x, y) => view.scrollBy(x, y),
    onHandles: next => { handles = next; scheduler?.invalidateText('selection'); }, composing: () => input.isComposing });
  const language = options.language; const highlighter = language?.kind === 'lines' ? new IncrementalHighlighter(language) : undefined;
  highlighter?.setDoc(core.state.doc);
  const runtime = new PluginRuntime({ store, widgets, getState: () => core.state, requestFrame: () => scheduler?.invalidateText('annotations'),
    coordsAtPos: pos => core.coordsAtPos(pos), lineTop: line => layout.heightmap.lineTop(line),
    viewport: () => { const v = view.viewport; return { fromLine: layout.heightmap.lineAtHeight(v.scrollTop).line,
      toLine: layout.heightmap.lineAtHeight(v.scrollTop + v.height).line + 1, top: v.scrollTop, height: v.height }; },
    theme: () => ({ palette, tokens: palette.token }) });
  const stopUpdate = core.subscribe(update => {
    if (update.docChanged) { revision++; runtime.docChanged(update); layout.setText(update.state.doc, update.changes);
      highlighter?.applyChanges(update.changes, update.state.doc); if (language?.kind === 'spans') language.noteChanges?.(update.changes, update.state); }
    scheduler?.invalidateText(update.docChanged ? 'doc' : 'selection'); for (const cb of updates) cb(update);
  });
  language?.kind === 'spans' && language.setInvalidate?.(() => scheduler?.invalidateText('syntax'));
  scheduler = new FrameScheduler(frameHost, doc, host, { onViewport(info) { dpr = info.dpr; view.setViewport(info); }, onFrame(ctx) {
    const start = performance.now(); const result = renderEditorFrame({ core, layout, renderer, view, input, widgets, layer, store, runtime,
      ...(language ? { language } : {}), ...(highlighter ? { highlighter } : {}), scheduler: scheduler!, handles, revision: presentationRevision, frameMs: ctx.frameMs, dpr, widgetRevision });
    widgetRevision = result.widgetRevision; const end = performance.now();
    if (result.rendered) for (const cb of presents) cb({ frameStart: start, frameEnd: end, workMs: end - start, revision, visibleComplete: result.visibleComplete });
  } });
  const starts = new WeakMap<Event, number>(); const capture = (event: Event) => { starts.set(event, performance.now()); };
  const bubble = (event: Event) => { const end = performance.now(), start = starts.get(event) ?? end;
    for (const cb of inputs) cb({ eventTimeStamp: event.timeStamp, start, end, workMs: end - start, revisionAfter: revision }); };
  for (const name of ['keydown', 'beforeinput']) { host.addEventListener(name, capture, true); host.addEventListener(name, bubble); }
  for (const plugin of options.plugins ?? []) runtime.add(plugin);
  scheduler.invalidateText('doc');
  return {
    get state() { return core.state; }, dispatch: spec => core.dispatch(spec), undo: () => core.undo(), redo: () => core.redo(),
    setDoc: text => { core.dispatch({ changes: { from: 0, to: core.state.doc.length, insert: text }, annotations: Transaction.addToHistory.of(false) }); layout.resetWidestShaped(); },
    onUpdate: cb => { updates.add(cb); return () => { updates.delete(cb); }; }, onScroll: cb => { scrolls.add(cb); return () => { scrolls.delete(cb); }; },
    get scrollTop() { return view.scrollPosition.top; }, scrollTo: target => { const top = 'top' in target ? target.top : layout.heightmap.lineTop(core.state.doc.lineAt(target.pos).number - 1); view.scrollBy(0, top - view.scrollPosition.top); },
    lineTop: line => layout.heightmap.lineTop(line), lineAtHeight: y => layout.heightmap.lineAtHeight(y).line,
    addPlugin: plugin => runtime.add(plugin), focus: () => input.focus(), requestFrame: () => scheduler?.request(),
    get revision() { return revision; }, get stats() { return renderer.stats; },
    perf: { onPresent: cb => { presents.add(cb); return () => { presents.delete(cb); }; }, onInput: cb => { inputs.add(cb); return () => { inputs.delete(cb); }; } },
    destroy() { if (disposed) return; disposed = true; scheduler?.dispose(); runtime.destroy(); stopUpdate(); pointer.dispose(); input.dispose(); layer.dispose(); renderer.dispose(); view.dispose(); core.dispose();
      if (language?.kind === 'spans') language.dispose?.(); for (const name of ['keydown', 'beforeinput']) { host.removeEventListener(name, capture, true); host.removeEventListener(name, bubble); }
      canvas.remove(); bridge.remove(); host.style.position = previousPosition; updates.clear(); scrolls.clear(); presents.clear(); inputs.clear(); },
  };
}
