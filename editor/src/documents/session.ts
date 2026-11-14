import { Text, Transaction } from '@codemirror/state';
import { isolateHistory } from '@codemirror/commands';
import type { CodeSurface } from '../app/apis';
import type { DocSource } from './types';

/** Tracks the identity and saved baseline for the single CodeSurface document. */
export class DocumentSession {
  private currentSource: DocSource = { kind: 'new' };
  private baseline: Text | null = null;
  private changedSinceConstruction = false;
  private readonly listeners = new Set<() => void>();
  private readonly unsubscribe: () => void;
  private disposed = false;

  constructor(private readonly surface: CodeSurface) {
    this.unsubscribe = surface.subscribe((update) => {
      if (update.docChanged) this.changedSinceConstruction = true;
    });
  }

  get source(): DocSource { return this.currentSource; }

  dirty(): boolean {
    const doc = this.surface.state.doc;
    return this.baseline === null ? !doc.eq(Text.empty) : !doc.eq(this.baseline);
  }

  replace(text: string, source: DocSource): void {
    const doc = this.surface.state.doc;
    this.surface.dispatch({
      changes: { from: 0, to: doc.length, insert: text },
      selection: { anchor: 0 },
      annotations: [isolateHistory.of('full'), Transaction.userEvent.of('input.replace.document')],
      scrollIntoView: true,
    });
    this.currentSource = source;
    this.baseline = this.surface.state.doc;
    this.notifyBaseline();
  }

  loadInitial(text: string, source: DocSource, opts: { restored: boolean }): boolean {
    if (this.changedSinceConstruction || !this.surface.state.doc.eq(Text.empty)) return false;
    this.surface.dispatch({
      changes: { from: 0, to: this.surface.state.doc.length, insert: text },
      selection: { anchor: 0 },
      annotations: [Transaction.addToHistory.of(false), Transaction.userEvent.of('input.load.document')],
      scrollIntoView: true,
    });
    this.currentSource = source;
    this.baseline = opts.restored ? null : this.surface.state.doc;
    this.notifyBaseline();
    return true;
  }

  markSaved(source: DocSource): void {
    this.currentSource = source;
    this.baseline = this.surface.state.doc;
    this.notifyBaseline();
  }

  setSource(source: DocSource): void { this.currentSource = source; }

  onBaseline(cb: () => void): () => void {
    this.listeners.add(cb);
    return () => this.listeners.delete(cb);
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.unsubscribe();
    this.listeners.clear();
  }

  private notifyBaseline(): void {
    for (const listener of [...this.listeners]) listener();
  }
}
