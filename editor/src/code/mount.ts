// Canvas code pane composition (design 15.1.5, 15.3.8.2-15.3.8.3).
// The headless CodeSurface owns the document; DOM owns input and accessibility only.

import { Text } from '@codemirror/state';
import { createComponent } from 'solid-js';
import { render } from 'solid-js/web';
import type { CodeApi, CodeAnnotation, SampleLibraryApi } from '../app/apis';
import type { ServerEnvelope } from '../protocol/types';
import type { EditorDeps, Mounted } from '../app/deps';
import { pane } from '../app/layout';
import { FrameScheduler, type FrameHost } from './frame';
import { DiagnosticsController } from './diagnostics';
import { EvalController } from './eval';
import { HighlightScheduler, TimeAnchor } from './highlight';
import { FallbackSpans, SyntaxSpans, type SpanProvider } from './syntax';
import { formatKeymap } from './format';
import { SampleBrowser, SampleLibrary, type DecodedAudio } from './samples';
import { DocumentSync } from './sync';
import { TransportBar } from './transport';
import { CodeSurface } from './code-view';
import { attachCompletion } from './completion-view';
import { CodeSurface as HeadlessSurface } from './surface';
import { InputController } from './input';
import { PointerController, type SelectionHandle } from './pointer';
import { TextLayout } from './layout';
import { CanvasRenderer } from './renderer';
import { ResourceLedger } from './resources';
import { CodeViewHost } from './view-host';
import { installPerfHook, recordOnset, recordPresented, removePerfHook, type VactrPerf } from './perf-hook';

export const DOC_FILE = 'main.vact';

export interface MountOptions {
  gl?: WebGL2RenderingContext;
  frameHost?: FrameHost;
  createCanvas?: () => HTMLCanvasElement;
}

function localStore(win: Window | null): Pick<Storage, 'getItem' | 'setItem'> | null {
  try { return win?.localStorage ?? null; } catch { return null; }
}
const STYLESHEET = new URL('./code.css', import.meta.url).href;
function addStylesheet(doc: Document): void {
  if (doc.querySelector('link[data-vact-code-css]')) return;
  const link = doc.createElement('link'); link.rel = 'stylesheet'; link.href = STYLESHEET; link.dataset.vactCodeCss = '';
  doc.head.appendChild(link);
}
function frameHost(win: Window, override?: FrameHost): FrameHost {
  return override ?? (win as unknown as FrameHost);
}

