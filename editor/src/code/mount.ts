// The `code` area (design 15.1.3, 15.1.5): the CodeMirror 6 editor in the
// code pane (the `.vact` mode, document sync, lint, eval keys and flash,
// playing-step highlighting), the transport bar in its pane, and the
// sample bank browser under the editor. It sets `deps.code`, the `CodeApi`
// of `app/apis.ts`.
//
// The code module never interprets language semantics for bindings: sites,
// forms and spans come from the session.

import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { lintGutter } from '@codemirror/lint';
import { EditorState, Text } from '@codemirror/state';
import { EditorView, keymap, lineNumbers } from '@codemirror/view';
import { createComponent } from 'solid-js';
import { render } from 'solid-js/web';
import type { CodeApi, SampleLibraryApi } from '../app/apis';
import type { ServerEnvelope } from '../protocol/types';
import type { EditorDeps, Mounted } from '../app/deps';
import { pane } from '../app/layout';
import { DiagnosticsController } from './diagnostics';
import { EvalController } from './eval';
import { HighlightScheduler, TimeAnchor, highlightExtension, setPlaying } from './highlight';
import { vactLanguage } from './language';
import { SampleBrowser, SampleLibrary, type DecodedAudio } from './samples';
import { DocumentSync } from './sync';
import { TransportBar } from './transport';
import { CodeSurface } from './code-view';

/** The document the code pane edits. */
export const DOC_FILE = 'main.vact';

function localStore(win: Window | null): Pick<Storage, 'getItem' | 'setItem'> | null {
  try {
    return win?.localStorage ?? null;
  } catch {
    return null;
  }
}

/** The area's stylesheet, emitted as an asset by Vite (no CSS module types are pinned). */
const STYLESHEET = new URL('./code.css', import.meta.url).href;

function addStylesheet(doc: Document): void {
  if (doc.querySelector('link[data-vact-code-css]')) return;
  const link = doc.createElement('link');
  link.rel = 'stylesheet';
  link.href = STYLESHEET;
  link.dataset.vactCodeCss = '';
  doc.head.appendChild(link);
}

