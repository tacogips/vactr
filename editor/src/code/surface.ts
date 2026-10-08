import { Compartment, EditorState, Transaction, type Extension, type TransactionSpec } from '@codemirror/state';
import { history, historyField, undo, redo, undoDepth, redoDepth } from '@codemirror/commands';
import type { CodeAnnotation, CodeRange, CodeRect, CodeSurface as SurfaceContract, CodeSurfaceUpdate, MomentaryGesture, MomentaryProvider, MomentaryStart } from '../app/apis';
import { HISTORY_UNDO_BYTES, trimUndoHistory, touches } from './history';
import { DocumentSync } from './sync';
import type { NumericGesture } from './pointer';

export interface SurfaceBridge {
  focus(): void;
  posAtCoords(coords: { x: number; y: number }): number | null;
  coordsAtPos(pos: number): CodeRect | null;
}
export interface CodeSurfaceOptions {
  sync: DocumentSync;
  extensions?: Extension;
  historyByteLimit?: number;
}

/** Single CPU document authority. Rendering and DOM input are attached by later plans. */
export class CodeSurface implements SurfaceContract {
  private current: EditorState;
  readonly sync: DocumentSync;
  private readonly undoSlot = new Compartment();
  private readonly listeners = new Set<(update: CodeSurfaceUpdate) => void>();
  private readonly pointers = new Set<(event: PointerEvent) => void>();
  private readonly blurListeners = new Set<() => void>();
  private readonly compositionListeners = new Set<() => void>();
  private readonly numericDragProviders = new Set<(event: PointerEvent, pos: number) => NumericGesture | null>();
  private readonly momentaryProviders = new Set<MomentaryProvider>();
  private readonly animatedSources = new Map<string, (frameMs: number) => readonly CodeAnnotation[]>();
  private readonly animatedWakeListeners = new Set<() => void>();
  private readonly highestKeymaps: { key: string; run(): boolean }[][] = [];
  private readonly defaultKeymaps: { key: string; run(): boolean }[][] = [];
  private readonly annotations = new Map<string, readonly CodeAnnotation[]>();
  private bridge: SurfaceBridge | null = null;
  private composing: CodeRange | null = null;
  private deferred: (() => void)[] = [];
  private disposed = false;
  private dispatching = false;
  private readonly byteLimit: number;
  private undoBytes = 0;
  private reduced = false;
  private readonly historyMemo = new WeakMap<object, number>();

