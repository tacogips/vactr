import type { ShapedRun } from './layout';

export const SEGMENT_CLUSTERS = 256;
export const TEXT_BLOCK_INSTANCES = 64;
export const MAX_TEXT_BLOCKS = 1536;

export interface SegmentKey {
  runs: readonly ShapedRun[];
  chunk: number;
  fontGeneration: number;
  dpr: number;
  styleHash: string;
  atlasGeneration: number;
}

export interface CachedSegment<T> {
  key: SegmentKey;
  value: T;
  blocks: number[];
}

/** Stable run-array identity lets unchanged lines survive document renumbering. */
export class SegmentCache<T> {
  private readonly ids = new WeakMap<object, number>();
  private nextId = 1;
  private readonly entries = new Map<string, CachedSegment<T>>();
  private readonly free: number[] = [];
  private highWater = 0;

  get size(): number { return this.entries.size; }
  get usedBlocks(): number { return [...this.entries.values()].reduce((sum, entry) => sum + entry.blocks.length, 0); }
  get highWaterBlocks(): number { return this.highWater; }

  key(key: SegmentKey): string {
    let id = this.ids.get(key.runs as object);
    if (!id) { id = this.nextId++; this.ids.set(key.runs as object, id); }
    return `${id}:${key.chunk}:${key.fontGeneration}:${key.dpr}:${key.styleHash}:${key.atlasGeneration}`;
  }

  get(key: SegmentKey): CachedSegment<T> | undefined { return this.entries.get(this.key(key)); }

  ensureBlocks(key: SegmentKey, count: number): boolean {
    const entry = this.get(key);
    if (!entry) return false;
    if (count <= entry.blocks.length) return true;
    const extra = this.allocateBlocks(count - entry.blocks.length);
    if (!extra) return false;
    entry.blocks.push(...extra); return true;
  }

  set(key: SegmentKey, value: T, blockCount: number): CachedSegment<T> | null {
    const id = this.key(key);
    const prefix = `${id.slice(0, id.indexOf(':', id.indexOf(':') + 1))}:`;
    for (const [oldId, old] of this.entries) if (oldId.startsWith(prefix)) {
      this.entries.delete(oldId); this.releaseBlocks(old.blocks);
    }
    const blocks = this.allocateBlocks(blockCount);
    if (!blocks) return null;
    const entry = { key, value, blocks };
    this.entries.set(id, entry);
    return entry;
  }

  delete(key: SegmentKey): boolean {
    const id = this.key(key), entry = this.entries.get(id);
    if (!entry) return false;
    this.entries.delete(id); this.releaseBlocks(entry.blocks); return true;
  }

  deleteOutside(keep: ReadonlySet<string>): CachedSegment<T>[] {
    const removed: CachedSegment<T>[] = [];
    for (const [id, entry] of this.entries) if (!keep.has(id)) {
      this.entries.delete(id); this.releaseBlocks(entry.blocks); removed.push(entry);
    }
    return removed;
  }

  entriesInOrder(): CachedSegment<T>[] { return [...this.entries.values()]; }

  compact(): Map<number, number> {
    const remap = new Map<number, number>(); let next = 0;
    for (const entry of this.entries.values()) for (let i = 0; i < entry.blocks.length; i++) {
      const old = entry.blocks[i]!; remap.set(old, next); entry.blocks[i] = next++;
    }
    this.highWater = next; this.free.length = 0; return remap;
  }

  clear(): CachedSegment<T>[] {
    const removed = [...this.entries.values()]; this.entries.clear(); this.free.length = 0; this.highWater = 0; return removed;
  }

  private allocateBlocks(count: number): number[] | null {
    if (!Number.isSafeInteger(count) || count < 0 || this.usedBlocks + count > MAX_TEXT_BLOCKS) return null;
    const blocks: number[] = [];
    while (blocks.length < count && this.free.length) blocks.push(this.free.pop()!);
    while (blocks.length < count) {
      if (this.highWater >= MAX_TEXT_BLOCKS) { this.releaseBlocks(blocks); return null; }
      blocks.push(this.highWater++);
    }
    return blocks;
  }

  private releaseBlocks(blocks: readonly number[]): void { this.free.push(...blocks); }
}