export function mount(root: HTMLElement, deps: EditorDeps): Mounted {
  const { client, store, clock, tier } = deps;
  const doc = root.ownerDocument;
  const win = doc.defaultView;
  addStylesheet(doc);
  const codePane = pane(root, 'code');
  const transportPane = pane(root, 'transport');

  const holder = doc.createElement('div');
  const disposeCodeView = render(() => createComponent(CodeSurface, {}), holder);
  const host = holder.firstElementChild as HTMLElement;
  codePane.appendChild(host);

  const initial = Text.of(['']);
  const sync = new DocumentSync(client.document(DOC_FILE), initial);
  const anchor = new TimeAnchor(clock, tier === 'browser' ? 'audio' : 'receipt');

  let view: EditorView | null = null;
  const highlight = new HighlightScheduler({
    clock,
    file: DOC_FILE,
    map: (span, rev) => sync.mapWireSpan(span, rev),
    tempo: () => store.tempo,
    anchor,
    apply: (ranges) => view?.dispatch({ effects: setPlaying.of(ranges) }),
  });
  // Filled once the transport exists: every eval starts audio (the key press
  // is a user gesture) and shows its outcome (design 15.2).
  let onEval: ((reply: Promise<ServerEnvelope>) => void) | null = null;
  const evalCtl = new EvalController({
    client,
    sync,
    onHush: () => highlight.clear(),
    onEval: (reply) => onEval?.(reply),
  });
  const diagnostics = new DiagnosticsController({
    client,
    sync,
    tier,
    ...(deps.core ? { core: deps.core } : {}),
    text: () => view?.state.doc.toString() ?? '',
  });

  view = new EditorView({
    parent: host,
    state: EditorState.create({
      doc: initial,
      extensions: [
        // First: every later listener sees the edit already recorded.
        sync.extension(),
        vactLanguage(),
        lineNumbers(),
        history(),
        lintGutter(),
        highlightExtension(),
        evalCtl.extension(),
        keymap.of([...defaultKeymap, ...historyKeymap]),
      ],
    }),
  });
  const editorView = view;
  evalCtl.attach(editorView);
  diagnostics.attach(editorView);

  const transport = new TransportBar(transportPane, {
    client,
    clock,
    anchor,
    onHush: () => highlight.clear(),
    audio: deps.core?.host.ctx ?? null,
    onRun: () => void evalCtl.evalAll(),
  });
  onEval = (reply) => {
    void transport.audio.start();
    transport.evalStarted(reply);
  };

  const core = deps.core;
  const library =
    tier === 'browser'
      ? new SampleLibrary({
          ...(core ? { core } : {}),
          ...(core
            ? { decode: (bytes: ArrayBuffer): Promise<DecodedAudio> => core.host.ctx.decodeAudioData(bytes) }
            : {}),
        })
      : null;
  const browser = new SampleBrowser(codePane, {
    tier,
    ...(library ? { library } : {}),
    storage: localStore(win),
  });

  // The latest eval revision of the document: the revision of its sites.
  let evalRev: number | null = null;
  const offs: (() => void)[] = [
    client.on('playing', (env) => {
      if (env.kind !== 'playing') return;
      highlight.onPlaying(env.body.events);
      transport.onPlaying(env.body.events);
    }),
    client.on('eval-result', (env) => {
      if (env.kind !== 'eval-result') return;
      if (env.body.file === DOC_FILE) evalRev = env.body.doc_revision;
      transport.onDiagnostics(env.body.diagnostics);
    }),
    client.on('diag', (env) => {
      if (env.kind === 'diag') transport.onDiagnostics(env.body.add);
    }),
    store.subscribe(['tempo'], (_c, s) => {
      if (s.tempo) transport.onTempo(s.tempo);
    }),
    store.subscribe(['levels'], (_c, s) => {
      if (s.levels) transport.onLevels(s.levels);
    }),
    store.subscribe(['manifest'], (_c, s) => browser.setSounds(s.manifest?.sounds ?? [])),
  ];
  if (store.tempo) transport.onTempo(store.tempo);
  if (store.levels) transport.onLevels(store.levels);
  browser.setSounds(store.manifest?.sounds ?? []);

  const samples: SampleLibraryApi = {
    frames: (bank, index) => library?.frames(bank, index) ?? null,
    openBrowser: (bank) => browser.open(bank),
  };

  const api: CodeApi = {
    view: editorView,
    mapWireSpan: (span, rev) => sync.mapWireSpan(span, rev),
    currentRevision: (file) => (file === DOC_FILE ? sync.revision : 0),
    selectedSiteId: () => {
      const rev = evalRev;
      if (rev === null) return null;
      const head = editorView.state.selection.main.head;
      let best: { id: number; len: number } | null = null;
      for (const site of store.sitesOf(DOC_FILE)) {
        const r = sync.mapWireSpan(site.span, rev);
        if (!r || head < r.from || head > r.to) continue;
        if (!best || r.to - r.from < best.len) best = { id: site.id, len: r.to - r.from };
      }
      return best?.id ?? null;
    },
    samples,
  };
  deps.code = api;

  // Per animation frame: highlights and the transport bar.
  let frame: number | null = null;
  const loop = (): void => {
    highlight.tick();
    transport.tick();
    frame = win?.requestAnimationFrame?.(loop) ?? null;
  };
  frame = win?.requestAnimationFrame?.(loop) ?? null;

  return {
    dispose() {
      if (frame !== null) win?.cancelAnimationFrame?.(frame);
      for (const off of offs) off();
      evalCtl.dispose();
      diagnostics.dispose();
      transport.dispose();
      browser.dispose();
      editorView.destroy();
      disposeCodeView();
      host.remove();
      view = null;
      if (deps.code === api) delete deps.code;
    },
  };
}