export function mount(root: HTMLElement, deps: EditorDeps, opts: MountOptions = {}): Mounted {
  const { client, store, clock, tier } = deps;
  const doc = root.ownerDocument; const win = doc.defaultView;
  if (!win) throw new Error('Canvas editor requires a window');
  addStylesheet(doc);
  const codePane = pane(root, 'code'); const transportPane = pane(root, 'transport');
  codePane.dataset.syntax = 'fallback';
  const holder = doc.createElement('div');
  const disposeCanvasHost = render(() => createComponent(CodeSurface, {}), holder);
  const hostEl = holder.firstElementChild as HTMLElement; codePane.appendChild(hostEl);
  const canvas = doc.createElement('canvas'); canvas.className = 'vact-code-canvas'; canvas.setAttribute('aria-hidden', 'true');
  const inputContainer = doc.createElement('div'); inputContainer.className = 'vact-code-input-bridge';
  const gpuStatus = doc.createElement('div'); gpuStatus.className = 'vact-code-gpu-status'; gpuStatus.setAttribute('role', 'status');
  const diagTip = doc.createElement('div'); diagTip.className = 'vact-code-diag-tooltip'; diagTip.setAttribute('role', 'tooltip'); diagTip.hidden = true;
  hostEl.append(canvas, inputContainer, gpuStatus, diagTip);

  const sync = new DocumentSync(client.document(DOC_FILE), Text.of(['']));
  const surface = new HeadlessSurface({ sync });
  const metricsCanvas = doc.createElement('canvas');
  const metrics = metricsCanvas.getContext('2d');
  const textMetrics = metrics ?? { font: '', measureText: (text: string) => ({ width: text.length * 8 }) };
  const layout = new TextLayout(textMetrics, { font: '13px ui-monospace, SFMono-Regular, Menlo, monospace', lineHeight: 18, baseline: 14 });
  const ledger = new ResourceLedger();
  const renderer = new CanvasRenderer(canvas, layout, { ledger, ...(opts.gl ? { gl: opts.gl } : {}),
    ...(opts.createCanvas ? { createCanvas: opts.createCanvas } : {}), onStatus: (status) => {
      gpuStatus.textContent = status.kind === 'unavailable' ? 'GPU unavailable; editing and save remain available' : status.message;
      gpuStatus.dataset.gpu = status.kind;
    } });
  const schedulerHost = frameHost(win, opts.frameHost);
  let scheduler: FrameScheduler | undefined;
  let input: InputController | undefined;
  let pointer: PointerController;
  let viewHost: CodeViewHost;
  let disposed = false;
  let evalRanges: CodeAnnotation[] = [];
  let playingRanges: { from: number; to: number }[] = [];
  let handles: readonly SelectionHandle[] = [];
  let syntaxProvider: SpanProvider = new FallbackSpans();
  let syntaxTruncated = 0;
  let displayDoc: Text | null = null;
  let renderedDoc: Text | null = null;
  let displayRevision = -1;
  let displayDirty = true;
  let perfApi: VactrPerf | null = null;
  let backgroundStop: (() => void) | null = null;
  let backgroundRevision = 0;
  let staticAnnotations: CodeAnnotation[] = [];
  let staticRevision = 0;
  const hideDiagnosticTip = (): void => { diagTip.hidden = true; };
  const anchor = new TimeAnchor(clock, tier === 'browser' ? 'audio' : 'receipt');
  const highlight = new HighlightScheduler({ clock, file: DOC_FILE, map: (span, rev) => sync.mapWireSpan(span, rev),
    tempo: () => store.tempo, anchor, ...(deps.audible ? { audible: deps.audible, epoch: () => store.transportSample?.epoch ?? null } : {}) });
  const evalCtl = new EvalController({ client, sync, onHush: () => highlight.clear(), onEval: (reply) => onEval?.(reply) });
  const diagnostics = new DiagnosticsController({ client, sync, tier, ...(deps.core ? { core: deps.core } : {}),
    text: () => surface.state.doc.toString(), announce: (message) => input?.accessibility.announce(message) });
  evalCtl.attach(surface);
  const disposeFormat = formatKeymap(surface, () => deps.formatter);
  const completion = deps.completion ? attachCompletion(surface, deps.completion) : null;

  const transport = new TransportBar(transportPane, { client, clock, anchor,
    ...(deps.audible ? { audible: deps.audible, sample: () => store.transportSample } : {}),
    onHush: () => highlight.clear(), audio: deps.core?.host.ctx ?? null, onRun: () => void evalCtl.evalAll() });
  let onEval: ((reply: Promise<ServerEnvelope>) => void) | null = null;
  onEval = (reply) => { void transport.audio.start(); transport.evalStarted(reply); };

  const core = deps.core;
  const library = tier === 'browser' ? new SampleLibrary({ ...(core ? { core } : {}),
    ...(core ? { decode: (bytes: ArrayBuffer): Promise<DecodedAudio> => core.host.ctx.decodeAudioData(bytes) } : {}) }) : null;
  const browser = new SampleBrowser(codePane, { tier, ...(library ? { library } : {}), storage: localStore(win) });
  const samples: SampleLibraryApi = { frames: (bank, index) => library?.frames(bank, index) ?? null, openBrowser: (bank) => browser.open(bank) };
  let evalRev: number | null = null;
  const api: CodeApi = {
    surface, mapWireSpan: (span, rev) => sync.mapWireSpan(span, rev),
    currentRevision: (file) => file === DOC_FILE ? sync.revision : 0,
    selectedSiteId: () => {
      if (evalRev === null) return null;
      const head = surface.state.selection.main.head; let best: { id: number; len: number } | null = null;
      for (const site of store.sitesOf(DOC_FILE)) {
        const range = sync.mapWireSpan(site.span, evalRev);
        if (!range || head < range.from || head > range.to) continue;
        if (!best || range.to - range.from < best.len) best = { id: site.id, len: range.to - range.from };
      }
      return best?.id ?? null;
    }, samples,
  };
  deps.code = api;

  const viewport = { width: 1, height: 1, dpr: schedulerHost.devicePixelRatio || 1, keyboardInset: 0 };
  viewHost = new CodeViewHost(hostEl, surface, layout, schedulerHost, () => {
    hideDiagnosticTip(); scheduler?.invalidateText();
  });
  input = new InputController(surface, inputContainer, { label: 'Code editor', scrollCaret: () => viewHost.scrollCaret(),
    onPresentation: (presentation) => {
      if (displayDoc !== presentation.doc) { displayDoc = presentation.doc; layout.setText(displayDoc); displayDirty = true; scheduler?.invalidateText(); }
    } });
  viewHost.setFocus(() => input.focus());
  pointer = new PointerController(surface, canvas, { focus: () => input.focus(), scrollBy: (x, y) => viewHost.scrollBy(x, y),
    onHandles: (next) => { handles = next; scheduler?.setActive('handles', next.length > 0); scheduler?.invalidateText(); },
    composing: () => input.isComposing });
  const forwardPointer = (event: Event): void => surface.notifyPointer(event as PointerEvent);
  const pointerNames = ['pointerdown', 'pointermove', 'pointerup', 'pointercancel'] as const;
  for (const name of pointerNames) canvas.addEventListener(name, forwardPointer);
  const removePointerForwarding = (): void => { for (const name of pointerNames) canvas.removeEventListener(name, forwardPointer); };

  const perfEnabled = new URLSearchParams(win.location.search).get('perf') === '1';
  scheduler = new FrameScheduler(schedulerHost, doc, hostEl, { perf: perfEnabled, revision: () => sync.revision,
    onViewport(info) {
      Object.assign(viewport, info);
      viewHost.setViewport(info);
      renderer.setViewport(viewHost.viewport, info.dpr);
      surface.notifyBlur(); // CompletionPopup closes if the caret can no longer be anchored.
    },
    onFrame(ctx) {
      if (disposed) return;
      const audible = deps.audible?.sample(ctx.frameMs) ?? { time: clock.now(), targetMs: ctx.frameMs, provenance: 'unavailable' as const, valid: false };
      const active = highlight.tick(ctx.frameMs);
      playingRanges = active;
      scheduler?.setActive('playing', highlight.size > 0);
      transport.tick(ctx.frameMs);
      scheduler?.setActive('transport', !!store.tempo);
      if (ctx.textDirty && (displayDirty || displayRevision !== sync.revision)) {
        displayRevision = sync.revision;
        const presentation = input.presentation;
        if (renderedDoc !== presentation.doc) { renderedDoc = presentation.doc; renderer.setText(renderedDoc); }
        displayDirty = false;
      }
      if (ctx.textDirty) {
        const view = viewHost.viewport;
        const first = Math.max(0, Math.floor(view.scrollTop / layout.font.lineHeight) - Math.ceil(view.height / layout.font.lineHeight));
        const last = Math.min(surface.state.doc.lines, Math.ceil((view.scrollTop + view.height * 2) / layout.font.lineHeight));
        const from = surface.state.doc.line(Math.min(surface.state.doc.lines, first + 1)).from;
        const to = surface.state.doc.line(Math.max(1, last)).to;
        const result = syntaxProvider.spans(surface.state, from, to, 16384);
        syntaxTruncated += result.truncated ? 1 : 0;
        if (syntaxProvider instanceof FallbackSpans) codePane.dataset.syntax = 'fallback';
        const selection = surface.state.selection.main;
        staticAnnotations = [...result.spans, ...surface.annotationRanges(),
          ...(selection.empty ? [] : [{ from: selection.from, to: selection.to, kind: 'selection' as const }]),
          ...(input.presentation.annotations)];
        staticRevision++;
        const animated: CodeAnnotation[] = [...playingRanges.map((r) => ({ ...r, kind: 'playing' as const })), ...evalRanges];
        const cursor = input.presentation.cursor;
        renderer.setViewport(view, viewport.dpr);
        renderer.render({ annotations: staticAnnotations, annotationsRevision: staticRevision, animated, textRevision: displayRevision, cursor, cursorVisible: true, handles });
        const dropped = highlight.stats.overflow + highlight.stats.horizonDrops + highlight.stats.epochDrops + client.queueStats.dropped;
        if (dropped) gpuStatus.dataset.telemetryDropped = String(dropped); else delete gpuStatus.dataset.telemetryDropped;
      } else {
        const animated: CodeAnnotation[] = [...playingRanges.map((r) => ({ ...r, kind: 'playing' as const })), ...evalRanges];
        renderer.setViewport(viewHost.viewport, viewport.dpr);
        renderer.render({ annotations: staticAnnotations, annotationsRevision: staticRevision, animated, textRevision: displayRevision, cursor: input.presentation.cursor, cursorVisible: true, handles });
      }
      if (!backgroundStop && deps.visual?.onBackgroundCanvas) backgroundStop = deps.visual.onBackgroundCanvas((background) => {
        renderer.setBackground(background, ++backgroundRevision); scheduler?.request();
      });
      if (renderer.textPending) scheduler?.request();
      const perf = scheduler?.perf;
      if (perf && perfApi) {
        const sample = deps.audible?.sample(ctx.frameMs) ?? audible;
        const ordered = [...playingRanges].sort((a, b) => a.from - b.from || a.to - b.to);
        const activeKey = ordered.map((r) => `${r.from}-${r.to}`).join(',');
        recordPresented(perfApi, { frameMs: ctx.frameMs, targetMs: sample.targetMs, audibleTime: sample.time,
          provenance: sample.provenance, valid: sample.valid, activeKey, beatCycle: transport.state.cycle,
          beatFlash: transport.state.beatFlash, revision: sync.revision, handles: handles.length, epoch: store.transportSample?.epoch ?? null });
      }
    } });
  scheduler.invalidateText();

  const keyRecord = (event: Event): void => scheduler?.perf?.recordKey((event as KeyboardEvent).timeStamp, sync.revision);
  input.accessibility.textarea.addEventListener('keydown', keyRecord);
  const stopFlash = evalCtl.onFlash((range, kind) => {
    if (kind) evalRanges = [...evalRanges.filter((r) => !(r.from === range.from && r.to === range.to)), { ...range, kind: 'eval', className: kind }];
    else evalRanges = evalRanges.filter((r) => !(r.from === range.from && r.to === range.to));
    scheduler.setActive('eval', evalRanges.length > 0); scheduler.request();
  });
  const stopSurface = surface.subscribe((update) => {
    scheduler.invalidateText();
    if (update.docChanged) hideDiagnosticTip();
    if (update.docChanged) syntaxProvider.noteChanges?.(update.changes, update.state);
  });
  diagnostics.attach(surface);
  const showDiagnosticTip = (event: Event): void => {
    const pointerEvent = event as PointerEvent;
    if (input.isComposing || pointerEvent.buttons !== 0) { hideDiagnosticTip(); return; }
    const pos = surface.posAtCoords({ x: pointerEvent.clientX, y: pointerEvent.clientY });
    const diagnostic = pos === null ? null : diagnostics.diagnosticAt(pos);
    const coords = diagnostic ? surface.coordsAtPos(diagnostic.from) : null;
    if (!diagnostic || !coords) { hideDiagnosticTip(); return; }
    const hostRect = hostEl.getBoundingClientRect();
    diagTip.textContent = diagnostic.message;
    diagTip.style.left = `${coords.left - hostRect.left}px`;
    diagTip.style.top = `${coords.bottom - hostRect.top}px`;
    diagTip.hidden = false;
  };
  canvas.addEventListener('pointermove', showDiagnosticTip);
  canvas.addEventListener('pointerleave', hideDiagnosticTip);

  const offs: (() => void)[] = [
    client.on('playing', (env) => {
      if (env.kind !== 'playing') return;
      highlight.onPlaying(env.body.events); transport.onPlaying(env.body.events); scheduler.setActive('playing', true);
    }),
    client.on('eval-result', (env) => {
      if (env.kind !== 'eval-result') return;
      if (env.body.file === DOC_FILE) evalRev = env.body.doc_revision;
      transport.onDiagnostics(env.body.diagnostics);
    }),
    client.on('diag', (env) => { if (env.kind === 'diag') transport.onDiagnostics(env.body.add); }),
    store.subscribe(['tempo'], (_c, s) => { if (s.tempo) transport.onTempo(s.tempo); }),
    store.subscribe(['levels'], (_c, s) => { if (s.levels) transport.onLevels(s.levels); }),
    store.subscribe(['manifest'], (_c, s) => browser.setSounds(s.manifest?.sounds ?? [])),
    highlight.onAccept((onset) => recordOnset(perfApi, { ...onset, receivedMs: win.performance.now() })),
  ];
  if (store.tempo) transport.onTempo(store.tempo);
  if (store.levels) transport.onLevels(store.levels);
  browser.setSounds(store.manifest?.sounds ?? []);

  if (deps.syntax) void deps.syntax().then((syntax) => {
    if (disposed) return;
    syntaxProvider = new SyntaxSpans(syntax); codePane.dataset.syntax = 'tree-sitter'; scheduler.invalidateText();
  }, () => undefined);
  if (perfEnabled && scheduler.perf) perfApi = installPerfHook({ win, perf: scheduler.perf, surface, revision: () => sync.revision,
    ledger, highlight, renderer, client, store, syntaxTruncated: () => syntaxTruncated,
    disposeCode: () => mounted.dispose() });

  const mounted: Mounted = {
    dispose() {
      if (disposed) return; disposed = true;
      scheduler.dispose(); pointer.dispose(); input.dispose(); renderer.dispose(); viewHost.dispose();
      removePointerForwarding(); input.accessibility.textarea.removeEventListener('keydown', keyRecord);
      canvas.removeEventListener('pointermove', showDiagnosticTip); canvas.removeEventListener('pointerleave', hideDiagnosticTip);
      for (const off of offs) off(); stopFlash(); stopSurface(); diagnostics.dispose(); evalCtl.dispose();
      disposeFormat(); completion?.dispose(); syntaxProvider.dispose?.(); backgroundStop?.();
      transport.dispose(); browser.dispose(); surface.dispose(); disposeCanvasHost(); hostEl.remove();
      if (deps.code === api) delete deps.code;
      removePerfHook(win);
    },
  };
  return mounted;
}
