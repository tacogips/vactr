import type { Extension } from '@codemirror/state';
import { EditorCore, touches } from 'emaki/engine';
import type { CodeAnnotation, CodeSurface as SurfaceContract, MomentaryGesture, MomentaryProvider, MomentaryStart } from '../app/apis';
import { DocumentSync } from './sync';

export interface CodeSurfaceOptions {
  sync: DocumentSync;
  extensions?: Extension;
  historyByteLimit?: number;
}

export class CodeSurface extends EditorCore implements SurfaceContract {
  readonly sync: DocumentSync;
  private readonly annotations = new Map<string, readonly CodeAnnotation[]>();
  private readonly momentaryProviders = new Set<MomentaryProvider>();
  private readonly animatedSources = new Map<string, (frameMs: number) => readonly CodeAnnotation[]>();
  private readonly animatedWakeListeners = new Set<() => void>();
  private surfaceDisposed = false;

  constructor(options: CodeSurfaceOptions) {
    const doc = options.sync.history.text(options.sync.history.current);
    if (!doc) throw new Error('Missing current revision');
    super({ doc, extensions: options.extensions, historyByteLimit: options.historyByteLimit, externalHistory: options.sync.history });
    this.sync = options.sync;
    this.onTransaction((tr) => this.sync.apply(tr));
    this.onTransaction((tr) => {
      if (!tr.docChanged) return;
      for (const [owner, ranges] of this.annotations) this.annotations.set(owner, Object.freeze(ranges.flatMap((r) =>
        touches(tr.changes, r.from, r.to) ? [] : [Object.freeze({ ...r, from: tr.changes.mapPos(r.from, 1), to: tr.changes.mapPos(r.to, r.from === r.to ? 1 : -1) })])));
    });
  }
  annotate(owner: string, ranges: readonly CodeAnnotation[]): void {
    if (this.surfaceDisposed) return;
    for (const r of ranges) if (!Number.isInteger(r.from) || !Number.isInteger(r.to) || r.from < 0 || r.to < r.from || r.to > this.state.doc.length) throw new RangeError('Invalid annotation');
    if (ranges.length) this.annotations.set(owner, Object.freeze(ranges.map((r) => Object.freeze({ ...r }))));
    else this.annotations.delete(owner);
    this.publish({ state: this.state, changes: this.state.changes(), docChanged: false, selectionSet: false, userEvent: null });
  }
  annotationRanges(): readonly CodeAnnotation[] { return Object.freeze([...this.annotations.values()].flat()); }
  registerMomentaryDrag(provider: MomentaryProvider): () => void { this.momentaryProviders.add(provider); return () => this.momentaryProviders.delete(provider); }
  momentaryHit(pos: number): boolean { for (const p of this.momentaryProviders) if (p.hit(pos)) return true; return false; }
  momentaryDrag(start: MomentaryStart): MomentaryGesture | null { for (const p of this.momentaryProviders) { const g = p.begin(start); if (g) return g; } return null; }
  registerAnimated(owner: string, rows: (frameMs: number) => readonly CodeAnnotation[]): { wake(): void; dispose(): void } {
    this.animatedSources.set(owner, rows);
    return { wake: () => { for (const cb of [...this.animatedWakeListeners]) cb(); }, dispose: () => { if (this.animatedSources.get(owner) === rows) this.animatedSources.delete(owner); } };
  }
  animatedRows(frameMs: number): CodeAnnotation[] { return [...this.animatedSources.values()].flatMap((rows) => [...rows(frameMs)]); }
  onAnimatedWake(cb: () => void): () => void { this.animatedWakeListeners.add(cb); return () => this.animatedWakeListeners.delete(cb); }
  dispose(): void {
    this.surfaceDisposed = true;
    this.annotations.clear(); this.momentaryProviders.clear(); this.animatedSources.clear(); this.animatedWakeListeners.clear();
    super.dispose();
    this.sync.doc.dispose();
  }
}
