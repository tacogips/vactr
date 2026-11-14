import { createSignal, type Accessor } from 'solid-js';
import { Transaction } from '@codemirror/state';
import type { Formatter } from './format';
import { formatDocument } from './format';
import type { EvalController } from './eval';
import type { TextLayout } from 'emaki/engine';
import type { CodeSurface } from './surface';
import type { InputController } from 'emaki/engine';
import { browserClipboard } from './vim-clipboard';
import { readVimEnabled, writeVimEnabled } from './vim-settings';
import { mountVimStatus } from './vim-status-view';
import { compileKeymap, createVimController, KEYMAP_PRESETS, LocalKeymapStore, normalizeKey } from 'emaki/vim';
import type { HopLabel, VimController, VimEngineOptions, VimHost, VimSnapshot } from 'emaki/vim';
import type { ViewHost as CodeViewHost } from 'emaki/engine';
import type { StorageLike } from '../ui/shell';
import { FileCancelled } from '../platform/files';
import { BrowserJsonFiles, JsonFileTooLarge, type JsonFiles } from '../platform/json-files';
import { migrateLegacyActionIds, VIM_KEYMAP_KEY, type KeymapStore } from './vim-keymap-store';
import { isFileBoundary } from './file-boundary';
import { mountVimHints } from './vim-hints-view';
import { mountVimKeymapPanel } from './vim-keymap-view';

export type CursorShape = VimSnapshot['cursorShape'];
export const VIM_SYNC_MESSAGE = 'vactr syncs the document live; use :eval, :evalall or :hush';
const SYNC_COMMANDS = new Set(['w', 'q', 'wq', 'x']);

export interface VimDeps {
  surface: CodeSurface;
  input: InputController;
  viewHost: CodeViewHost;
  layout: TextLayout;
  evalCtl: EvalController;
  formatter: () => Formatter | undefined;
  diagnostics?: { current(): readonly { from: number }[] };
  onChange: () => void;
}

export interface VimCursor { cursor: number; cursorShape?: CursorShape; cursorEnd?: number | null }
export interface VimMode {
  enabled: Accessor<boolean>;
  toggle(): void;
  connect(deps: VimDeps): void;
  cursor(defaultCursor: number): VimCursor;
  controller(): VimController | null;
  overlay(): { hopLabels?: readonly HopLabel[]; hopRevision?: number };
  dispose(): void;
}

/** Maps a CodeSurface onto the emaki VimHost contract; viewport rows follow the uniform line height. */
export function createVimHost(deps: Pick<VimDeps, 'surface' | 'viewHost' | 'layout'>): VimHost {
  const { surface, viewHost, layout } = deps;
  const lineHeight = (): number => layout.font.lineHeight;
  const topLine = (): number => Math.max(1, Math.min(surface.state.doc.lines, Math.ceil(viewHost.viewport.scrollTop / lineHeight()) + 1));
  const rows = (): number => Math.max(1, Math.floor(viewHost.viewport.height / lineHeight()));
  return {
    get state() { return surface.state; },
    dispatch: (spec) => surface.dispatch(spec),
    undo: () => surface.undo(),
    redo: () => surface.redo(),
    get scrollTop() { return viewHost.viewport.scrollTop; },
    scrollTo: (target) => {
      if ('pos' in target) viewHost.scrollCaret();
      else viewHost.scrollBy(0, target.top - viewHost.viewport.scrollTop);
    },
    lineTop: (line) => (line - 1) * lineHeight(),
    lineAtHeight: (y) => Math.max(1, Math.min(surface.state.doc.lines, Math.floor(y / lineHeight()) + 1)),
    get viewport() { return { fromLine: topLine(), toLine: topLine() + rows() - 1, top: viewHost.viewport.scrollTop, height: viewHost.viewport.height }; },
    coordsAtPos: (pos) => layout.coordsAtPos(pos, viewHost.viewport),
    marks: () => ({ set: () => undefined, clear: () => undefined }),
  };
}

