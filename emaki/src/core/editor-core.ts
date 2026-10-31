import { Compartment, EditorState, Transaction, type ChangeSet, type Text, type Extension, type TransactionSpec } from '@codemirror/state';
import { history, historyField, undo, redo, undoDepth, redoDepth } from '@codemirror/commands';
import type { Range, Rect } from '../types';
import { HISTORY_UNDO_BYTES, trimUndoHistory } from './undo-budget';
import type { NumericGesture } from '../input/pointer';

export interface SurfaceBridge {
  focus(): void;
  posAtCoords(coords: { x: number; y: number }): number | null;
  coordsAtPos(pos: number): Rect | null;
}
export interface ExternalHistoryBudget { readonly retainedBytes: number; readonly reducedDepth: boolean; trimToBytes(bytes: number): void }
export interface EditorUpdate { state: EditorState; changes: ChangeSet; docChanged: boolean; selectionSet: boolean; userEvent: string | null }
export interface EditorCoreOptions {
  doc: Text | string;
  externalHistory?: ExternalHistoryBudget;
  extensions?: Extension;
  historyByteLimit?: number;
}

/** Single CPU document authority. Rendering and DOM input are attached by later plans. */
export class EditorCore {
  private current: EditorState;
  private readonly externalHistory?: ExternalHistoryBudget;
  private readonly transactionListeners = new Set<(tr: Transaction) => void>();
  private readonly undoSlot = new Compartment();
  private readonly listeners = new Set<(update: EditorUpdate) => void>();
  private readonly pointers = new Set<(event: PointerEvent) => void>();
  private readonly blurListeners = new Set<() => void>();
  private readonly compositionListeners = new Set<() => void>();
  private readonly numericDragProviders = new Set<(event: PointerEvent, pos: number) => NumericGesture | null>();
  private readonly highestKeymaps: { key: string; run(): boolean }[][] = [];
  private readonly defaultKeymaps: { key: string; run(): boolean }[][] = [];
  private bridge: SurfaceBridge | null = null;
  private composing: Range | null = null;
  private deferred: (() => void)[] = [];
  private disposed = false;
  private dispatching = false;
  private readonly byteLimit: number;
  private undoBytes = 0;
  private reduced = false;
  private readonly historyMemo = new WeakMap<object, number>();

