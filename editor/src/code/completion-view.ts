import { CompletionService } from './completion';
import { CompletionPopup } from './completion-popup';
import {
  COMPLETION_KEYS,
  COMPLETION_USER_EVENT,
  type CompletionEngine,
  type CompletionKey,
  type CompletionSurface,
  type SurfaceChange,
} from './completion-types';
import { Transaction } from '@codemirror/state';
import type { CodeSurface } from '../app/apis';

/** Connects the UI-agnostic completion core to the headless code surface. */
export function attachCompletion(code: CodeSurface, engine: CompletionEngine): { dispose(): void } {
  const changes = new Set<(change: SurfaceChange) => void>();
  const keyHandlers = new Set<(key: CompletionKey) => boolean>();
  const blurHandlers = new Set<() => void>();
  const compositionHandlers = new Set<() => void>();
  let disposed = false;

  const subscribe = <T>(listeners: Set<T>, listener: T): (() => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  };
  const surface: CompletionSurface = {
    text: () => code.state.doc.toString(),
    version: () => code.state.doc,
    selection: () => code.state.selection.main,
    replace(from, to, insert) {
      if (from > to) return;
      code.dispatch({
        changes: { from, to, insert },
        selection: { anchor: from + insert.length },
        annotations: Transaction.userEvent.of(COMPLETION_USER_EVENT),
      });
    },
    caretRect(pos) {
      const rect = code.coordsAtPos(pos);
      return rect ? { left: rect.left, top: rect.top, bottom: rect.bottom } : null;
    },
    isComposing: () => code.compositionRange !== null,
    onChange: (listener) => subscribe(changes, listener),
    onKey: (handler) => subscribe(keyHandlers, handler),
    onBlur: (listener) => subscribe(blurHandlers, listener),
    onCompositionStart: (listener) => subscribe(compositionHandlers, listener),
    popupHost: () => document.body,
  };

  const keyBindings = COMPLETION_KEYS.map((key) => ({
    key,
    run: () => {
      for (const handler of keyHandlers) if (handler(key)) return true;
      return false;
    },
  }));
  const disposeKeymap = code.addKeymap(keyBindings, 'highest');
  const disposeBlur = code.onBlur(() => { for (const listener of blurHandlers) listener(); });
  const disposeComposition = code.onCompositionStart(() => { for (const listener of compositionHandlers) listener(); });
  const disposeUpdate = code.subscribe((update) => {
    if (disposed) return;
    let inserted = '';
    if (update.docChanged) update.changes.iterChanges((_fromA, _toA, fromB, toB) => {
      inserted += update.state.doc.sliceString(fromB, toB);
    });
    const change: SurfaceChange = { docChanged: update.docChanged, selectionChanged: update.selectionSet,
      userEvent: update.userEvent, inserted };
    for (const listener of changes) listener(change);
  });

  const popup = new CompletionPopup(surface, new CompletionService(engine));
  return {
    dispose() {
      if (disposed) return;
      disposed = true;
      disposeKeymap(); disposeBlur(); disposeComposition(); disposeUpdate();
      popup.dispose();
      changes.clear();
      keyHandlers.clear();
      blurHandlers.clear();
      compositionHandlers.clear();
    },
  };
}
