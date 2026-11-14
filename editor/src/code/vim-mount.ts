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

export interface VimDeps {
  surface: CodeSurface;
  input: InputController;
  viewHost: CodeViewHost;
  layout: TextLayout;
  evalCtl: EvalController;
  formatter: () => Formatter | undefined;
  onChange: () => void;
}

export interface VimCursor { cursor: number; cursorShape?: CursorShape; cursorEnd?: number | null }
export interface VimMode {
  enabled: Accessor<boolean>;
  toggle(): void;
  connect(deps: VimDeps): void;
  cursor(defaultCursor: number): VimCursor;
  controller(): VimController | null;
  dispose(): void;
}

export function createSurfaceHost(deps: VimDeps): VimHost {
  const { surface, input, viewHost, layout, evalCtl } = deps;
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
    },
    closePopupWithEscape: () => surface.runKeymaps(new KeyboardEvent('keydown', { key: 'Escape', cancelable: true }), 'highest'),
    announce: (message, assertive) => input.accessibility.announce(message, assertive),
    platform: /Mac|iP/.test(navigator.platform) ? 'apple' : 'other',
    matchesDefaultKeymap: (event) => surface.matchesKeymap({
      key: event.key, code: event.code ?? '', ctrlKey: event.ctrlKey, metaKey: event.metaKey,
      shiftKey: event.shiftKey, altKey: event.altKey,
    }, 'default'),
    compositionActive: () => surface.compositionRange !== null,
  };
}

export function createVimMode(storage: StorageLike | null, hostEl: HTMLElement): VimMode {
  const [enabled, setEnabled] = createSignal(readVimEnabled(storage));
  let deps: VimDeps | null = null;
  let vim: VimController | null = null;
  let stopStatus: (() => void) | null = null;
  let stopSnapshot: (() => void) | null = null;
  const disable = (): void => {
    if (!deps || !vim) return;
    deps.input.setVim(null);
    stopSnapshot?.(); stopSnapshot = null;
    vim.dispose(); vim = null;
    stopStatus?.(); stopStatus = null;
    delete hostEl.dataset.vim;
    deps.onChange();
  };
  const enable = (): void => {
    if (!deps || vim) return;
    vim = new VimController(createSurfaceHost(deps));
    deps.input.setVim(vim);
    hostEl.dataset.vim = 'on';
    const [snapshot, setSnapshot] = createSignal<VimSnapshot>(vim.snapshot);
    stopSnapshot = vim.subscribe((next) => { setSnapshot(next); deps?.onChange(); });
    stopStatus = mountVimStatus(hostEl, snapshot, {
      submit: (text) => { vim?.submitPrompt(text); deps?.input.focus(); },
      cancel: () => { vim?.cancelPrompt(); deps?.input.focus(); },
      tapMode: () => { vim?.tapMode(); deps?.input.focus(); },
    });
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
    dispose() { disable(); deps = null; },
  };
}
