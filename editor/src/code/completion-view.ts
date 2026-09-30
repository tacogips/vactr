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
import { Prec, StateEffect, Transaction } from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';

/** Connects completion to the current EditorView through the canvas-ready surface contract. */
export function attachCompletion(view: EditorView, engine: CompletionEngine): { dispose(): void } {
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
    text: () => view.state.doc.toString(),
    selection: () => view.state.selection.main,
    replace(from, to, insert) {
      if (from > to) return;
      view.dispatch({
        changes: { from, to, insert },
        selection: { anchor: from + insert.length },
        annotations: Transaction.userEvent.of(COMPLETION_USER_EVENT),
        scrollIntoView: true,
      });
    },
    caretRect(pos) {
      const rect = view.coordsAtPos(pos);
      return rect ? { left: rect.left, top: rect.top, bottom: rect.bottom } : null;
    },
    isComposing: () => view.composing,
    onChange: (listener) => subscribe(changes, listener),
    onKey: (handler) => subscribe(keyHandlers, handler),
    onBlur: (listener) => subscribe(blurHandlers, listener),
    onCompositionStart: (listener) => subscribe(compositionHandlers, listener),
    popupHost: () => view.dom.ownerDocument.body,
  };

  const keyBindings = COMPLETION_KEYS.map((key) => ({
    key,
    run: () => {
      for (const handler of keyHandlers) if (handler(key)) return true;
      return false;
    },
  }));
  view.dispatch({
    effects: StateEffect.appendConfig.of([
      Prec.highest(keymap.of(keyBindings)),
      EditorView.updateListener.of((update) => {
        if (disposed) return;
        const userEvent = [...update.transactions]
          .reverse()
          .map((transaction) => transaction.annotation(Transaction.userEvent))
          .find((event) => event !== undefined) ?? null;
        let inserted = '';
        if (update.docChanged) {
          for (const transaction of update.transactions) {
            transaction.changes.iterChanges((_fromA, _toA, _fromB, _toB, text) => {
              inserted += text.toString();
            });
          }
        }
        const change: SurfaceChange = {
          docChanged: update.docChanged,
          selectionChanged: update.selectionSet,
          userEvent,
          inserted,
        };
        for (const listener of changes) listener(change);
      }),
      EditorView.domEventHandlers({
        blur: () => {
          for (const listener of blurHandlers) listener();
          return false;
        },
        compositionstart: () => {
          for (const listener of compositionHandlers) listener();
          return false;
        },
      }),
    ]),
  });

  const popup = new CompletionPopup(surface, new CompletionService(engine));
  return {
    dispose() {
      if (disposed) return;
      disposed = true;
      popup.dispose();
      changes.clear();
      keyHandlers.clear();
      blurHandlers.clear();
      compositionHandlers.clear();
    },
  };
}