/** Engine options for vactr: app actions, host hooks, the sync-message ex handler and vactr-owned rendering. */
export function createVimOptions(deps: VimDeps): VimEngineOptions {
  const { surface, input, viewHost, layout, evalCtl } = deps;
  const win = hostWindow(input.accessibility.textarea);
  const diagnosticPosition = (from: number, direction: 1 | -1): number | null => {
    const positions = [...new Set((deps.diagnostics?.current() ?? []).map((item) => item.from))].sort((a, b) => a - b);
    if (!positions.length) return null;
    return direction > 0
      ? positions.find((position) => position > from) ?? positions[0]!
      : [...positions].reverse().find((position) => position < from) ?? positions[positions.length - 1]!;
  };
  return {
    clipboard: browserClipboard(),
    timer: (ms, cb) => { const id = win.setTimeout(cb, ms); return () => win.clearTimeout(id); },
    render: { search: false, hints: false, hop: false },
    actions: {
      'emaki.evaluate': () => { void evalCtl.evalAtCursor(); },
      'emaki.evaluateAll': () => { void evalCtl.evalAll(); },
      'emaki.hush': () => { evalCtl.hush(); },
      'emaki.format': () => { const formatter = deps.formatter(); if (formatter) void formatDocument(surface, formatter); },
      'emaki.diagnostic.next': (ctx) => diagnosticPosition(ctx.cursor, 1),
      'emaki.diagnostic.prev': (ctx) => diagnosticPosition(ctx.cursor, -1),
      'emaki.completion': () => { surface.runKeymaps(new win.KeyboardEvent('keydown', { key: ' ', code: 'Space', ctrlKey: true, cancelable: true }), 'highest'); },
    },
    ex: (command) => {
      if (!SYNC_COMMANDS.has(command.name)) return false;
      input.accessibility.announce(VIM_SYNC_MESSAGE);
      return { message: VIM_SYNC_MESSAGE, kind: 'info' };
    },
    hooks: {
      announce: (message, assertive) => input.accessibility.announce(message, assertive),
      closePopupWithEscape: () => surface.runKeymaps(new win.KeyboardEvent('keydown', { key: 'Escape', cancelable: true }), 'highest'),
      platform: /Mac|iP/.test(navigator.platform) ? 'apple' : 'other',
      matchesDefaultKeymap: (event) => surface.matchesKeymap({
        key: event.key, code: event.code ?? '', ctrlKey: event.ctrlKey, metaKey: event.metaKey,
        shiftKey: event.shiftKey, altKey: event.altKey,
      }, 'default'),
      compositionActive: () => surface.compositionRange !== null,
      historyReduced: () => surface.retentionStatus.reducedDepth,
      scrollLines: (lines) => viewHost.scrollBy(0, lines * layout.font.lineHeight),
      scrollCaret: () => viewHost.scrollCaret(),
      typeText: (text) => input.typeText(text),
      defaultKey: (key) => input.defaultKey(key),
      isVisible: (pos) => {
        const viewport = viewHost.viewport;
        const rect = layout.coordsAtPos(pos, viewport);
        return rect !== null && rect.left >= 0 && rect.right <= viewport.width && rect.top >= 0 && rect.bottom <= viewport.height;
      },
      isDocumentBoundary: isFileBoundary,
      routeMetaKeys: true,
    },
  };
}

/** Builds the emaki engine over a CodeSurface and forwards external transactions to it. */
export function createSurfaceVim(deps: VimDeps): { controller: VimController; dispose(): void } {
  const controller = createVimController(createVimHost(deps), createVimOptions(deps));
  const stop = deps.surface.subscribe((update) => controller.noteUpdate(update));
  return { controller, dispose() { stop(); controller.dispose(); } };
}

export interface VimKeymapDeps { store: KeymapStore; files: JsonFiles }

