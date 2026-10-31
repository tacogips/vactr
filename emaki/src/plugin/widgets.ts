import { MapMode, type ChangeSet, type Text } from '@codemirror/state';
import type { BlockWidgetSpec, EolWidgetSpec } from '../types';

type Spec = BlockWidgetSpec | EolWidgetSpec;
export type ResolvedBlockWidget = BlockWidgetSpec & { id: string; line: number };
export type ResolvedEolWidget = EolWidgetSpec & { id: string; line: number };

/** Owns widget anchors; line numbers in resolved widgets are zero based. */
export class WidgetSet {
  private owners = new Map<string, { blocks: BlockWidgetSpec[]; eols: EolWidgetSpec[] }>();
  private listeners = new Set<(id: string) => void>();
  private version = 0;
  constructor(private length: number) {}
  get revision(): number { return this.version; }
  onRemoved(cb: (id: string) => void): () => void { this.listeners.add(cb); return () => this.listeners.delete(cb); }
  private removed(id: string): void { for (const cb of this.listeners) cb(id); }
  set(owner: string, blocks: readonly BlockWidgetSpec[], eol: readonly EolWidgetSpec[] = []): void {
    const keys = new Set<string>();
    for (const spec of [...blocks, ...eol]) {
      if (!Number.isInteger(spec.pos) || spec.pos < 0 || spec.pos > this.length || keys.has(spec.key)) throw new RangeError('Invalid widget anchor or duplicate key');
      keys.add(spec.key);
      const size = 'height' in spec ? spec.height : spec.width;
      if (!Number.isFinite(size) || size < 0 || size > ('height' in spec ? 10000 : 4000)) throw new RangeError('Invalid widget size');
    }
    const old = this.owners.get(owner);
    for (const spec of [...(old?.blocks ?? []), ...(old?.eols ?? [])]) {
      const next = [...blocks, ...eol].find(item => item.key === spec.key);
      if (!next || next.content.kind !== spec.content.kind) this.removed(`${owner}\0${spec.key}`);
    }
    const same = (a: readonly Spec[], b: readonly Spec[]) => a.length === b.length && a.every((item, i) => {
      const next = b[i]; return Object.keys(item).length === Object.keys(next).length && Object.entries(item).every(([k, v]) => (next as unknown as Record<string, unknown>)[k] === v);
    });
    if (old && same(old.blocks, blocks) && same(old.eols, eol)) return;
    if (!old && !blocks.length && !eol.length) return;
    this.owners.set(owner, { blocks: blocks.map(x => ({ ...x })), eols: eol.map(x => ({ ...x })) }); this.version++;
  }
  clear(owner: string): void {
    const old = this.owners.get(owner); if (!old) return;
    this.owners.delete(owner); for (const spec of [...old.blocks, ...old.eols]) this.removed(`${owner}\0${spec.key}`); this.version++;
  }
  map(changes: ChangeSet): void {
    if (changes.length !== this.length) throw new RangeError('Widget document length mismatch');
    let changed = false;
    for (const [owner, group] of this.owners) {
      const map = <T extends Spec>(items: T[]): T[] => items.flatMap(item => {
        const pos = changes.mapPos(item.pos, -1, MapMode.TrackDel);
        if (pos === null) { this.removed(`${owner}\0${item.key}`); changed = true; return []; }
        if (pos !== item.pos) { changed = true; return [{ ...item, pos }]; }
        return [item];
      });
      group.blocks = map(group.blocks); group.eols = map(group.eols);
    }
    this.length = changes.newLength; if (changed) this.version++;
  }
  blocks(doc: Text): ResolvedBlockWidget[] { return [...this.owners].flatMap(([owner, group]) => group.blocks.map(spec => ({ ...spec, id: `${owner}\0${spec.key}`, line: doc.lineAt(spec.pos).number - 1 }))); }
  eols(doc: Text): ResolvedEolWidget[] { return [...this.owners].flatMap(([owner, group]) => group.eols.map(spec => ({ ...spec, id: `${owner}\0${spec.key}`, line: doc.lineAt(spec.pos).number - 1 }))); }
  entries(doc: Text): { line: number; above: number; below: number }[] {
    const entries = new Map<number, { line: number; above: number; below: number }>();
    for (const block of this.blocks(doc)) { const entry = entries.get(block.line) ?? { line: block.line, above: 0, below: 0 }; entry[block.side] += block.height; entries.set(block.line, entry); }
    return [...entries.values()].sort((a, b) => a.line - b.line);
  }
}