  constructor(options: EditorCoreOptions) {
    this.externalHistory = options.externalHistory;
    this.byteLimit = Math.max(0, Math.min(HISTORY_UNDO_BYTES, options.historyByteLimit ?? HISTORY_UNDO_BYTES));
    this.current = EditorState.create({ doc: options.doc, extensions: [this.undoSlot.of(history()), options.extensions ?? []] });
  }
  get state(): EditorState { return this.current; }
  get compositionRange(): Range | null { return this.composing; }
  get retentionStatus(): { historyBytes: number; undoBytes: number; reducedDepth: boolean; undoDepth: number; redoDepth: number } {
    return { historyBytes: (this.externalHistory?.retainedBytes ?? 0), undoBytes: this.undoBytes,
      reducedDepth: this.reduced || (this.externalHistory?.reducedDepth ?? false), undoDepth: undoDepth(this.state), redoDepth: redoDepth(this.state) };
  }
  dispatch(...specs: (TransactionSpec | Transaction)[]): void {
    if (this.disposed) throw new Error('EditorCore disposed');
    if (this.dispatching) throw new Error('Reentrant EditorCore dispatch');
    if (!specs.length) return;
    // A Transaction may only be dispatched against the authority that created it.
    const transactions = specs.filter((s): s is Transaction => 'startState' in s);
    if (transactions.length && (specs.length !== 1 || transactions[0]?.startState !== this.current)) throw new Error('Stale or mixed transaction');
    const tr = transactions[0] ?? this.current.update(...specs as TransactionSpec[]);
    this.dispatching = true;
    try {
      this.current = tr.state;
      for (const cb of [...this.transactionListeners]) cb(tr);
      if (tr.docChanged) {
        if (this.composing) this.composing = Object.freeze({ from: tr.changes.mapPos(this.composing.from, -1), to: tr.changes.mapPos(this.composing.to, 1) });
      }
      this.boundHistory();
      this.publish({ state: this.current, changes: tr.changes, docChanged: tr.docChanged, selectionSet: tr.selection !== undefined,
        userEvent: tr.annotation(Transaction.userEvent) ?? null });
    } finally { this.dispatching = false; }
  }
  private boundHistory(): void {
    const value = this.current.field(historyField);
    const initial = trimUndoHistory(value, this.byteLimit, this.historyMemo);
    this.externalHistory?.trimToBytes(Math.max(0, this.byteLimit - initial.bytes));
    const bounded = trimUndoHistory(initial.value, this.byteLimit - (this.externalHistory?.retainedBytes ?? 0), this.historyMemo);
    this.undoBytes = bounded.bytes;
    if (!initial.reduced && !bounded.reduced) return;
    this.reduced = true;
    // Remove and reinitialize only the history field; all other state fields survive.
    this.current = this.current.update({ effects: this.undoSlot.reconfigure([]), filter: false }).state;
    this.current = this.current.update({ effects: this.undoSlot.reconfigure([history(), historyField.init(() => bounded.value)]), filter: false }).state;
  }
  undo(): boolean { return undo({ state: this.state, dispatch: (tr) => this.dispatch(tr) }); }
  redo(): boolean { return redo({ state: this.state, dispatch: (tr) => this.dispatch(tr) }); }
  onTransaction(cb: (tr: Transaction) => void): () => void { this.transactionListeners.add(cb); return () => this.transactionListeners.delete(cb); }
  subscribe(cb: (update: EditorUpdate) => void): () => void {
    if (this.disposed) throw new Error('EditorCore disposed');
    this.listeners.add(cb);
    return () => this.listeners.delete(cb);
  }
  protected publish(update: EditorUpdate): void { for (const cb of [...this.listeners]) cb(update); }
  attachBridge(bridge: SurfaceBridge): () => void {
    this.bridge = bridge;
    return () => { if (this.bridge === bridge) this.bridge = null; };
  }
  focus(): void { this.bridge?.focus(); }
  posAtCoords(coords: { x: number; y: number }): number | null { return this.bridge?.posAtCoords(coords) ?? null; }
  coordsAtPos(pos: number): Rect | null { return this.bridge?.coordsAtPos(pos) ?? null; }
  onPointer(cb: (event: PointerEvent) => void): () => void { this.pointers.add(cb); return () => this.pointers.delete(cb); }
  notifyPointer(event: PointerEvent): void { if (!this.disposed) for (const cb of [...this.pointers]) cb(event); }
  notifyBlur(): void { if (!this.disposed) for (const cb of [...this.blurListeners]) cb(); }
  notifyCompositionStart(): void { if (!this.disposed) for (const cb of [...this.compositionListeners]) cb(); }
  onBlur(cb: () => void): () => void { this.blurListeners.add(cb); return () => this.blurListeners.delete(cb); }
  onCompositionStart(cb: () => void): () => void { this.compositionListeners.add(cb); return () => this.compositionListeners.delete(cb); }
  addKeymap(bindings: readonly { key: string; run(): boolean }[], precedence: 'highest' | 'default' = 'default'): () => void {
    const registry = precedence === 'highest' ? this.highestKeymaps : this.defaultKeymaps;
    const group = [...bindings]; registry.push(group);
    return () => { const index = registry.indexOf(group); if (index >= 0) registry.splice(index, 1); };
  }
  runKeymaps(event: KeyboardEvent, precedence?: 'highest' | 'default'): boolean {
    const groups = precedence === undefined ? [...this.highestKeymaps, ...this.defaultKeymaps]
      : [...(precedence === 'highest' ? this.highestKeymaps : this.defaultKeymaps)];
    for (const group of groups) for (const binding of group) {
      if (matchesKey(event, binding.key) && binding.run()) { event.preventDefault(); return true; }
    }
    return false;
  }
  matchesKeymap(event: Pick<KeyboardEvent, 'key' | 'code' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'>, precedence: 'highest' | 'default'): boolean {
    const registry = precedence === 'highest' ? this.highestKeymaps : this.defaultKeymaps;
    return registry.some(group => group.some(binding => matchesKey(event, binding.key)));
  }
  registerNumericDrag(provider: (event: PointerEvent, pos: number) => NumericGesture | null): () => void {
    this.numericDragProviders.add(provider); return () => this.numericDragProviders.delete(provider);
  }
  numericDrag(event: PointerEvent, pos: number): NumericGesture | null {
    for (const provider of this.numericDragProviders) { const gesture = provider(event, pos); if (gesture) return gesture; }
    return null;
  }
  setCompositionRange(range: Range | null): void {
    this.composing = range ? Object.freeze({ ...range }) : null;
    if (!range) { const pending = this.deferred; this.deferred = []; for (const write of pending) write(); }
  }
  deferSourceWrite(range: Range, write: () => void): void {
    if (this.disposed) return;
    const c = this.composing;
    if (c && range.from <= c.to && range.to >= c.from) this.deferred.push(write);
    else write();
  }
  dispose(): void {
    this.disposed = true;
    this.listeners.clear(); this.pointers.clear(); this.blurListeners.clear(); this.compositionListeners.clear();
    this.numericDragProviders.clear(); this.transactionListeners.clear(); this.highestKeymaps.length = 0; this.defaultKeymaps.length = 0;
    this.deferred = [];
    this.composing = null; this.bridge = null;
  }
}

function matchesKey(event: Pick<KeyboardEvent, 'key' | 'code' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'>, binding: string): boolean {
  const parts = binding.split('-'); const key = parts.pop()?.toLowerCase();
  if (!key) return false;
  const modifiers = new Set(parts);
  const apple = /Mac|iP/.test(typeof navigator === 'undefined' ? '' : navigator.platform);
  const wantsCtrl = modifiers.has('Ctrl') || (modifiers.has('Mod') && !apple);
  const wantsMeta = modifiers.has('Meta') || (modifiers.has('Mod') && apple);
  if (event.ctrlKey !== wantsCtrl || event.metaKey !== wantsMeta) return false;
  if (!!event.shiftKey !== modifiers.has('Shift') || !!event.altKey !== modifiers.has('Alt')) return false;
  const produced = event.key.toLowerCase();
  if (produced === key) return true;
  if ((event.shiftKey || event.altKey) && event.code) {
    const code = event.code.toLowerCase();
    const unshifted = code.startsWith('key') ? code.slice(3) : code.startsWith('digit') ? code.slice(5) : code;
    return unshifted === key;
  }
  return false;
}