export function createVimMode(storage: StorageLike | null, hostEl: HTMLElement, keymapDeps?: VimKeymapDeps): VimMode {
  const win = hostWindow(hostEl);
  const keymap = keymapDeps ?? { store: new LocalKeymapStore({ key: VIM_KEYMAP_KEY, storage: localStorageFor(win) ?? undefined }), files: new BrowserJsonFiles(win) };
  const [enabled, setEnabled] = createSignal(readVimEnabled(storage));
  const [keymapText, setKeymapText] = createSignal(KEYMAP_PRESETS[0]!.text);
  const [issues, setIssues] = createSignal<ReturnType<typeof compileKeymap>['issues']>([]);
  const [panelOpen, setPanelOpen] = createSignal(false);
  const [activePreset, setActivePreset] = createSignal<string | null>(null);
  const [panelStatus, setPanelStatus] = createSignal<string | null>(null);
  let deps: VimDeps | null = null;
  let vim: VimController | null = null;
  let stopVim: (() => void) | null = null;
  let stopStatus: (() => void) | null = null;
  let stopSnapshot: (() => void) | null = null;
  let stopHints: (() => void) | null = null;
  let stopPanel: (() => void) | null = null;
  let stopFocusout: (() => void) | null = null;
  let loaded = false;
  const apply = (value: ReturnType<typeof compileKeymap>['keymap']): void => { compiledSignal = value; vim?.setKeymap(value); };
  let compiledSignal: ReturnType<typeof compileKeymap>['keymap'] = null;
  const compile = (text: string): ReturnType<typeof compileKeymap> => compileKeymap(migrateLegacyActionIds(text));
  const load = async (): Promise<void> => {
    try {
      const stored = await keymap.store.load();
      if (stored !== null) {
        setKeymapText(stored);
        const result = compile(stored);
        setIssues(result.issues); apply(result.keymap);
        const skipped = result.issues.filter((issue) => issue.severity === 'error').length;
        if (skipped) deps?.input.accessibility.announce(`keymap: ${skipped} entries skipped`);
      }
    } catch {
      setPanelStatus('keymap not loaded: storage unavailable');
      deps?.input.accessibility.announce('keymap not loaded: storage unavailable');
    } finally { loaded = true; }
  };
  const disable = (): void => {
    if (!deps || !vim) return;
    deps.input.setVim(null);
    stopFocusout?.(); stopFocusout = null;
    stopHints?.(); stopHints = null;
    stopPanel?.(); stopPanel = null;
    stopSnapshot?.(); stopSnapshot = null;
    stopVim?.(); stopVim = null; vim = null;
    stopStatus?.(); stopStatus = null;
    setPanelOpen(false);
    delete hostEl.dataset.vim;
    deps.onChange();
  };
  const enable = (): void => {
    if (!deps || vim) return;
    const created = createSurfaceVim(deps);
    vim = created.controller; stopVim = created.dispose;
    vim.setKeymap(compiledSignal);
    deps.input.setVim(vim);
    hostEl.dataset.vim = 'on';
    const [snapshot, setSnapshot] = createSignal<VimSnapshot>(vim.snapshot);
    stopSnapshot = vim.subscribe((next) => { setSnapshot(next); deps?.onChange(); });
    stopHints = mountVimHints(hostEl, () => snapshot().hints ?? null);
    stopPanel = mountVimKeymapPanel(hostEl, {
      open: panelOpen, text: keymapText, issues, presets: KEYMAP_PRESETS,
      activePreset, status: panelStatus,
      onPreset(id) {
        const preset = KEYMAP_PRESETS.find((item) => item.id === id);
        if (!preset) return;
        setKeymapText(preset.text); setActivePreset(id); setPanelStatus(null);
      },
      onInput(text) { setKeymapText(text); setActivePreset(null); },
      onApply() {
        const result = compile(keymapText()); setIssues(result.issues);
        if (!result.keymap) return;
        apply(result.keymap);
        void keymap.store.save(keymapText()).then((saved) => {
          setPanelStatus(saved.ok ? 'saved' : saved.reason === 'too-large' ? 'keymap not saved: too-large' : 'keymap not saved: storage unavailable');
        }, (error: unknown) => setPanelStatus(String(error)));
      },
      onImport() {
        void keymap.files.importJson().then((text) => { setKeymapText(text); setActivePreset(null); setPanelStatus(null); })
          .catch((error: unknown) => {
            if (error instanceof FileCancelled) return;
            setPanelStatus(error instanceof JsonFileTooLarge ? 'keymap import: too-large' : String(error));
          });
      },
      onExport() { void keymap.files.exportJson('vactr-vim-keymap.json', keymapText()).catch((error: unknown) => setPanelStatus(String(error))); },
      onReset() {
        void keymap.store.clear().then(() => 'reset', (error: unknown) => String(error)).then((message) => {
          setKeymapText(KEYMAP_PRESETS[0]!.text); setIssues([]); setActivePreset('default'); apply(null);
          setPanelStatus(message);
        });
      },
      onClose() { setPanelOpen(false); deps?.input.focus(); },
    });
    const bridge = deps.input.accessibility.textarea.parentElement;
    if (bridge) {
      const onFocusout = (): void => vim?.blur();
      bridge.addEventListener('focusout', onFocusout);
      stopFocusout = () => bridge.removeEventListener('focusout', onFocusout);
    }
    stopStatus = mountVimStatus(hostEl, snapshot, {
      submit: (text) => { vim?.submitPrompt(text); deps?.input.focus(); },
      cancel: () => { vim?.cancelPrompt(); deps?.input.focus(); },
      tapMode: () => { vim?.tapMode(); deps?.input.focus(); },
      openKeymap: () => { setPanelStatus(null); setPanelOpen(true); },
    }, (input) => {
      const key = normalizeKey(input, true);
      return key === null ? null : compiledSignal?.cmdline.get(key) ?? null;
    });
    if (!loaded) void load();
    deps.onChange();
  };
  return {
    enabled,
    toggle() {
      const next = !enabled();
      setEnabled(next); writeVimEnabled(storage, next);
      if (next) enable(); else disable();
      deps?.input.focus();
    },
    connect(nextDeps) { deps = nextDeps; if (enabled()) enable(); },
    cursor(defaultCursor) {
      if (!vim || !deps || deps.input.isComposing) return { cursor: defaultCursor };
      const snapshot = vim.snapshot;
      return {
        cursor: snapshot.displayCursor,
        cursorShape: snapshot.cursorShape,
        cursorEnd: snapshot.displayCursorEnd === snapshot.displayCursor ? null : snapshot.displayCursorEnd,
      };
    },
    controller: () => vim,
    overlay() {
      const hop = vim?.snapshot.hop;
      return hop ? { hopLabels: hop.labels, hopRevision: hop.revision } : {};
    },
    dispose() { disable(); deps = null; },
  };
}

function hostWindow(element: HTMLElement): Window & typeof globalThis {
  const win = element.ownerDocument.defaultView;
  if (!win) throw new Error('Vim mount requires a window');
  return win as Window & typeof globalThis;
}
function localStorageFor(win: Window): Pick<Storage, 'getItem' | 'setItem' | 'removeItem'> | null {
  try { return win.localStorage; } catch { return null; }
}