  constructor(options: CodeSurfaceOptions) {
    this.sync = options.sync;
    this.byteLimit = Math.max(0, Math.min(HISTORY_UNDO_BYTES, options.historyByteLimit ?? HISTORY_UNDO_BYTES));
    const doc = this.sync.history.text(this.sync.history.current);
    if (!doc) throw new Error('Missing current revision');
    this.current = EditorState.create({ doc, extensions: [this.undoSlot.of(history()), options.extensions ?? []] });
  }
  get state(): EditorState { return this.current; }
  get compositionRange(): CodeRange | null { return this.composing; }
  get retentionStatus(): { historyBytes: number; undoBytes: number; reducedDepth: boolean; undoDepth: number; redoDepth: number } {
    return { historyBytes: this.sync.history.retainedBytes, undoBytes: this.undoBytes,
      reducedDepth: this.reduced || this.sync.history.reducedDepth, undoDepth: undoDepth(this.state), redoDepth: redoDepth(this.state) };
  }
  dispatch(...specs: (TransactionSpec | Transaction)[]): void {
    if (this.disposed) throw new Error('CodeSurface disposed');
    if (this.dispatching) throw new Error('Reentrant CodeSurface dispatch');
    if (!specs.length) return;
    // A Transaction may only be dispatched against the authority that created it.
    const transactions = specs.filter((s): s is Transaction => 'startState' in s);
    if (transactions.length && (specs.length !== 1 || transactions[0]?.startState !== this.current)) throw new Error('Stale or mixed transaction');
    const tr = transactions[0] ?? this.current.update(...specs as TransactionSpec[]);
    this.dispatching = true;
    try {
      this.current = tr.state;
      this.sync.apply(tr); // exactly once, before any surface subscriber can issue a write
      if (tr.docChanged) {
        for (const [owner, ranges] of this.annotations) this.annotations.set(owner, Object.freeze(ranges.flatMap((r) =>
          touches(tr.changes, r.from, r.to) ? [] : [Object.freeze({ ...r, from: tr.changes.mapPos(r.from, 1), to: tr.changes.mapPos(r.to, r.from === r.to ? 1 : -1) })])));
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
    this.sync.history.trimToBytes(Math.max(0, this.byteLimit - initial.bytes));
    const bounded = trimUndoHistory(initial.value, this.byteLimit - this.sync.history.retainedBytes, this.historyMemo);
    this.undoBytes = bounded.bytes;
    if (!initial.reduced && !bounded.reduced) return;
    this.reduced = true;
    // Remove and reinitialize only the history field; all other state fields survive.
    this.current = this.current.update({ effects: this.undoSlot.reconfigure([]), filter: false }).state;
    this.current = this.current.update({ effects: this.undoSlot.reconfigure([history(), historyField.init(() => bounded.value)]), filter: false }).state;
  }
  undo(): boolean { return undo({ state: this.state, dispatch: (tr) => this.dispatch(tr) }); }
  redo(): boolean { return redo({ state: this.state, dispatch: (tr) => this.dispatch(tr) }); }
  subscribe(cb: (update: CodeSurfaceUpdate) => void): () => void {
    if (this.disposed) throw new Error('CodeSurface disposed');
    this.listeners.add(cb);
    return () => this.listeners.delete(cb);
  }
  private publish(update: CodeSurfaceUpdate): void { for (const cb of [...this.listeners]) cb(update); }
  attachBridge(bridge: SurfaceBridge): () => void {
    this.bridge = bridge;
    return () => { if (this.bridge === bridge) this.bridge = null; };
  }
  focus(): void { this.bridge?.focus(); }
  posAtCoords(coords: { x: number; y: number }): number | null { return this.bridge?.posAtCoords(coords) ?? null; }
  coordsAtPos(pos: number): CodeRect | null { return this.bridge?.coordsAtPos(pos) ?? null; }
  annotate(owner: string, ranges: readonly CodeAnnotation[]): void {
    if (this.disposed) return;
    for (const r of ranges) if (!Number.isInteger(r.from) || !Number.isInteger(r.to) || r.from < 0 || r.to < r.from || r.to > this.state.doc.length) throw new RangeError('Invalid annotation');
    if (ranges.length) this.annotations.set(owner, Object.freeze(ranges.map((r) => Object.freeze({ ...r }))));
    else this.annotations.delete(owner);
    this.publish({ state: this.state, changes: this.state.changes(), docChanged: false, selectionSet: false, userEvent: null });
  }
  annotationRanges(): readonly CodeAnnotation[] { return Object.freeze([...this.annotations.values()].flat()); }
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
  runKeymaps(event: KeyboardEvent): boolean {
    for (const group of [...this.highestKeymaps, ...this.defaultKeymaps]) for (const binding of group) {
      if (matchesKey(event, binding.key) && binding.run()) { event.preventDefault(); return true; }
    }
    return false;
  }
  registerNumericDrag(provider: (event: PointerEvent, pos: number) => NumericGesture | null): () => void {
    this.numericDragProviders.add(provider); return () => this.numericDragProviders.delete(provider);
  }
  numericDrag(event: PointerEvent, pos: number): NumericGesture | null {
    for (const provider of this.numericDragProviders) { const gesture = provider(event, pos); if (gesture) return gesture; }
    return null;
  }
  registerMomentaryDrag(provider: MomentaryProvider): () => void { this.momentaryProviders.add(provider); return () => this.momentaryProviders.delete(provider); }
  momentaryHit(pos: number): boolean { for (const p of this.momentaryProviders) if (p.hit(pos)) return true; return false; }
  momentaryDrag(start: MomentaryStart): MomentaryGesture | null { for (const p of this.momentaryProviders) { const g = p.begin(start); if (g) return g; } return null; }
  registerAnimated(owner: string, rows: (frameMs: number) => readonly CodeAnnotation[]): { wake(): void; dispose(): void } {
    this.animatedSources.set(owner, rows);
    return { wake: () => { for (const cb of [...this.animatedWakeListeners]) cb(); }, dispose: () => { if (this.animatedSources.get(owner) === rows) this.animatedSources.delete(owner); } };
  }
  animatedRows(frameMs: number): CodeAnnotation[] { return [...this.animatedSources.values()].flatMap((rows) => [...rows(frameMs)]); }
  onAnimatedWake(cb: () => void): () => void { this.animatedWakeListeners.add(cb); return () => this.animatedWakeListeners.delete(cb); }
  setCompositionRange(range: CodeRange | null): void {
    this.composing = range ? Object.freeze({ ...range }) : null;
    if (!range) { const pending = this.deferred; this.deferred = []; for (const write of pending) write(); }
  }
  deferSourceWrite(range: CodeRange, write: () => void): void {
    if (this.disposed) return;
    const c = this.composing;
    if (c && range.from <= c.to && range.to >= c.from) this.deferred.push(write);
    else write();
  }
  dispose(): void {
    this.disposed = true;
    this.listeners.clear(); this.pointers.clear(); this.blurListeners.clear(); this.compositionListeners.clear();
    this.numericDragProviders.clear(); this.momentaryProviders.clear(); this.animatedSources.clear(); this.animatedWakeListeners.clear(); this.highestKeymaps.length = 0; this.defaultKeymaps.length = 0;
    this.annotations.clear(); this.deferred = [];
    this.composing = null; this.bridge = null;
    this.sync.doc.dispose();
  }
}

function matchesKey(event: KeyboardEvent, binding: string): boolean {
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
