import { createSignal, type Accessor } from 'solid-js';
import type { Formatter } from './format';
import { formatDocument } from './format';
import type { EvalController } from './eval';
import type { TextLayout } from './layout';
import type { CodeSurface } from './surface';
import type { InputController } from './input';
import { browserClipboard } from './vim-clipboard';
import { readVimEnabled, writeVimEnabled } from './vim-settings';
import { mountVimStatus } from './vim-status-view';
import { VimController } from './vim/controller';
import type { CursorShape, VimHost, VimSnapshot } from './vim/types';
import type { CodeViewHost } from './view-host';
import type { StorageLike } from '../ui/shell';
import { FileCancelled } from '../platform/files';
import { BrowserJsonFiles, JsonFileTooLarge, type JsonFiles } from '../platform/json-files';
import { compileKeymap } from './vim/keymap-schema';
import { KEYMAP_PRESETS } from './vim/presets';
import { normalizeKey } from './vim/keys';
import { LocalKeymapStore, type KeymapStore } from './vim-keymap-store';
import { mountVimHints } from './vim-hints-view';
import { mountVimKeymapPanel } from './vim-keymap-view';
import type { HopLabel } from './vim/types';

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

export function createSurfaceHost(deps: VimDeps): VimHost {
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
    state: () => surface.state,
    dispatch: (spec) => surface.dispatch(spec),
    subscribe: (cb) => surface.subscribe(cb),
    undo: () => surface.undo(),
    redo: () => surface.redo(),
    historyReduced: () => surface.retentionStatus.reducedDepth,
    viewport: () => {
      const lineHeight = layout.font.lineHeight;
      const viewport = viewHost.viewport;
      return {
        topLine: Math.max(1, Math.min(surface.state.doc.lines, Math.ceil(viewport.scrollTop / lineHeight) + 1)),
        rows: Math.max(1, Math.floor(viewport.height / lineHeight)),
      };
    },
    scrollLines: (lines) => viewHost.scrollBy(0, lines * layout.font.lineHeight),
    scrollCaret: () => viewHost.scrollCaret(),
    clipboard: browserClipboard(),
    actions: {
      evalAtCursor: () => { void evalCtl.evalAtCursor(); },
      evalAll: () => { void evalCtl.evalAll(); },
      hush: () => evalCtl.hush(),
      format: () => { const formatter = deps.formatter(); if (formatter) void formatDocument(surface, formatter); },
      diagnosticNext: (from) => diagnosticPosition(from, 1),
      diagnosticPrev: (from) => diagnosticPosition(from, -1),
      openCompletion: () => { surface.runKeymaps(new win.KeyboardEvent('keydown', { key: ' ', code: 'Space', ctrlKey: true, cancelable: true }), 'highest'); },
    },
    closePopupWithEscape: () => surface.runKeymaps(new win.KeyboardEvent('keydown', { key: 'Escape', cancelable: true }), 'highest'),
    announce: (message, assertive) => input.accessibility.announce(message, assertive),
    platform: /Mac|iP/.test(navigator.platform) ? 'apple' : 'other',
    matchesDefaultKeymap: (event) => surface.matchesKeymap({
      key: event.key, code: event.code ?? '', ctrlKey: event.ctrlKey, metaKey: event.metaKey,
      shiftKey: event.shiftKey, altKey: event.altKey,
    }, 'default'),
    compositionActive: () => surface.compositionRange !== null,
    typeText: (text) => input.typeText(text),
    defaultKey: (key) => input.defaultKey(key),
    isVisible: (pos) => {
      const viewport = viewHost.viewport;
      const rect = layout.coordsAtPos(pos, viewport);
      return rect !== null && rect.left >= 0 && rect.right <= viewport.width && rect.top >= 0 && rect.bottom <= viewport.height;
    },
    timer: (ms, cb) => { const id = win.setTimeout(cb, ms); return () => win.clearTimeout(id); },
  };
}

export interface VimKeymapDeps { store: KeymapStore; files: JsonFiles }

export function createVimMode(storage: StorageLike | null, hostEl: HTMLElement, keymapDeps?: VimKeymapDeps): VimMode {
  const win = hostWindow(hostEl);
  const keymap = keymapDeps ?? { store: new LocalKeymapStore(localStorageFor(win)), files: new BrowserJsonFiles(win) };
  const [enabled, setEnabled] = createSignal(readVimEnabled(storage));
  const [keymapText, setKeymapText] = createSignal(KEYMAP_PRESETS[0]!.text);
  const [issues, setIssues] = createSignal<ReturnType<typeof compileKeymap>['issues']>([]);
  const [panelOpen, setPanelOpen] = createSignal(false);
  const [activePreset, setActivePreset] = createSignal<string | null>(null);
  const [panelStatus, setPanelStatus] = createSignal<string | null>(null);
  let deps: VimDeps | null = null;
  let vim: VimController | null = null;
  let stopStatus: (() => void) | null = null;
  let stopSnapshot: (() => void) | null = null;
  let stopHints: (() => void) | null = null;
  let stopPanel: (() => void) | null = null;
  let stopFocusout: (() => void) | null = null;
  let loaded = false;
  const apply = (value: ReturnType<typeof compileKeymap>['keymap']): void => { compiledSignal = value; vim?.setKeymap(value); };
  let compiledSignal: ReturnType<typeof compileKeymap>['keymap'] = null;
  const compile = (text: string): ReturnType<typeof compileKeymap> => compileKeymap(text);
  const load = async (): Promise<void> => {
    try {
      const stored = await keymap.store.get();
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
    vim.dispose(); vim = null;
    stopStatus?.(); stopStatus = null;
    setPanelOpen(false);
    delete hostEl.dataset.vim;
    deps.onChange();
  };
  const enable = (): void => {
    if (!deps || vim) return;
    vim = new VimController(createSurfaceHost(deps));
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
        void keymap.store.set(keymapText()).then((saved) => {
          setPanelStatus(saved.ok ? 'saved' : saved.reason === 'too-large' ? 'keymap not saved: too-large' : saved.message ?? 'keymap not saved: storage unavailable');
        });
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
        void keymap.store.reset().then((result) => {
          setKeymapText(KEYMAP_PRESETS[0]!.text); setIssues([]); setActivePreset('default'); apply(null);
          setPanelStatus(result.ok ? 'reset' : result.reason === 'too-large' ? 'keymap not saved: too-large' : result.message ?? 'keymap not saved: storage unavailable');
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
